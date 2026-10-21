//! Title details from the addon protocol's `meta` resource (`{base}/meta/{type}/{id}.json`).

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaDetails {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub poster: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub release_info: Option<String>,
    pub runtime: Option<String>,
    pub imdb_rating: Option<String>,
    pub genres: Vec<String>,
    pub directors: Vec<String>,
    pub cast: Vec<String>,
    /// Episodes grouped by season, regular seasons first and specials (season 0) last.
    pub seasons: Vec<Season>,
    /// Videos without a season number, such as the parts of a channel or a movie collection.
    pub videos: Vec<Episode>,
    /// Every video id in the order the addon lists them, which Stremio's watched marks follow.
    pub video_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub number: u32,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    /// The video id used to request streams for this episode.
    pub id: String,
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub released: Option<String>,
    pub thumbnail: Option<String>,
    pub overview: Option<String>,
}

fn text(value: &Value, key: &str) -> Option<String> {
    let v = value.get(key)?;
    let s = match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

fn number(value: &Value, key: &str) -> Option<u32> {
    let v = value.get(key)?;
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .and_then(|n| u32::try_from(n).ok())
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Names from `links` entries of one category, e.g. `{"name": "…", "category": "Cast"}`.
fn link_names(meta: &Value, category: &str) -> Vec<String> {
    meta.get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|l| {
            l.get("category")
                .and_then(Value::as_str)
                .is_some_and(|c| c.eq_ignore_ascii_case(category))
        })
        .filter_map(|l| text(l, "name"))
        .collect()
}

/// Prefers `links` of a category and falls back to the older plain list fields.
fn people(meta: &Value, category: &str, legacy_keys: &[&str]) -> Vec<String> {
    let from_links = link_names(meta, category);
    if !from_links.is_empty() {
        return from_links;
    }
    legacy_keys
        .iter()
        .map(|key| strings(meta, key))
        .find(|list| !list.is_empty())
        .unwrap_or_default()
}

fn parse_episode(video: &Value) -> Option<Episode> {
    let id = text(video, "id")?;
    let season = number(video, "season");
    let episode = number(video, "episode").or_else(|| number(video, "number"));
    let title = text(video, "title")
        .or_else(|| text(video, "name"))
        .unwrap_or_else(|| match episode {
            Some(n) => format!("Episode {n}"),
            None => id.clone(),
        });
    Some(Episode {
        title,
        season,
        episode,
        released: text(video, "released").or_else(|| text(video, "firstAired")),
        thumbnail: text(video, "thumbnail"),
        overview: text(video, "overview").or_else(|| text(video, "description")),
        id,
    })
}

fn group_videos(meta: &Value) -> (Vec<Season>, Vec<Episode>) {
    let mut seasons: Vec<Season> = Vec::new();
    let mut loose = Vec::new();
    let videos = meta.get("videos").and_then(Value::as_array);
    for episode in videos.into_iter().flatten().filter_map(parse_episode) {
        match episode.season {
            Some(number) => match seasons.iter_mut().find(|s| s.number == number) {
                Some(season) => season.episodes.push(episode),
                None => seasons.push(Season {
                    number,
                    episodes: vec![episode],
                }),
            },
            None => loose.push(episode),
        }
    }
    // Specials (season 0) go after the regular seasons.
    seasons.sort_by_key(|s| (s.number == 0, s.number));
    for season in &mut seasons {
        season
            .episodes
            .sort_by_key(|e| (e.episode.unwrap_or(u32::MAX), e.released.clone()));
    }
    (seasons, loose)
}

/// Reads the `meta` object of a meta response. Returns `None` when there is no usable meta.
pub fn parse_meta(body: &Value) -> Option<MetaDetails> {
    let meta = body.get("meta").filter(|m| m.is_object())?;
    let id = text(meta, "id")?;
    let (seasons, videos) = group_videos(meta);
    let video_ids = meta
        .get("videos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_episode)
        .map(|e| e.id)
        .collect();
    let mut genres = link_names(meta, "Genres");
    if genres.is_empty() {
        genres = strings(meta, "genres");
    }
    if genres.is_empty() {
        genres = strings(meta, "genre");
    }
    Some(MetaDetails {
        kind: text(meta, "type").unwrap_or_default(),
        name: text(meta, "name").unwrap_or_else(|| id.clone()),
        poster: text(meta, "poster"),
        background: text(meta, "background"),
        logo: text(meta, "logo"),
        description: text(meta, "description"),
        release_info: text(meta, "releaseInfo").or_else(|| text(meta, "year")),
        runtime: text(meta, "runtime"),
        imdb_rating: text(meta, "imdbRating"),
        genres,
        directors: people(meta, "Directors", &["director", "directors"]),
        cast: people(meta, "Cast", &["cast"]),
        seasons,
        videos,
        video_ids,
        id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn series() -> Value {
        json!({"meta": {
            "id": "tt0903747", "type": "series", "name": "Breaking Bad",
            "poster": "https://p/poster.jpg", "background": "https://p/bg.jpg",
            "description": "A chemistry teacher…", "releaseInfo": "2008–2013",
            "runtime": "49 min", "imdbRating": "9.5",
            "genres": ["Crime", "Drama"],
            "links": [
                {"name": "Bryan Cranston", "category": "Cast", "url": "stremio:///search?search=x"},
                {"name": "Aaron Paul", "category": "Cast", "url": ""},
                {"name": "Drama", "category": "Genres", "url": ""},
                {"name": "9.5", "category": "imdb", "url": ""}
            ],
            "cast": ["Old Cast Field"],
            "director": ["Vince Gilligan"],
            "videos": [
                {"id": "tt0903747:2:1", "title": "Seven Thirty-Seven", "season": 2, "episode": 1,
                 "released": "2009-03-08T00:00:00.000Z"},
                {"id": "tt0903747:1:2", "name": "Cat's in the Bag...", "season": 1, "episode": 2},
                {"id": "tt0903747:0:1", "title": "Special", "season": 0, "episode": 1},
                {"id": "tt0903747:1:1", "title": "Pilot", "season": 1, "episode": 1,
                 "overview": "Walter White…", "thumbnail": "https://p/1.jpg"},
                {"id": "tt0903747:1:3", "season": "1", "episode": "3"},
                {"title": "no id"}
            ]
        }})
    }

    #[test]
    fn parses_series_details() {
        let meta = parse_meta(&series()).unwrap();
        assert_eq!(meta.name, "Breaking Bad");
        assert_eq!(meta.release_info.as_deref(), Some("2008–2013"));
        // Links win over the older list fields.
        assert_eq!(meta.cast, ["Bryan Cranston", "Aaron Paul"]);
        assert_eq!(meta.genres, ["Drama"]);
        assert_eq!(meta.directors, ["Vince Gilligan"]);
    }

    #[test]
    fn groups_and_sorts_episodes() {
        let meta = parse_meta(&series()).unwrap();
        let numbers: Vec<u32> = meta.seasons.iter().map(|s| s.number).collect();
        assert_eq!(numbers, [1, 2, 0]);
        let season1: Vec<&str> = meta.seasons[0]
            .episodes
            .iter()
            .map(|e| e.title.as_str())
            .collect();
        // Numbers given as strings still sort, and a missing title gets a default.
        assert_eq!(season1, ["Pilot", "Cat's in the Bag...", "Episode 3"]);
        assert_eq!(
            meta.seasons[0].episodes[0].overview.as_deref(),
            Some("Walter White…")
        );
        assert!(meta.videos.is_empty());
        // Watched marks follow the addon's order, not the sorted one.
        assert_eq!(
            meta.video_ids,
            [
                "tt0903747:2:1",
                "tt0903747:1:2",
                "tt0903747:0:1",
                "tt0903747:1:1",
                "tt0903747:1:3"
            ]
        );
    }

    #[test]
    fn parses_movie_with_legacy_fields() {
        let meta = parse_meta(&json!({"meta": {
            "id": "tt0111161", "type": "movie", "name": "The Shawshank Redemption",
            "year": 1994, "genre": ["Drama"], "cast": ["Tim Robbins"],
            "director": ["Frank Darabont"], "videos": []
        }}))
        .unwrap();
        assert_eq!(meta.release_info.as_deref(), Some("1994"));
        assert_eq!(meta.genres, ["Drama"]);
        assert_eq!(meta.cast, ["Tim Robbins"]);
        assert!(meta.seasons.is_empty());
    }

    #[test]
    fn keeps_videos_without_season() {
        let meta = parse_meta(&json!({"meta": {
            "id": "ch1", "type": "channel", "name": "Channel",
            "videos": [{"id": "v1", "title": "First"}, {"id": "v2", "title": "Second"}]
        }}))
        .unwrap();
        assert!(meta.seasons.is_empty());
        assert_eq!(meta.videos.len(), 2);
    }

    #[test]
    fn rejects_missing_meta() {
        assert_eq!(parse_meta(&json!({"meta": null})), None);
        assert_eq!(parse_meta(&json!({})), None);
        assert_eq!(parse_meta(&json!({"meta": {"name": "no id"}})), None);
    }
}

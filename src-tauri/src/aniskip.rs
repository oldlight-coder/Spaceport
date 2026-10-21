//! Intro, outro and recap times for anime from AniSkip (`api.aniskip.com`).
//!
//! AniSkip identifies shows by MyAnimeList id, so the title is first mapped to one:
//! `kitsu:` ids through Kitsu's mapping API, `anilist:` ids through AniList, `mal:` ids as they
//! are, and IMDb-based series (Cinemeta) by searching AniList for the title name, accepting only
//! a result whose title matches.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use crate::stremio::api::StremioApi;

const KITSU_API: &str = "https://kitsu.app/api/edge";
const ANILIST_API: &str = "https://graphql.anilist.co";
const ANISKIP_API: &str = "https://api.aniskip.com/v2/skip-times";
const TIMEOUT: Duration = Duration::from_secs(8);

/// Which anime and episode a video is, as far as its ids tell.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimeRef {
    Kitsu {
        id: u64,
        episode: u32,
    },
    Mal {
        id: u64,
        episode: u32,
    },
    Anilist {
        id: u64,
        episode: u32,
    },
    /// An IMDb-style series: found by name, one MyAnimeList entry per season.
    Named {
        name: String,
        season: u32,
        episode: u32,
    },
}

impl AnimeRef {
    pub fn episode(&self) -> u32 {
        match self {
            Self::Kitsu { episode, .. }
            | Self::Mal { episode, .. }
            | Self::Anilist { episode, .. }
            | Self::Named { episode, .. } => *episode,
        }
    }

    /// The key under which the mapped MyAnimeList id is remembered.
    fn cache_key(&self) -> String {
        match self {
            Self::Kitsu { id, .. } => format!("kitsu:{id}"),
            Self::Mal { id, .. } => format!("mal:{id}"),
            Self::Anilist { id, .. } => format!("anilist:{id}"),
            Self::Named { name, season, .. } => format!("name:{}:{season}", normalize(name)),
        }
    }
}

/// Works out the anime and episode from the title id ("kitsu:7442"), the video id
/// ("kitsu:7442:5" or "tt2560140:2:5") and, for IMDb-style ids, the title's name and genres.
/// Titles that are not anime give `None`.
pub fn anime_ref(
    title_id: &str,
    video_id: &str,
    name: &str,
    genres: &[String],
) -> Option<AnimeRef> {
    let parts: Vec<&str> = video_id.split(':').collect();
    let number = |s: &str| s.parse::<u64>().ok();
    match parts.as_slice() {
        [prefix, id, rest @ ..] if ["kitsu", "mal", "anilist"].contains(prefix) => {
            let id = number(id)?;
            // A movie or single video has no episode part and counts as episode 1.
            let episode = rest.last().and_then(|e| e.parse().ok()).unwrap_or(1);
            Some(match *prefix {
                "kitsu" => AnimeRef::Kitsu { id, episode },
                "mal" => AnimeRef::Mal { id, episode },
                _ => AnimeRef::Anilist { id, episode },
            })
        }
        [imdb, season, episode] if imdb.starts_with("tt") && *imdb == title_id => {
            let is_animation = genres
                .iter()
                .any(|g| g.eq_ignore_ascii_case("animation") || g.eq_ignore_ascii_case("anime"));
            if !is_animation || name.trim().is_empty() {
                return None;
            }
            let season: u32 = season.parse().ok()?;
            if season == 0 {
                return None;
            }
            Some(AnimeRef::Named {
                name: name.to_string(),
                season,
                episode: episode.parse().ok()?,
            })
        }
        _ => None,
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads the MyAnimeList id out of Kitsu's `/anime/{id}/mappings` answer.
pub fn mal_from_kitsu(body: &Value) -> Option<u64> {
    body.get("data")?.as_array()?.iter().find_map(|m| {
        let attributes = m.get("attributes")?;
        if attributes.get("externalSite")?.as_str()? != "myanimelist/anime" {
            return None;
        }
        attributes.get("externalId")?.as_str()?.trim().parse().ok()
    })
}

/// Reads `idMal` out of AniList's answer to a lookup by id.
pub fn mal_from_anilist_id(body: &Value) -> Option<u64> {
    body.pointer("/data/Media/idMal")?.as_u64()
}

/// Picks the AniList search result that is this show: one of its titles (or synonyms) must be
/// the name, followed for later seasons by the season number. Results without a MyAnimeList id
/// are skipped.
pub fn mal_from_anilist_search(body: &Value, name: &str, season: u32) -> Option<u64> {
    let wanted = normalize(name);
    let media = body.pointer("/data/Page/media")?.as_array()?;
    media.iter().find_map(|m| {
        let id = m.get("idMal")?.as_u64()?;
        let mut titles: Vec<String> = ["romaji", "english", "native"]
            .iter()
            .filter_map(|k| m.pointer(&format!("/title/{k}"))?.as_str())
            .map(normalize)
            .collect();
        if let Some(synonyms) = m.get("synonyms").and_then(Value::as_array) {
            titles.extend(synonyms.iter().filter_map(Value::as_str).map(normalize));
        }
        let matches = titles.iter().any(|t| {
            if season == 1 {
                *t == wanted
            } else {
                let rest = t.strip_prefix(&wanted).map(str::trim).unwrap_or("");
                t.starts_with(&wanted) && rest.split_whitespace().any(|w| w == season.to_string())
            }
        });
        matches.then_some(id)
    })
}

/// What a skippable part is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipKind {
    Intro,
    Outro,
    Recap,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipSegment {
    pub kind: SkipKind,
    /// Seconds.
    pub start: f64,
    pub end: f64,
}

/// Reads AniSkip's `results`, one segment per kind (the first given), ordered by start time.
pub fn parse_skip_times(body: &Value) -> Vec<SkipSegment> {
    if body.get("found").and_then(Value::as_bool) == Some(false) {
        return Vec::new();
    }
    let results = body.get("results").and_then(Value::as_array);
    let mut segments: Vec<SkipSegment> = Vec::new();
    for r in results.into_iter().flatten() {
        let kind = match r.get("skipType").and_then(Value::as_str) {
            Some("op" | "mixed-op") => SkipKind::Intro,
            Some("ed" | "mixed-ed") => SkipKind::Outro,
            Some("recap") => SkipKind::Recap,
            _ => continue,
        };
        let (Some(start), Some(end)) = (
            r.pointer("/interval/startTime").and_then(Value::as_f64),
            r.pointer("/interval/endTime").and_then(Value::as_f64),
        ) else {
            continue;
        };
        if end <= start || start < 0.0 || segments.iter().any(|s| s.kind == kind) {
            continue;
        }
        segments.push(SkipSegment { kind, start, end });
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
}

/// Looks up MyAnimeList ids once per show and fetches skip times.
#[derive(Default)]
pub struct AniSkip {
    /// Definite answers per show: the id, or why there is none. Network errors are not kept.
    mal_ids: Mutex<HashMap<String, Result<u64, String>>>,
}

/// Why a lookup stopped, worded for the player's status line.
fn unreachable(service: &str, error: impl std::fmt::Display) -> String {
    format!("could not reach {service} ({error})")
}

/// Names to search AniList with: the full name, then the part before a colon
/// ("Frieren: Beyond Journey's End" also tries "Frieren").
fn search_names(name: &str, season: u32) -> Vec<String> {
    let mut names = vec![name.trim().to_string()];
    if let Some((short, _)) = name.split_once(':') {
        if !short.trim().is_empty() {
            names.push(short.trim().to_string());
        }
    }
    if season > 1 {
        names = names.into_iter().map(|n| format!("{n} {season}")).collect();
    }
    names
}

impl AniSkip {
    async fn mal_id(&self, api: &StremioApi, anime: &AnimeRef) -> Result<u64, String> {
        let key = anime.cache_key();
        if let Some(known) = self.mal_ids.lock().unwrap().get(&key) {
            return known.clone();
        }
        let found: Result<u64, String> = match anime {
            AnimeRef::Mal { id, .. } => Ok(*id),
            AnimeRef::Kitsu { id, .. } => {
                let url = format!("{KITSU_API}/anime/{id}/mappings");
                let body = api
                    .service_get(&url, TIMEOUT)
                    .await
                    .map_err(|e| unreachable("Kitsu", e))?;
                body.as_ref()
                    .and_then(mal_from_kitsu)
                    .ok_or_else(|| format!("Kitsu has no MyAnimeList link for kitsu:{id}"))
            }
            AnimeRef::Anilist { id, .. } => {
                let query = json!({
                    "query": "query($id:Int){Media(id:$id,type:ANIME){idMal}}",
                    "variables": { "id": id }
                });
                let body = api
                    .service_post(ANILIST_API, &query, TIMEOUT)
                    .await
                    .map_err(|e| unreachable("AniList", e))?;
                mal_from_anilist_id(&body)
                    .ok_or_else(|| format!("AniList has no MyAnimeList id for anilist:{id}"))
            }
            AnimeRef::Named { name, season, .. } => {
                let mut found = None;
                for search in search_names(name, *season) {
                    let query = json!({
                        "query": "query($s:String){Page(perPage:10){media(search:$s,type:ANIME){idMal title{romaji english native} synonyms}}}",
                        "variables": { "s": search }
                    });
                    let body = api
                        .service_post(ANILIST_API, &query, TIMEOUT)
                        .await
                        .map_err(|e| unreachable("AniList", e))?;
                    found = mal_from_anilist_search(&body, name, *season);
                    if found.is_some() {
                        break;
                    }
                }
                found.ok_or_else(|| format!("AniList has no show named \"{name}\" season {season}"))
            }
        };
        self.mal_ids.lock().unwrap().insert(key, found.clone());
        found
    }

    /// Skip times for one episode. `episode_length` (seconds) lets AniSkip pick timings that
    /// fit this release; 0 when unknown. The error says where the lookup stopped.
    pub async fn skip_times(
        &self,
        api: &StremioApi,
        anime: &AnimeRef,
        episode_length: f64,
    ) -> Result<Vec<SkipSegment>, String> {
        let mal = self.mal_id(api, anime).await?;
        let url = format!(
            "{ANISKIP_API}/{mal}/{}?types[]=op&types[]=ed&types[]=mixed-op&types[]=mixed-ed&types[]=recap&episodeLength={}",
            anime.episode(),
            episode_length.max(0.0).round() as u64
        );
        let body = api
            .service_get(&url, TIMEOUT)
            .await
            .map_err(|e| unreachable("AniSkip", e))?;
        let segments = body.as_ref().map(parse_skip_times).unwrap_or_default();
        if segments.is_empty() {
            Err(format!(
                "no skip times for episode {} yet (MyAnimeList {mal})",
                anime.episode()
            ))
        } else {
            Ok(segments)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genres(list: &[&str]) -> Vec<String> {
        list.iter().map(|g| g.to_string()).collect()
    }

    #[test]
    fn recognises_anime_ids() {
        assert_eq!(
            anime_ref("kitsu:7442", "kitsu:7442:5", "Attack on Titan", &[]),
            Some(AnimeRef::Kitsu {
                id: 7442,
                episode: 5
            })
        );
        assert_eq!(
            anime_ref("kitsu:7442", "kitsu:7442", "Movie", &[]),
            Some(AnimeRef::Kitsu {
                id: 7442,
                episode: 1
            })
        );
        assert_eq!(
            anime_ref("mal:5114", "mal:5114:3", "x", &[]),
            Some(AnimeRef::Mal {
                id: 5114,
                episode: 3
            })
        );
        assert_eq!(
            anime_ref("anilist:16498", "anilist:16498:12", "x", &[]),
            Some(AnimeRef::Anilist {
                id: 16498,
                episode: 12
            })
        );
    }

    #[test]
    fn imdb_series_only_when_animated() {
        let anime = anime_ref(
            "tt2560140",
            "tt2560140:2:5",
            "Attack on Titan",
            &genres(&["Animation", "Action"]),
        );
        assert_eq!(
            anime,
            Some(AnimeRef::Named {
                name: "Attack on Titan".into(),
                season: 2,
                episode: 5
            })
        );
        assert_eq!(
            anime_ref(
                "tt0903747",
                "tt0903747:1:1",
                "Breaking Bad",
                &genres(&["Drama"])
            ),
            None
        );
        // Specials have no MyAnimeList season to map to.
        assert_eq!(
            anime_ref("tt2560140", "tt2560140:0:1", "AoT", &genres(&["Animation"])),
            None
        );
        // Movies on IMDb ids are left alone.
        assert_eq!(
            anime_ref(
                "tt5311514",
                "tt5311514",
                "Your Name",
                &genres(&["Animation"])
            ),
            None
        );
    }

    #[test]
    fn kitsu_mappings() {
        let body = json!({"data": [
            {"id": "1", "type": "mappings", "attributes": {"externalSite": "anidb", "externalId": "9541"}},
            {"id": "2", "type": "mappings", "attributes": {"externalSite": "myanimelist/anime", "externalId": "16498"}}
        ]});
        assert_eq!(mal_from_kitsu(&body), Some(16498));
        assert_eq!(mal_from_kitsu(&json!({"data": []})), None);
    }

    #[test]
    fn anilist_lookup_by_id() {
        assert_eq!(
            mal_from_anilist_id(&json!({"data": {"Media": {"idMal": 16498}}})),
            Some(16498)
        );
        assert_eq!(
            mal_from_anilist_id(&json!({"data": {"Media": {"idMal": null}}})),
            None
        );
    }

    fn search_body() -> Value {
        json!({"data": {"Page": {"media": [
            {"idMal": 99999, "title": {"romaji": "Shingeki no Kyojin: Lost Girls", "english": "Attack on Titan: Lost Girls", "native": null}, "synonyms": []},
            {"idMal": 16498, "title": {"romaji": "Shingeki no Kyojin", "english": "Attack on Titan", "native": "進撃の巨人"}, "synonyms": ["AoT"]},
            {"idMal": 25777, "title": {"romaji": "Shingeki no Kyojin Season 2", "english": "Attack on Titan Season 2", "native": null}, "synonyms": []},
            {"idMal": null, "title": {"romaji": "Attack on Titan", "english": null, "native": null}, "synonyms": []}
        ]}}})
    }

    #[test]
    fn anilist_search_needs_a_matching_title() {
        assert_eq!(
            mal_from_anilist_search(&search_body(), "Attack on Titan", 1),
            Some(16498)
        );
        assert_eq!(
            mal_from_anilist_search(&search_body(), "attack on titan!", 1),
            Some(16498)
        );
        assert_eq!(
            mal_from_anilist_search(&search_body(), "Attack on Titan", 2),
            Some(25777)
        );
        assert_eq!(
            mal_from_anilist_search(&search_body(), "Attack on Titan", 3),
            None
        );
        assert_eq!(
            mal_from_anilist_search(&search_body(), "The Simpsons", 1),
            None
        );
    }

    #[test]
    fn searches_full_and_short_names() {
        assert_eq!(
            search_names("Frieren: Beyond Journey's End", 1),
            ["Frieren: Beyond Journey's End", "Frieren"]
        );
        assert_eq!(search_names("Attack on Titan", 2), ["Attack on Titan 2"]);
    }

    #[test]
    fn frieren_matches_its_english_title() {
        let body = json!({"data": {"Page": {"media": [
            {"idMal": 52991, "title": {"romaji": "Sousou no Frieren", "english": "Frieren: Beyond Journey\u{2019}s End", "native": null}, "synonyms": ["Frieren at the Funeral"]}
        ]}}});
        assert_eq!(
            mal_from_anilist_search(&body, "Frieren: Beyond Journey's End", 1),
            Some(52991)
        );
    }

    #[test]
    fn skip_times() {
        let body = json!({"found": true, "results": [
            {"interval": {"startTime": 1290.5, "endTime": 1380.0}, "skipType": "ed", "skipId": "b", "episodeLength": 1420.0},
            {"interval": {"startTime": 75.2, "endTime": 165.2}, "skipType": "op", "skipId": "a", "episodeLength": 1420.0},
            {"interval": {"startTime": 80.0, "endTime": 170.0}, "skipType": "mixed-op", "skipId": "c", "episodeLength": 1420.0},
            {"interval": {"startTime": 0.0, "endTime": 45.0}, "skipType": "recap", "skipId": "d", "episodeLength": 1420.0},
            {"interval": {"startTime": 5.0, "endTime": 2.0}, "skipType": "op", "skipId": "bad"}
        ], "message": "Successfully found skip times", "statusCode": 200});
        let segments = parse_skip_times(&body);
        assert_eq!(
            segments,
            [
                SkipSegment {
                    kind: SkipKind::Recap,
                    start: 0.0,
                    end: 45.0
                },
                SkipSegment {
                    kind: SkipKind::Intro,
                    start: 75.2,
                    end: 165.2
                },
                SkipSegment {
                    kind: SkipKind::Outro,
                    start: 1290.5,
                    end: 1380.0
                },
            ]
        );
        assert!(parse_skip_times(&json!({"found": false, "results": []})).is_empty());
        assert!(parse_skip_times(&json!({})).is_empty());
    }
}

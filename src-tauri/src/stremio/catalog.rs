//! Catalogs from the addon protocol's `catalog` resource.
//!
//! A catalog is listed in an addon manifest as `{type, id, name, extra}` and fetched from
//! `{base}/catalog/{type}/{id}.json`, or `{base}/catalog/{type}/{id}/{extra}.json` when extra
//! properties such as `genre` or `skip` are given.

use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

use super::addons::Addon;

/// Cinemeta, Stremio's official metadata addon and Spaceport's default catalog source.
pub const CINEMETA_URL: &str = "https://v3-cinemeta.strem.io/manifest.json";

/// One catalog of one installed addon, enough for the UI to request it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRef {
    pub transport_url: String,
    pub addon_name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
    /// Genre filter values the catalog offers, in the addon's order.
    pub genres: Vec<String>,
    /// The catalog only answers when a genre is given; the first genre is the default.
    pub genre_required: bool,
    /// The catalog accepts `skip` for paging.
    pub supports_skip: bool,
}

/// A title in a catalog response.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaPreview {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub poster: Option<String>,
    pub poster_shape: Option<String>,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum CatalogError {
    #[error("not an addon manifest URL")]
    BadTransportUrl,
}

fn has_catalog_resource(manifest: &Value) -> bool {
    manifest
        .get("resources")
        .and_then(Value::as_array)
        .is_some_and(|resources| {
            resources.iter().any(|r| {
                r.as_str().or_else(|| r.get("name").and_then(Value::as_str)) == Some("catalog")
            })
        })
}

/// What a catalog accepts as extra properties, from the newer `extra: [{name, isRequired,
/// options}]` form or the older `extraSupported` / `extraRequired` / `genres` fields.
struct Extras {
    genres: Vec<String>,
    genre_required: bool,
    supports_skip: bool,
    /// Some required input other than a genre from a fixed list, such as a search term.
    needs_input: bool,
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn extras(catalog: &Value) -> Extras {
    let extra = catalog.get("extra").and_then(Value::as_array);
    let named = |name: &str| {
        extra
            .into_iter()
            .flatten()
            .find(|e| e.get("name").and_then(Value::as_str) == Some(name))
    };
    let legacy_supported = string_list(catalog.get("extraSupported"));
    let legacy_required = string_list(catalog.get("extraRequired"));

    let mut genres = string_list(named("genre").and_then(|g| g.get("options")));
    if genres.is_empty() {
        genres = string_list(catalog.get("genres"));
    }

    let mut required: Vec<String> = extra
        .into_iter()
        .flatten()
        .filter(|e| e.get("isRequired").and_then(Value::as_bool) == Some(true))
        .filter_map(|e| e.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    required.extend(legacy_required);

    let genre_required = required.iter().any(|r| r == "genre");
    let needs_input = required.iter().any(|r| r != "genre" || genres.is_empty());

    Extras {
        supports_skip: named("skip").is_some() || legacy_supported.iter().any(|s| s == "skip"),
        genres,
        genre_required,
        needs_input,
    }
}

fn catalog_refs(descriptor: &Value) -> Vec<CatalogRef> {
    let (Some(addon), Some(manifest)) = (
        Addon::from_descriptor(descriptor),
        descriptor.get("manifest"),
    ) else {
        return Vec::new();
    };
    if !has_catalog_resource(manifest) {
        return Vec::new();
    }
    let catalogs = manifest.get("catalogs").and_then(Value::as_array);
    catalogs
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let extras = extras(c);
            if extras.needs_input {
                return None;
            }
            let kind = c.get("type")?.as_str()?.to_string();
            let id = c.get("id")?.as_str()?.to_string();
            let name = c
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.trim().is_empty())
                .unwrap_or(&id)
                .to_string();
            Some(CatalogRef {
                transport_url: addon.transport_url.clone(),
                addon_name: addon.name.clone(),
                kind,
                id,
                name,
                genres: extras.genres,
                genre_required: extras.genre_required,
                supports_skip: extras.supports_skip,
            })
        })
        .collect()
}

/// Every catalog of the installed addons that can be browsed without typing anything, in
/// addon order. Home shows those without a required genre; the type pages show all of them.
pub fn browsable_catalogs(collection: &[Value]) -> Vec<CatalogRef> {
    collection.iter().flat_map(catalog_refs).collect()
}

/// Whether the collection already contains an addon with catalogs.
pub fn has_catalog_addon(collection: &[Value]) -> bool {
    !browsable_catalogs(collection).is_empty()
}

/// Percent-encodes everything except RFC 3986 unreserved characters.
fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Builds a resource URL such as `{base}/catalog/movie/top.json` for an addon.
pub fn resource_url(
    transport_url: &str,
    resource: &str,
    kind: &str,
    id: &str,
    extra: &[(String, String)],
) -> Result<String, CatalogError> {
    let mut url = Url::parse(transport_url).map_err(|_| CatalogError::BadTransportUrl)?;
    let base = url
        .path()
        .strip_suffix("/manifest.json")
        .ok_or(CatalogError::BadTransportUrl)?
        .to_string();
    let mut path = format!(
        "{base}/{}/{}/{}",
        encode_component(resource),
        encode_component(kind),
        encode_component(id)
    );
    if !extra.is_empty() {
        let pairs: Vec<String> = extra
            .iter()
            .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
            .collect();
        // The extra segment is a query string inside the path: `genre=Action&skip=100`.
        path.push('/');
        path.push_str(&pairs.join("&"));
    }
    path.push_str(".json");
    url.set_path(&path);
    Ok(url.to_string())
}

/// Reads the `metas` list of a catalog response, skipping entries without an id.
pub fn parse_metas(body: &Value) -> Vec<MetaPreview> {
    let metas = body.get("metas").and_then(Value::as_array);
    metas
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let text = |key: &str| {
                m.get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            let id = text("id")?;
            Some(MetaPreview {
                kind: text("type").unwrap_or_default(),
                name: text("name").unwrap_or_else(|| id.clone()),
                poster: text("poster"),
                poster_shape: text("posterShape"),
                id,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn collection() -> Vec<Value> {
        vec![
            json!({
                "transportUrl": CINEMETA_URL,
                "manifest": {
                    "id": "com.linvo.cinemeta", "name": "Cinemeta", "version": "3",
                    "resources": ["catalog", "meta"], "types": ["movie", "series"],
                    "catalogs": [
                        {"type": "movie", "id": "top", "name": "Popular",
                         "extra": [{"name": "genre", "options": ["Action", "Drama"]},
                                   {"name": "skip"}]},
                        {"type": "movie", "id": "search", "name": "Search",
                         "extra": [{"name": "search", "isRequired": true}]},
                        {"type": "series", "id": "top"}
                    ]
                }
            }),
            json!({
                "transportUrl": "https://streams.example/manifest.json",
                "manifest": {
                    "id": "streams", "name": "Streams", "version": "1",
                    "resources": [{"name": "stream", "types": ["movie"]}], "types": ["movie"],
                    "catalogs": [{"type": "movie", "id": "ignored"}]
                }
            }),
            json!({
                "transportUrl": "https://anime.example/manifest.json",
                "manifest": {
                    "id": "anime", "name": "Anime", "version": "1",
                    "resources": [{"name": "catalog", "types": ["anime"]}], "types": ["anime"],
                    "catalogs": [
                        {"type": "anime", "id": "trending", "name": "Trending"},
                        {"type": "anime", "id": "legacy", "extraRequired": ["search"]},
                        {"type": "anime", "id": "old", "genres": ["Mecha"],
                         "extraSupported": ["genre", "skip"]},
                        {"type": "anime", "id": "by-genre", "extra": [
                            {"name": "genre", "isRequired": true, "options": ["Isekai"]}]},
                        {"type": "anime", "id": "free-genre", "extra": [
                            {"name": "genre", "isRequired": true}]}
                    ]
                }
            }),
        ]
    }

    #[test]
    fn lists_browsable_catalogs_in_addon_order() {
        let rows = browsable_catalogs(&collection());
        let ids: Vec<(&str, &str, &str)> = rows
            .iter()
            .map(|r| (r.addon_name.as_str(), r.kind.as_str(), r.id.as_str()))
            .collect();
        assert_eq!(
            ids,
            [
                ("Cinemeta", "movie", "top"),
                ("Cinemeta", "series", "top"),
                ("Anime", "anime", "trending"),
                ("Anime", "anime", "old"),
                ("Anime", "anime", "by-genre"),
            ]
        );
        // A catalog without a name is labelled with its id.
        assert_eq!(rows[1].name, "top");
    }

    #[test]
    fn reads_genres_and_paging() {
        let rows = browsable_catalogs(&collection());
        let cinemeta = &rows[0];
        assert_eq!(cinemeta.genres, ["Action", "Drama"]);
        assert!(cinemeta.supports_skip && !cinemeta.genre_required);
        let series = &rows[1];
        assert!(series.genres.is_empty() && !series.supports_skip);
        let legacy = &rows[3];
        assert_eq!(legacy.genres, ["Mecha"]);
        assert!(legacy.supports_skip);
        let by_genre = &rows[4];
        assert_eq!(by_genre.genres, ["Isekai"]);
        assert!(by_genre.genre_required && !by_genre.supports_skip);
    }

    #[test]
    fn detects_missing_catalog_addons() {
        assert!(has_catalog_addon(&collection()));
        assert!(!has_catalog_addon(&collection()[1..2]));
    }

    #[test]
    fn builds_catalog_urls() {
        assert_eq!(
            resource_url(CINEMETA_URL, "catalog", "movie", "top", &[]).unwrap(),
            "https://v3-cinemeta.strem.io/catalog/movie/top.json"
        );
        assert_eq!(
            resource_url(
                "https://addon.example/abc%3D/manifest.json?x=1",
                "catalog",
                "series",
                "kitsu:top airing",
                &[
                    ("genre".into(), "Sci-Fi & Fantasy".into()),
                    ("skip".into(), "100".into())
                ]
            )
            .unwrap(),
            "https://addon.example/abc%3D/catalog/series/kitsu%3Atop%20airing/\
             genre=Sci-Fi%20%26%20Fantasy&skip=100.json?x=1"
        );
        assert_eq!(
            resource_url("https://addon.example/", "catalog", "movie", "top", &[]),
            Err(CatalogError::BadTransportUrl)
        );
    }

    #[test]
    fn parses_metas() {
        let metas = parse_metas(&json!({"metas": [
            {"id": "tt1", "type": "movie", "name": "One", "poster": "https://p/1.jpg"},
            {"id": "tt2", "type": "movie", "posterShape": "landscape", "poster": ""},
            {"name": "no id"}
        ]}));
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].poster.as_deref(), Some("https://p/1.jpg"));
        assert_eq!(metas[1].name, "tt2");
        assert_eq!(metas[1].poster, None);
        assert_eq!(metas[1].poster_shape.as_deref(), Some("landscape"));
        assert!(parse_metas(&json!({})).is_empty());
    }
}

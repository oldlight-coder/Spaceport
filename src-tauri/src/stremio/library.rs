//! The user's Stremio library, stored in the `libraryItem` datastore collection.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A library entry as Stremio stores it. Only the fields Spaceport reads are listed.
#[derive(Debug, Clone, Deserialize)]
pub struct RawLibraryItem {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub poster: Option<String>,
    /// Titles the user removed stay in the datastore with this flag set.
    #[serde(default)]
    pub removed: bool,
    /// Titles that were only opened, never added, are kept as temporary entries.
    #[serde(default)]
    pub temp: bool,
    /// Last modification time as an ISO 8601 string.
    #[serde(rename = "_mtime", default)]
    pub mtime: String,
    /// When the title was first stored, as an ISO 8601 string.
    #[serde(rename = "_ctime", default)]
    pub ctime: Option<Value>,
    #[serde(default)]
    pub state: Option<Value>,
}

/// A library title as the UI sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub poster: Option<String>,
    /// When it was added (ISO 8601), for sorting by date added.
    pub added: String,
    /// When it was last watched (ISO 8601), if ever.
    pub last_watched: Option<String>,
}

/// An ISO 8601 time stored as a non-empty string; Stremio sometimes stores null instead.
fn time(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string)
}

/// Keeps the titles that are really in the library, most recently changed first. Entries that
/// cannot be read are skipped.
pub fn visible_items(stored: &[Value]) -> Vec<LibraryItem> {
    let mut raw: Vec<RawLibraryItem> = stored
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    raw.retain(|item| !item.removed && !item.temp);
    // ISO 8601 timestamps in the same format sort correctly as strings.
    raw.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    raw.into_iter()
        .map(|item| LibraryItem {
            id: item.id,
            name: item.name,
            kind: item.kind,
            poster: item.poster.filter(|p| !p.trim().is_empty()),
            added: time(item.ctime.as_ref()).unwrap_or_else(|| item.mtime.clone()),
            last_watched: time(item.state.as_ref().and_then(|s| s.get("lastWatched"))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"_id":"tt0111161","name":"The Shawshank Redemption","type":"movie",
         "poster":"https://images.example/1.jpg","removed":false,"temp":false,
         "_ctime":"2024-01-01T10:00:00.000Z","_mtime":"2024-03-01T10:00:00.000Z",
         "state":{"timeOffset":0,"duration":0}},
        {"_id":"tt0903747","name":"Breaking Bad","type":"series","poster":"",
         "removed":false,"temp":false,"_mtime":"2024-05-01T10:00:00.000Z"},
        {"_id":"tt0068646","name":"The Godfather","type":"movie","poster":null,
         "removed":true,"temp":false,"_mtime":"2024-06-01T10:00:00.000Z"},
        {"_id":"tt1375666","name":"Inception","type":"movie",
         "removed":false,"temp":true,"_mtime":"2024-07-01T10:00:00.000Z"},
        {"_id":"kitsu:1","type":"series"}
    ]"#;

    #[test]
    fn filters_removed_and_temp_and_sorts_newest_first() {
        let raw: Vec<Value> = serde_json::from_str(SAMPLE).unwrap();
        let items = visible_items(&raw);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["tt0903747", "tt0111161", "kitsu:1"]);
    }

    #[test]
    fn reads_added_and_last_watched_times() {
        let raw = serde_json::json!([
            {"_id": "a", "_ctime": "2024-01-01T10:00:00.000Z", "_mtime": "2024-03-01T10:00:00.000Z",
             "state": {"lastWatched": "2024-02-01T10:00:00.000Z"}},
            {"_id": "b", "_mtime": "2024-04-01T10:00:00.000Z", "state": {"lastWatched": null}}
        ]);
        let items = visible_items(raw.as_array().unwrap());
        assert_eq!(items[1].added, "2024-01-01T10:00:00.000Z");
        assert_eq!(
            items[1].last_watched.as_deref(),
            Some("2024-02-01T10:00:00.000Z")
        );
        // Without a creation time the last change stands in.
        assert_eq!(items[0].added, "2024-04-01T10:00:00.000Z");
        assert_eq!(items[0].last_watched, None);
    }

    #[test]
    fn empty_poster_becomes_none() {
        let raw: Vec<Value> = serde_json::from_str(SAMPLE).unwrap();
        let items = visible_items(&raw);
        assert_eq!(items[0].poster, None);
        assert_eq!(
            items[1].poster.as_deref(),
            Some("https://images.example/1.jpg")
        );
    }

    #[test]
    fn missing_fields_use_defaults() {
        let raw: Vec<RawLibraryItem> = serde_json::from_str(SAMPLE).unwrap();
        let last = &raw[4];
        assert_eq!(last.name, "");
        assert!(!last.removed && !last.temp);
    }
}

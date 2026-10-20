//! The user's Stremio library, stored in the `libraryItem` datastore collection.

use serde::{Deserialize, Serialize};

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
}

/// Keeps the titles that are really in the library, most recently changed first.
pub fn visible_items(mut raw: Vec<RawLibraryItem>) -> Vec<LibraryItem> {
    raw.retain(|item| !item.removed && !item.temp);
    // ISO 8601 timestamps in the same format sort correctly as strings.
    raw.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    raw.into_iter()
        .map(|item| LibraryItem {
            id: item.id,
            name: item.name,
            kind: item.kind,
            poster: item.poster.filter(|p| !p.trim().is_empty()),
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
        let raw: Vec<RawLibraryItem> = serde_json::from_str(SAMPLE).unwrap();
        let items = visible_items(raw);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["tt0903747", "tt0111161", "kitsu:1"]);
    }

    #[test]
    fn empty_poster_becomes_none() {
        let raw: Vec<RawLibraryItem> = serde_json::from_str(SAMPLE).unwrap();
        let items = visible_items(raw);
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

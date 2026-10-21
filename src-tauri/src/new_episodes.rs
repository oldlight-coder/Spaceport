//! Which episodes of a series are new to the user.
//!
//! An episode is new when it was released after the user last watched the series, is already
//! out, and is not marked watched. "Last watched" is the library item's `lastWatched`; for a
//! title only marked as watching on this computer it is the time of the mark. Specials
//! (season 0) are left out.

use serde_json::Value;

use crate::progress::parse_iso_millis;
use crate::stremio::meta::MetaDetails;
use crate::watched;

/// The ids of the new episodes, in the addon's order. `since` is the user's last watch, in
/// milliseconds since 1970; without one nothing counts as new, since the whole series would.
pub fn new_episodes(
    meta: &MetaDetails,
    item: Option<&Value>,
    since: Option<i64>,
    now: i64,
) -> Vec<String> {
    let last_watched = item
        .and_then(|i| i.get("state"))
        .and_then(|s| s.get("lastWatched"))
        .and_then(Value::as_str)
        .and_then(parse_iso_millis)
        .filter(|ms| *ms > 0);
    let Some(since) = last_watched.or(since) else {
        return Vec::new();
    };
    let marks = item.map(|i| watched::read_marks(i, &meta.video_ids));
    if marks.as_ref().is_some_and(|m| m.title) {
        return Vec::new();
    }
    let seen = |id: &str| {
        marks
            .as_ref()
            .is_some_and(|m| m.videos.iter().any(|v| v == id))
    };
    meta.seasons
        .iter()
        .filter(|s| s.number > 0)
        .flat_map(|s| &s.episodes)
        .filter(|ep| {
            let released = ep.released.as_deref().and_then(parse_iso_millis);
            released.is_some_and(|r| r > since && r <= now) && !seen(&ep.id)
        })
        .map(|ep| ep.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stremio::meta::parse_meta;
    use serde_json::json;

    fn show() -> MetaDetails {
        parse_meta(&json!({"meta": {"id": "tt1", "type": "series", "name": "Show", "videos": [
            {"id": "tt1:0:1", "season": 0, "episode": 1, "released": "2026-10-10T00:00:00.000Z"},
            {"id": "tt1:1:1", "season": 1, "episode": 1, "released": "2026-09-01T00:00:00.000Z"},
            {"id": "tt1:1:2", "season": 1, "episode": 2, "released": "2026-10-08T00:00:00.000Z"},
            {"id": "tt1:1:3", "season": 1, "episode": 3, "released": "2026-10-15T00:00:00.000Z"},
            {"id": "tt1:1:4", "season": 1, "episode": 4, "released": "2026-10-29T00:00:00.000Z"}
        ]}}))
        .unwrap()
    }

    const NOW: &str = "2026-10-21T12:00:00.000Z";

    fn ms(text: &str) -> i64 {
        parse_iso_millis(text).unwrap()
    }

    #[test]
    fn released_since_the_last_watch_and_already_out() {
        let item = json!({"state": {"lastWatched": "2026-10-05T20:00:00.000Z"}});
        assert_eq!(
            new_episodes(&show(), Some(&item), None, ms(NOW)),
            ["tt1:1:2", "tt1:1:3"]
        );
    }

    #[test]
    fn watched_episodes_are_not_new() {
        let meta = show();
        let mut item = json!({"state": {"lastWatched": "2026-10-05T20:00:00.000Z"}});
        // Marking also stamps the last watch, here the same time as before.
        let at = "2026-10-05T20:00:00.000Z";
        watched::apply_marks(&mut item, &meta.video_ids, &["tt1:1:2".into()], true, at);
        assert_eq!(new_episodes(&meta, Some(&item), None, ms(NOW)), ["tt1:1:3"]);
        item["state"]["flaggedWatched"] = json!(1);
        assert!(new_episodes(&meta, Some(&item), None, ms(NOW)).is_empty());
    }

    #[test]
    fn a_watching_mark_counts_from_when_it_was_made() {
        let since = ms("2026-10-10T00:00:00.000Z");
        assert_eq!(
            new_episodes(&show(), None, Some(since), ms(NOW)),
            ["tt1:1:3"]
        );
        // Stremio's last watch wins over the mark.
        let item = json!({"state": {"lastWatched": "2026-10-16T00:00:00.000Z"}});
        assert!(new_episodes(&show(), Some(&item), Some(since), ms(NOW)).is_empty());
    }

    #[test]
    fn nothing_is_new_without_a_last_watch() {
        let item = json!({"state": {"lastWatched": null}});
        assert!(new_episodes(&show(), Some(&item), None, ms(NOW)).is_empty());
        assert!(new_episodes(&show(), None, None, ms(NOW)).is_empty());
    }
}

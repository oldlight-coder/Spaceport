//! Watch progress: the Stremio library item state (shared with the official apps) and a local
//! per-video store for resuming any episode.
//!
//! Stremio keeps one position per title: the `state` of its library item holds the video being
//! watched (`video_id`), the position (`timeOffset`) and length (`duration`) in milliseconds and
//! when it was last watched. Spaceport writes the same fields, so progress shows up in the
//! official app, and keeps every video's position locally because Stremio only keeps the last.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Positions in the first half minute are not worth resuming.
const MIN_RESUME_SECONDS: f64 = 30.0;
/// Within this share of the end a video counts as finished and starts over next time.
const FINISHED_SHARE: f64 = 0.95;

/// Formats a time as Stremio stores it: ISO 8601 in UTC with milliseconds.
pub fn iso_time(time: SystemTime) -> String {
    let ms = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let (days, rest) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    )
}

/// Reads an ISO 8601 UTC time ("2026-10-21T10:00:00.000Z") as milliseconds since 1970.
pub fn parse_iso_millis(text: &str) -> Option<i64> {
    let text = text.trim().strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>());
    let (year, month, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let (hms, millis) = match time.split_once('.') {
        Some((hms, frac)) => {
            let frac: String = frac.chars().take(3).collect();
            let scale = 10i64.pow(3 - frac.len() as u32);
            (hms, frac.parse::<i64>().ok()? * scale)
        }
        None => (time, 0),
    };
    let mut t = hms.split(':').map(|p| p.parse::<i64>());
    let (h, m, sec) = (t.next()?.ok()?, t.next()?.ok()?, t.next()?.ok()?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days since 1970-01-01 (inverse of the civil date conversion above).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + h) * 60 + m) * 60_000 + sec * 1000 + millis)
}

/// The position to store: a video watched to (nearly) its end is stored as not started, so it
/// leaves Continue Watching and plays from the beginning next time.
pub fn stored_position(position: f64, duration: f64, finished: bool) -> f64 {
    let near_end = duration > 0.0 && position >= duration * FINISHED_SHARE;
    if finished || near_end {
        0.0
    } else {
        position.max(0.0)
    }
}

/// Where to start a video that was stopped at `position` of `duration` seconds, if anywhere.
pub fn resume_point(position: f64, duration: f64) -> Option<f64> {
    if !position.is_finite() || position < MIN_RESUME_SECONDS {
        return None;
    }
    if duration.is_finite() && duration > 0.0 && position >= duration * FINISHED_SHARE {
        return None;
    }
    Some(position)
}

/// The title a progress update belongs to, for creating its library item when it is new.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub poster: Option<String>,
    /// Every video id in the addon's order, for watched marks. Empty for movies.
    #[serde(default)]
    pub video_ids: Vec<String>,
}

/// A library item for a title that is not in the library yet. Like the official apps, it is
/// stored as a temporary, removed entry: it shows in Continue Watching but not in the library.
pub fn new_library_item(title: &TitleInfo, now: &str) -> Value {
    json!({
        "_id": title.id,
        "name": title.name,
        "type": title.kind,
        "poster": title.poster,
        "posterShape": "poster",
        "removed": true,
        "temp": true,
        "_ctime": now,
        "_mtime": now,
        "state": {
            "lastWatched": now,
            "timeWatched": 0,
            "timeOffset": 0,
            "overallTimeWatched": 0,
            "timesWatched": 0,
            "flaggedWatched": 0,
            "duration": 0,
            "video_id": null,
            "watched": null,
            "noNotif": false
        },
        "behaviorHints": {
            "defaultVideoId": null,
            "featuredVideoId": null,
            "hasScheduledVideos": false
        }
    })
}

fn number(state: &Map<String, Value>, key: &str) -> f64 {
    state.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Writes a position into a library item's state. `watched_seconds` is the time actually
/// watched since the last update, added to the watch-time totals. Fields Spaceport does not
/// know are kept as they are.
pub fn apply_progress(
    item: &mut Value,
    video_id: &str,
    position: f64,
    duration: f64,
    watched_seconds: f64,
    now: &str,
) {
    let Some(object) = item.as_object_mut() else {
        return;
    };
    object.insert("_mtime".into(), json!(now));
    let state = object
        .entry("state")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    let Some(state) = state else {
        return;
    };
    let same_video = state.get("video_id").and_then(Value::as_str) == Some(video_id);
    let watched_ms = (watched_seconds.max(0.0) * 1000.0).round();
    let time_watched = if same_video {
        number(state, "timeWatched") + watched_ms
    } else {
        watched_ms
    };
    state.insert("video_id".into(), json!(video_id));
    state.insert(
        "timeOffset".into(),
        json!((position.max(0.0) * 1000.0).round() as u64),
    );
    if duration.is_finite() && duration > 0.0 {
        state.insert("duration".into(), json!((duration * 1000.0).round() as u64));
    }
    state.insert("timeWatched".into(), json!(time_watched as u64));
    let overall = number(state, "overallTimeWatched") + watched_ms;
    state.insert("overallTimeWatched".into(), json!(overall as u64));
    state.insert("lastWatched".into(), json!(now));
}

/// A title in the Continue Watching row.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueItem {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub poster: Option<String>,
    pub video_id: Option<String>,
    /// Seconds.
    pub position: f64,
    pub duration: f64,
    pub last_watched: String,
}

/// The titles Stremio's Continue Watching shows: started, not of type "other", and either in
/// the library or a temporary entry; most recently watched first.
pub fn continue_watching(items: &[Value]) -> Vec<ContinueItem> {
    let mut list: Vec<ContinueItem> = items
        .iter()
        .filter_map(|item| {
            let text = |key: &str| item.get(key).and_then(Value::as_str).map(str::to_string);
            let flag = |key: &str| item.get(key).and_then(Value::as_bool).unwrap_or(false);
            let state = item.get("state")?.as_object()?;
            let kind = text("type").unwrap_or_default();
            let offset = number(state, "timeOffset");
            let shown = kind != "other" && (!flag("removed") || flag("temp")) && offset > 0.0;
            if !shown {
                return None;
            }
            let id = text("_id")?;
            Some(ContinueItem {
                name: text("name").unwrap_or_else(|| id.clone()),
                poster: text("poster").filter(|p| !p.trim().is_empty()),
                video_id: state
                    .get("video_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                position: offset / 1000.0,
                duration: number(state, "duration") / 1000.0,
                last_watched: state
                    .get("lastWatched")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                kind,
                id,
            })
        })
        .collect();
    list.sort_by(|a, b| b.last_watched.cmp(&a.last_watched));
    list
}

/// One video's position as kept on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoProgress {
    pub position: f64,
    pub duration: f64,
    /// Milliseconds since 1970, to keep the newest when merging with Stremio's copy.
    pub updated: i64,
}

/// Positions per video id ("tt0903747:1:2", or the title id for a movie), stored as JSON in the
/// app's config folder.
pub struct ResumeStore {
    path: PathBuf,
}

impl ResumeStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("resume.json"),
        }
    }

    pub fn load(&self) -> HashMap<String, VideoProgress> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, all: &HashMap<String, VideoProgress>) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec(all)?)?;
        fs::rename(&tmp, &self.path)
    }

    pub fn update(&self, video_id: &str, progress: VideoProgress) -> std::io::Result<()> {
        let mut all = self.load();
        all.insert(video_id.to_string(), progress);
        self.save(&all)
    }

    /// Positions of every video of one title (ids equal to it or starting with "it:").
    pub fn for_title(&self, title_id: &str) -> HashMap<String, VideoProgress> {
        let prefix = format!("{title_id}:");
        self.load()
            .into_iter()
            .filter(|(id, _)| id == title_id || id.starts_with(&prefix))
            .collect()
    }
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_iso_times() {
        assert_eq!(iso_time(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        let t = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        assert_eq!(iso_time(t), "2023-11-14T22:13:20.123Z");
        // A leap day.
        let t = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(iso_time(t), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn parses_iso_times_back() {
        for ms in [0i64, 1_700_000_000_123, 951_782_400_000, 1_792_576_000_500] {
            let text = iso_time(UNIX_EPOCH + Duration::from_millis(ms as u64));
            assert_eq!(parse_iso_millis(&text), Some(ms), "{text}");
        }
        assert_eq!(
            parse_iso_millis("2026-10-21T10:00:00Z"),
            Some(1_792_576_800_000)
        );
        assert_eq!(
            parse_iso_millis("2026-10-21T10:00:00.5Z"),
            Some(1_792_576_800_500)
        );
        assert_eq!(parse_iso_millis("yesterday"), None);
        assert_eq!(parse_iso_millis("2026-13-01T00:00:00Z"), None);
    }

    #[test]
    fn finished_videos_are_stored_as_not_started() {
        assert_eq!(stored_position(1200.0, 3000.0, false), 1200.0);
        assert_eq!(stored_position(2900.0, 3000.0, false), 0.0);
        assert_eq!(stored_position(1200.0, 3000.0, true), 0.0);
    }

    #[test]
    fn resume_rules() {
        assert_eq!(resume_point(10.0, 3000.0), None);
        assert_eq!(resume_point(1234.0, 3000.0), Some(1234.0));
        assert_eq!(resume_point(2900.0, 3000.0), None);
        assert_eq!(resume_point(500.0, 0.0), Some(500.0));
        assert_eq!(resume_point(f64::NAN, 3000.0), None);
    }

    fn title() -> TitleInfo {
        TitleInfo {
            id: "tt0903747".into(),
            kind: "series".into(),
            name: "Breaking Bad".into(),
            poster: Some("https://p/1.jpg".into()),
            video_ids: vec![],
        }
    }

    #[test]
    fn new_items_are_temporary() {
        let item = new_library_item(&title(), "2026-10-21T10:00:00.000Z");
        assert_eq!(item["removed"], json!(true));
        assert_eq!(item["temp"], json!(true));
        assert_eq!(item["state"]["timeOffset"], json!(0));
    }

    #[test]
    fn progress_is_written_in_milliseconds() {
        let mut item = new_library_item(&title(), "t0");
        apply_progress(&mut item, "tt0903747:1:2", 754.4, 2890.0, 60.0, "t1");
        let state = &item["state"];
        assert_eq!(state["video_id"], json!("tt0903747:1:2"));
        assert_eq!(state["timeOffset"], json!(754_400));
        assert_eq!(state["duration"], json!(2_890_000));
        assert_eq!(state["timeWatched"], json!(60_000));
        assert_eq!(state["overallTimeWatched"], json!(60_000));
        assert_eq!(state["lastWatched"], json!("t1"));
        assert_eq!(item["_mtime"], json!("t1"));

        // The same video adds up; a new video restarts its own watch time.
        apply_progress(&mut item, "tt0903747:1:2", 814.4, 2890.0, 60.0, "t2");
        assert_eq!(item["state"]["timeWatched"], json!(120_000));
        apply_progress(&mut item, "tt0903747:1:3", 30.0, 2700.0, 30.0, "t3");
        assert_eq!(item["state"]["timeWatched"], json!(30_000));
        assert_eq!(item["state"]["overallTimeWatched"], json!(150_000));
    }

    #[test]
    fn keeps_unknown_fields() {
        let mut item =
            json!({"_id": "tt1", "state": {"watched": "abc", "noNotif": true}, "extra": 1});
        apply_progress(&mut item, "tt1", 100.0, 200.0, 5.0, "now");
        assert_eq!(item["extra"], json!(1));
        assert_eq!(item["state"]["watched"], json!("abc"));
        assert_eq!(item["state"]["noNotif"], json!(true));
    }

    #[test]
    fn continue_watching_matches_stremio_rules() {
        let items = vec![
            json!({"_id": "a", "type": "movie", "name": "A", "removed": false, "temp": false,
                   "state": {"timeOffset": 600000, "duration": 6000000, "lastWatched": "2026-10-01T00:00:00.000Z", "video_id": "a"}}),
            json!({"_id": "b", "type": "series", "name": "B", "removed": true, "temp": true,
                   "state": {"timeOffset": 1000, "lastWatched": "2026-10-05T00:00:00.000Z", "video_id": "b:1:1"}}),
            json!({"_id": "c", "type": "movie", "name": "Removed", "removed": true, "temp": false,
                   "state": {"timeOffset": 5000, "lastWatched": "2026-10-09T00:00:00.000Z"}}),
            json!({"_id": "d", "type": "movie", "name": "Unstarted", "removed": false,
                   "state": {"timeOffset": 0, "lastWatched": "2026-10-09T00:00:00.000Z"}}),
            json!({"_id": "e", "type": "other", "name": "Other", "removed": false,
                   "state": {"timeOffset": 5000}}),
        ];
        let list = continue_watching(&items);
        let ids: Vec<&str> = list.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(list[1].position, 600.0);
        assert_eq!(list[1].duration, 6000.0);
        assert_eq!(list[0].video_id.as_deref(), Some("b:1:1"));
    }

    #[test]
    fn local_store_per_title() {
        let dir = std::env::temp_dir().join(format!("spaceport-resume-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = ResumeStore::new(&dir);
        let p = |position| VideoProgress {
            position,
            duration: 3000.0,
            updated: 1,
        };
        store.update("tt1:1:1", p(100.0)).unwrap();
        store.update("tt1:1:2", p(200.0)).unwrap();
        store.update("tt10:1:1", p(300.0)).unwrap();
        store.update("tt1", p(400.0)).unwrap();
        let mine = store.for_title("tt1");
        assert_eq!(mine.len(), 3);
        assert_eq!(mine["tt1:1:2"].position, 200.0);
        let _ = fs::remove_dir_all(&dir);
    }
}

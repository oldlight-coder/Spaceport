//! Watched marks as Stremio keeps them in a library item's `state`.
//!
//! Movies use `flaggedWatched` (0 or 1). For series, `state.watched` records one bit per video
//! in the order the metadata lists them, written as `<anchor video id>:<anchor length>:<bits>`:
//! `<bits>` is the zlib-compressed bit array in base64 (bit `i` is bit `i % 8` of byte `i / 8`),
//! and the anchor is the last watched video and its 1-based position. When videos are added or
//! removed, positions are shifted so the anchor video keeps its bit.

use std::io::{Read, Write};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde_json::{json, Value};

/// Reads `state.watched` for the given videos (in metadata order). Unreadable data counts as
/// nothing watched.
pub fn decode(serialized: &str, video_ids: &[String]) -> Vec<bool> {
    let mut bits = vec![false; video_ids.len()];
    let mut parts = serialized.rsplitn(3, ':');
    let (Some(encoded), Some(anchor_len), Some(anchor_id)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return bits;
    };
    let Ok(anchor_len) = anchor_len.parse::<usize>() else {
        return bits;
    };
    let Ok(compressed) = STANDARD.decode(encoded) else {
        return bits;
    };
    let mut bytes = Vec::new();
    if ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut bytes)
        .is_err()
    {
        return bits;
    }
    let old = |i: usize| bytes.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 == 1);
    // Where the anchor video sits now compared with when the bits were written.
    let shift: isize = match video_ids.iter().position(|id| id == anchor_id) {
        Some(index) if anchor_len > 0 => index as isize - (anchor_len as isize - 1),
        _ => 0,
    };
    for (j, bit) in bits.iter_mut().enumerate() {
        let i = j as isize - shift;
        if i >= 0 {
            *bit = old(i as usize);
        }
    }
    bits
}

/// Writes the watched bits for the given videos. `None` when there are no videos.
pub fn encode(video_ids: &[String], bits: &[bool]) -> Option<String> {
    if video_ids.is_empty() {
        return None;
    }
    let mut bytes = vec![0u8; video_ids.len().div_ceil(8)];
    for (i, _) in bits
        .iter()
        .enumerate()
        .filter(|(_, b)| **b)
        .take(video_ids.len())
    {
        bytes[i / 8] |= 1 << (i % 8);
    }
    let anchor = bits
        .iter()
        .take(video_ids.len())
        .rposition(|b| *b)
        .unwrap_or(video_ids.len() - 1);
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&bytes).ok()?;
    let compressed = encoder.finish().ok()?;
    Some(format!(
        "{}:{}:{}",
        video_ids[anchor],
        anchor + 1,
        STANDARD.encode(compressed)
    ))
}

/// What is marked watched in a library item.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedMarks {
    /// The whole title is marked watched (`flaggedWatched`).
    pub title: bool,
    /// Watched video ids, for series.
    pub videos: Vec<String>,
}

fn state(item: &Value) -> Option<&serde_json::Map<String, Value>> {
    item.get("state")?.as_object()
}

pub fn read_marks(item: &Value, video_ids: &[String]) -> WatchedMarks {
    let state = state(item);
    let title = state
        .and_then(|s| s.get("flaggedWatched"))
        .and_then(Value::as_i64)
        .is_some_and(|f| f > 0);
    let bits = state
        .and_then(|s| s.get("watched"))
        .and_then(Value::as_str)
        .map(|w| decode(w, video_ids))
        .unwrap_or_else(|| vec![false; video_ids.len()]);
    WatchedMarks {
        title,
        videos: video_ids
            .iter()
            .zip(bits)
            .filter(|(_, b)| *b)
            .map(|(id, _)| id.clone())
            .collect(),
    }
}

/// Marks videos (or, with `videos` empty, the whole title) watched or unwatched in a library
/// item. Marking the whole title applies to every video too, and marking a video watched that
/// was in progress clears its position so it leaves Continue Watching.
pub fn apply_marks(
    item: &mut Value,
    video_ids: &[String],
    videos: &[String],
    watched: bool,
    now: &str,
) {
    let Some(object) = item.as_object_mut() else {
        return;
    };
    object.insert("_mtime".into(), json!(now));
    let Some(state) = object
        .entry("state")
        .or_insert_with(|| json!({}))
        .as_object_mut()
    else {
        return;
    };
    let whole_title = videos.is_empty();
    let mut bits = state
        .get("watched")
        .and_then(Value::as_str)
        .map(|w| decode(w, video_ids))
        .unwrap_or_else(|| vec![false; video_ids.len()]);
    for (id, bit) in video_ids.iter().zip(bits.iter_mut()) {
        if whole_title || videos.contains(id) {
            *bit = watched;
        }
    }
    if let Some(encoded) = encode(video_ids, &bits) {
        state.insert("watched".into(), json!(encoded));
    }
    if whole_title {
        state.insert("flaggedWatched".into(), json!(i32::from(watched)));
        let times = state
            .get("timesWatched")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let times = if watched { times.max(1) } else { 0 };
        state.insert("timesWatched".into(), json!(times));
    }
    let current = state
        .get("video_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let touches_current = whole_title
        || current
            .as_deref()
            .is_some_and(|c| videos.iter().any(|v| v == c));
    if watched && touches_current {
        state.insert("timeOffset".into(), json!(0));
    }
    if watched {
        state.insert("lastWatched".into(), json!(now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("tt1:1:{i}")).collect()
    }

    #[test]
    fn round_trips() {
        let videos = ids(12);
        let mut bits = vec![false; 12];
        bits[0] = true;
        bits[3] = true;
        bits[9] = true;
        let encoded = encode(&videos, &bits).unwrap();
        // The anchor is the last watched video, at its 1-based position.
        assert!(encoded.starts_with("tt1:1:10:10:"), "{encoded}");
        assert_eq!(decode(&encoded, &videos), bits);
    }

    #[test]
    fn nothing_watched_anchors_on_the_last_video() {
        let videos = ids(3);
        let encoded = encode(&videos, &[false, false, false]).unwrap();
        assert!(encoded.starts_with("tt1:1:3:3:"));
        assert_eq!(decode(&encoded, &videos), [false, false, false]);
        assert_eq!(encode(&[], &[]), None);
    }

    #[test]
    fn shifts_when_videos_are_added_before() {
        let old = ids(5);
        let mut bits = vec![false; 5];
        bits[1] = true;
        bits[4] = true;
        let encoded = encode(&old, &bits).unwrap();
        // Two specials were listed before the episodes since the marks were written.
        let mut new = vec!["tt1:0:1".to_string(), "tt1:0:2".to_string()];
        new.extend(old.clone());
        new.push("tt1:1:6".into());
        let decoded = decode(&encoded, &new);
        assert_eq!(
            decoded,
            [false, false, false, true, false, false, true, false]
        );
    }

    #[test]
    fn garbage_means_nothing_watched() {
        let videos = ids(3);
        assert_eq!(decode("nonsense", &videos), [false; 3]);
        assert_eq!(decode("tt1:1:1:x:!!!", &videos), [false; 3]);
        assert_eq!(decode("tt1:1:1:1:AAAA", &videos), [false; 3]);
    }

    #[test]
    fn marks_episodes_and_titles() {
        let videos = ids(4);
        let mut item =
            json!({"_id": "tt1", "state": {"video_id": "tt1:1:2", "timeOffset": 500000}});
        apply_marks(
            &mut item,
            &videos,
            &["tt1:1:1".into(), "tt1:1:2".into()],
            true,
            "now",
        );
        let marks = read_marks(&item, &videos);
        assert_eq!(marks.videos, ["tt1:1:1", "tt1:1:2"]);
        assert!(!marks.title);
        // The episode in progress was marked watched, so its position is cleared.
        assert_eq!(item["state"]["timeOffset"], json!(0));

        apply_marks(&mut item, &videos, &["tt1:1:1".into()], false, "later");
        assert_eq!(read_marks(&item, &videos).videos, ["tt1:1:2"]);

        apply_marks(&mut item, &videos, &[], true, "end");
        let marks = read_marks(&item, &videos);
        assert!(marks.title);
        assert_eq!(marks.videos.len(), 4);
        assert_eq!(item["state"]["timesWatched"], json!(1));

        apply_marks(&mut item, &videos, &[], false, "reset");
        let marks = read_marks(&item, &videos);
        assert!(!marks.title && marks.videos.is_empty());
        assert_eq!(item["state"]["timesWatched"], json!(0));
    }

    #[test]
    fn movies_without_videos_use_the_flag() {
        let mut item = json!({"_id": "tt2", "state": {}});
        apply_marks(&mut item, &[], &[], true, "now");
        assert_eq!(item["state"]["flaggedWatched"], json!(1));
        assert!(item["state"].get("watched").is_none());
        assert!(read_marks(&item, &[]).title);
    }
}

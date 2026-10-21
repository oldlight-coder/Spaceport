//! User filters that hide streams outright, as opposed to scoring that only reorders them.

use serde::{Deserialize, Serialize};

use crate::stremio::streams::Stream;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StreamFilters {
    /// Largest file to offer, in bytes. `None` allows any size.
    pub max_size: Option<u64>,
    /// Lowest vertical resolution to offer, e.g. 1080. `None` allows any.
    pub min_resolution: Option<u16>,
    /// Codecs to hide, using the parser's names ("AV1", "HEVC", "AVC", "VP9", "XviD").
    pub blocked_codecs: Vec<String>,
}

/// Why a stream was hidden, for the "hidden by your filters" list.
pub fn rejection(stream: &Stream, filters: &StreamFilters) -> Option<String> {
    let parsed = &stream.parsed;
    // Values the addon did not give are not held against the stream: it is kept and the
    // scoring decides how far down it goes.
    if let (Some(max), Some(size)) = (filters.max_size, parsed.size_bytes) {
        if size > max {
            return Some("Larger than your maximum size".to_string());
        }
    }
    if let (Some(min), Some(resolution)) = (filters.min_resolution, parsed.resolution) {
        if resolution < min {
            return Some(format!("Below {min}p"));
        }
    }
    if let Some(codec) = &parsed.codec {
        if filters
            .blocked_codecs
            .iter()
            .any(|b| b.eq_ignore_ascii_case(codec))
        {
            return Some(format!("{codec} is blocked"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ranking::parse::parse;

    fn stream(title: &str) -> Stream {
        Stream {
            addon_name: "A".into(),
            transport_url: "u".into(),
            name: None,
            title: Some(title.into()),
            url: Some("https://x".into()),
            info_hash: None,
            file_idx: None,
            sources: vec![],
            yt_id: None,
            external_url: None,
            filename: None,
            video_size: None,
            binge_group: None,
            not_web_ready: false,
            parsed: parse(None, Some(title), None, None),
        }
    }

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn no_filters_keep_everything() {
        let f = StreamFilters::default();
        assert_eq!(rejection(&stream("Movie 480p XviD 💾 40 GB"), &f), None);
    }

    #[test]
    fn max_size() {
        let f = StreamFilters {
            max_size: Some(10 * GIB),
            ..Default::default()
        };
        assert!(rejection(&stream("Movie 2160p 💾 25 GB"), &f).is_some());
        assert_eq!(rejection(&stream("Movie 1080p 💾 4 GB"), &f), None);
        // Unknown size is kept.
        assert_eq!(rejection(&stream("Movie 1080p"), &f), None);
    }

    #[test]
    fn min_resolution() {
        let f = StreamFilters {
            min_resolution: Some(1080),
            ..Default::default()
        };
        assert_eq!(
            rejection(&stream("Show 720p HDTV"), &f).as_deref(),
            Some("Below 1080p")
        );
        assert_eq!(rejection(&stream("Show 1080p WEB-DL"), &f), None);
        assert_eq!(rejection(&stream("Show 2160p WEB-DL"), &f), None);
        assert_eq!(rejection(&stream("Show WEB-DL"), &f), None);
    }

    #[test]
    fn blocked_codecs() {
        let f = StreamFilters {
            blocked_codecs: vec!["av1".into(), "HEVC".into()],
            ..Default::default()
        };
        assert_eq!(
            rejection(&stream("Show 1080p x265"), &f).as_deref(),
            Some("HEVC is blocked")
        );
        assert!(rejection(&stream("Show 1080p AV1"), &f).is_some());
        assert_eq!(rejection(&stream("Show 1080p x264"), &f), None);
    }
}

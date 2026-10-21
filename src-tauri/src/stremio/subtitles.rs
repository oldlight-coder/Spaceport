//! Subtitles from the addon protocol's `subtitles` resource
//! (`{base}/subtitles/{type}/{videoId}.json`).

use std::time::Duration;

use futures_util::future::join_all;
use serde::Serialize;
use serde_json::Value;

use super::api::StremioApi;
use super::catalog::resource_url;
use crate::language;

/// How long each subtitle addon may take.
pub const SUBTITLES_TIMEOUT: Duration = Duration::from_secs(10);

/// One subtitle file offered by an addon.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonSubtitle {
    /// Unique within one title: the addon's URL plus the subtitle's id or URL.
    pub key: String,
    pub url: String,
    /// The language as the addon gave it, usually ISO 639-2 ("eng").
    pub lang: String,
    /// The language as ISO 639-1 when known, for matching the user's preferences.
    pub lang_code: Option<String>,
    pub label: String,
    pub addon_name: String,
}

/// Reads the `subtitles` list, skipping entries without a usable web link.
pub fn parse_subtitles(body: &Value, addon_name: &str, transport_url: &str) -> Vec<AddonSubtitle> {
    let list = body.get("subtitles").and_then(Value::as_array);
    list.into_iter()
        .flatten()
        .filter_map(|s| {
            let url = s.get("url")?.as_str()?.trim();
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return None;
            }
            let lang = s
                .get("lang")
                .and_then(Value::as_str)
                .unwrap_or("und")
                .trim()
                .to_string();
            let id = s.get("id").and_then(Value::as_str).unwrap_or(url);
            Some(AddonSubtitle {
                key: format!("{transport_url}#{id}"),
                url: url.to_string(),
                lang_code: language::normalize(&lang).map(str::to_string),
                label: language::name(&lang),
                lang,
                addon_name: addon_name.to_string(),
            })
        })
        .collect()
}

/// Puts subtitles in the preferred languages first, in preference order, and numbers repeats of
/// the same language and addon so they can be told apart.
pub fn order_and_label(mut subs: Vec<AddonSubtitle>, preferred: &[String]) -> Vec<AddonSubtitle> {
    let rank = |s: &AddonSubtitle| {
        s.lang_code
            .as_deref()
            .and_then(|c| preferred.iter().position(|p| p == c))
            .unwrap_or(preferred.len())
    };
    subs.sort_by_key(|s| (rank(s), s.label.clone()));
    let mut seen: Vec<(String, String, usize)> = Vec::new();
    for sub in &mut subs {
        let count = match seen
            .iter_mut()
            .find(|(l, a, _)| *l == sub.label && *a == sub.addon_name)
        {
            Some(entry) => {
                entry.2 += 1;
                entry.2
            }
            None => {
                seen.push((sub.label.clone(), sub.addon_name.clone(), 1));
                1
            }
        };
        if count > 1 {
            sub.label = format!("{} {count}", sub.label);
        }
    }
    subs
}

/// Asks every subtitle addon for one video at once; failing addons are left out.
pub async fn fetch_subtitles(
    api: &StremioApi,
    targets: Vec<(String, String)>,
    kind: &str,
    video_id: &str,
) -> Vec<AddonSubtitle> {
    let requests = targets.into_iter().map(|(name, transport_url)| async move {
        let Ok(url) = resource_url(&transport_url, "subtitles", kind, video_id, &[]) else {
            return Vec::new();
        };
        match api.addon_json(&url, SUBTITLES_TIMEOUT).await {
            Ok(body) => parse_subtitles(&body, &name, &transport_url),
            Err(_) => Vec::new(),
        }
    });
    join_all(requests).await.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Vec<AddonSubtitle> {
        parse_subtitles(
            &json!({"subtitles": [
                {"id": "1", "url": "https://subs.example/1.srt", "lang": "eng"},
                {"id": "2", "url": "https://subs.example/2.srt", "lang": "ger"},
                {"id": "3", "url": "https://subs.example/3.srt", "lang": "eng"},
                {"url": "https://subs.example/4.srt", "lang": "pob"},
                {"id": "5", "url": "file:///etc/passwd", "lang": "eng"},
                {"id": "6", "lang": "eng"}
            ]}),
            "OpenSubtitles",
            "https://opensubtitles.example/manifest.json",
        )
    }

    #[test]
    fn parses_web_subtitles_only() {
        let subs = sample();
        assert_eq!(subs.len(), 4);
        assert_eq!(subs[0].label, "English");
        assert_eq!(subs[0].lang_code.as_deref(), Some("en"));
        assert_eq!(subs[1].lang_code.as_deref(), Some("de"));
        // Unknown codes keep the addon's text; entries without an id are keyed by URL.
        assert_eq!(subs[3].label, "POB");
        assert!(subs[3].key.ends_with("#https://subs.example/4.srt"));
    }

    #[test]
    fn preferred_languages_first_and_repeats_numbered() {
        let ordered = order_and_label(sample(), &["de".into(), "en".into()]);
        let labels: Vec<&str> = ordered.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["German", "English", "English 2", "POB"]);
    }

    #[test]
    fn missing_list_is_empty() {
        assert!(parse_subtitles(&json!({}), "A", "u").is_empty());
    }
}

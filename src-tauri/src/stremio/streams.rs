//! Streams from the addon protocol's `stream` resource (`{base}/stream/{type}/{videoId}.json`).

use std::time::Duration;

use futures_util::future::join_all;
use serde::Serialize;
use serde_json::Value;

use super::api::StremioApi;
use super::catalog::resource_url;

/// Time limit for each addon's stream answer. Addons that take longer are reported as timed out
/// and the others are still shown.
pub const STREAM_TIMEOUT: Duration = Duration::from_secs(15);

/// One playable source offered by an addon.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    pub addon_name: String,
    pub transport_url: String,
    /// Short label, often the addon or provider plus a quality tag.
    pub name: Option<String>,
    /// Longer text, usually the release name, size and seeders.
    pub title: Option<String>,
    /// Direct HTTP(S) link, including debrid links.
    pub url: Option<String>,
    /// Torrent info hash for the built-in torrent engine.
    pub info_hash: Option<String>,
    pub file_idx: Option<u32>,
    /// Extra trackers or DHT nodes for a torrent (`sources`).
    pub sources: Vec<String>,
    pub yt_id: Option<String>,
    /// A web page to open in the browser rather than play.
    pub external_url: Option<String>,
    pub filename: Option<String>,
    pub video_size: Option<u64>,
    pub binge_group: Option<String>,
    pub not_web_ready: bool,
}

/// How one addon answered a stream request.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonOutcome {
    pub addon_name: String,
    pub transport_url: String,
    pub count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamResults {
    /// Every stream from every addon, in addon order and then in each addon's own order.
    pub streams: Vec<Stream>,
    pub addons: Vec<AddonOutcome>,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn parse_stream(value: &Value, addon_name: &str, transport_url: &str) -> Option<Stream> {
    let hints = value.get("behaviorHints");
    let hint_text = |key: &str| hints.and_then(|h| text(h, key));
    let url = text(value, "url");
    let info_hash = text(value, "infoHash").map(|h| h.to_ascii_lowercase());
    let yt_id = text(value, "ytId");
    let external_url = text(value, "externalUrl");
    if url.is_none() && info_hash.is_none() && yt_id.is_none() && external_url.is_none() {
        return None;
    }
    Some(Stream {
        addon_name: addon_name.to_string(),
        transport_url: transport_url.to_string(),
        name: text(value, "name"),
        // `description` is the newer name for `title`.
        title: text(value, "description").or_else(|| text(value, "title")),
        url,
        info_hash,
        file_idx: value
            .get("fileIdx")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        sources: value
            .get("sources")
            .and_then(Value::as_array)
            .map(|s| {
                s.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        yt_id,
        external_url,
        filename: hint_text("filename"),
        video_size: hints
            .and_then(|h| h.get("videoSize"))
            .and_then(Value::as_u64),
        binge_group: hint_text("bingeGroup"),
        not_web_ready: hints
            .and_then(|h| h.get("notWebReady"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Reads the `streams` list of a stream response, skipping entries with nothing to play.
pub fn parse_streams(body: &Value, addon_name: &str, transport_url: &str) -> Vec<Stream> {
    body.get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| parse_stream(s, addon_name, transport_url))
        .collect()
}

/// Combines per-addon answers, kept in the order the addons were asked.
pub fn collect(answers: Vec<(String, String, Result<Vec<Stream>, String>)>) -> StreamResults {
    let mut streams = Vec::new();
    let mut addons = Vec::new();
    for (addon_name, transport_url, answer) in answers {
        let (count, error) = match answer {
            Ok(list) => {
                let count = list.len();
                streams.extend(list);
                (count, None)
            }
            Err(e) => (0, Some(e)),
        };
        addons.push(AddonOutcome {
            addon_name,
            transport_url,
            count,
            error,
        });
    }
    StreamResults { streams, addons }
}

/// Asks every addon in `targets` (name, transport URL) for streams of one video at the same time,
/// each with its own `timeout`.
pub async fn fetch_streams(
    api: &StremioApi,
    targets: Vec<(String, String)>,
    kind: &str,
    video_id: &str,
    timeout: Duration,
) -> StreamResults {
    let requests = targets.into_iter().map(|(name, transport_url)| async move {
        let answer = match resource_url(&transport_url, "stream", kind, video_id, &[]) {
            Ok(url) => api
                .addon_json(&url, timeout)
                .await
                .map(|body| parse_streams(&body, &name, &transport_url))
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        (name, transport_url, answer)
    });
    collect(join_all(requests).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Instant;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// An addon server that answers every request with `body` after `delay`.
    async fn addon(body: &'static str, delay: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let _ = socket.read(&mut buf).await;
                    tokio::time::sleep(delay).await;
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                });
            }
        });
        format!("http://{addr}/manifest.json")
    }

    #[tokio::test]
    async fn slow_addon_times_out_without_blocking_others() {
        let fast = addon(r#"{"streams":[{"url":"https://x/fast"}]}"#, Duration::ZERO).await;
        let slow = addon(
            r#"{"streams":[{"url":"https://x/slow"}]}"#,
            Duration::from_secs(5),
        )
        .await;
        let also_fast = addon(
            r#"{"streams":[{"url":"https://x/b"}]}"#,
            Duration::from_millis(100),
        )
        .await;
        let api = StremioApi::new("http://127.0.0.1:9");
        let started = Instant::now();
        let results = fetch_streams(
            &api,
            vec![
                ("Fast".into(), fast),
                ("Slow".into(), slow),
                ("Also fast".into(), also_fast),
            ],
            "movie",
            "tt1",
            Duration::from_millis(500),
        )
        .await;
        // All three ran at once: the total is about one timeout, not the sum of the delays.
        assert!(started.elapsed() < Duration::from_secs(2));
        let urls: Vec<&str> = results
            .streams
            .iter()
            .filter_map(|s| s.url.as_deref())
            .collect();
        assert_eq!(urls, ["https://x/fast", "https://x/b"]);
        assert!(results.addons[1].error.is_some());
        assert_eq!(results.addons[1].count, 0);
    }

    #[test]
    fn parses_stream_kinds() {
        let body = json!({"streams": [
            {"name": "Torrentio\n4k", "title": "Movie.2024.2160p.WEB-DL\n👤 120 💾 12.3 GB",
             "infoHash": "ABCDEF0123", "fileIdx": 2,
             "sources": ["tracker:udp://t.example:80", "dht:ABCDEF0123"],
             "behaviorHints": {"bingeGroup": "torrentio|4k", "filename": "Movie.mkv",
                               "videoSize": 13207024435u64}},
            {"name": "RD+", "description": "Movie 1080p", "url": "https://debrid.example/dl/1",
             "behaviorHints": {"notWebReady": true}},
            {"ytId": "dQw4w9WgXcQ", "title": "Trailer"},
            {"externalUrl": "https://example.org/watch", "name": "Web"},
            {"name": "nothing to play"}
        ]});
        let streams = parse_streams(&body, "Addon", "https://a/manifest.json");
        assert_eq!(streams.len(), 4);

        let torrent = &streams[0];
        assert_eq!(torrent.info_hash.as_deref(), Some("abcdef0123"));
        assert_eq!(torrent.file_idx, Some(2));
        assert_eq!(torrent.sources.len(), 2);
        assert_eq!(torrent.video_size, Some(13207024435));
        assert_eq!(torrent.filename.as_deref(), Some("Movie.mkv"));
        assert_eq!(torrent.binge_group.as_deref(), Some("torrentio|4k"));
        assert_eq!(torrent.addon_name, "Addon");

        let debrid = &streams[1];
        assert_eq!(debrid.title.as_deref(), Some("Movie 1080p"));
        assert!(debrid.not_web_ready);
        assert_eq!(streams[2].yt_id.as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(
            streams[3].external_url.as_deref(),
            Some("https://example.org/watch")
        );
    }

    #[test]
    fn handles_missing_list() {
        assert!(parse_streams(&json!({}), "A", "u").is_empty());
        assert!(parse_streams(&json!({"streams": null}), "A", "u").is_empty());
    }

    #[test]
    fn collects_in_addon_order_with_errors() {
        let one = parse_streams(&json!({"streams": [{"url": "https://x/1"}]}), "One", "u1");
        let three = parse_streams(
            &json!({"streams": [{"url": "https://x/3a"}, {"url": "https://x/3b"}]}),
            "Three",
            "u3",
        );
        let results = collect(vec![
            ("One".into(), "u1".into(), Ok(one)),
            ("Two".into(), "u2".into(), Err("timed out".into())),
            ("Three".into(), "u3".into(), Ok(three)),
        ]);
        let urls: Vec<&str> = results
            .streams
            .iter()
            .filter_map(|s| s.url.as_deref())
            .collect();
        assert_eq!(urls, ["https://x/1", "https://x/3a", "https://x/3b"]);
        assert_eq!(results.addons[1].error.as_deref(), Some("timed out"));
        assert_eq!(results.addons[2].count, 2);
    }
}

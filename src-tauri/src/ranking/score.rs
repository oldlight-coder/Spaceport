//! Scores streams so the best one can be played automatically.
//!
//! Every rule adds or subtracts points and records why, so the picker can show the reasons.
//! Higher totals rank first. The weights live in [`ScoringRules`] so they can be tuned without
//! touching the logic.

use serde::{Deserialize, Serialize};

use super::parse::ParsedStream;
use crate::stremio::streams::Stream;

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScoringRules {
    /// Highest resolution worth preferring; anything above scores the same as this.
    pub preferred_resolution: u16,
    /// Points per resolution step (2160 = 4 steps, 1440 = 3.5, 1080 = 3, 720 = 2, SD = 1).
    pub resolution_points: f64,
    pub hdr_points: f64,
    pub dolby_vision_points: f64,
    /// Points a debrid service's cached copy adds: it starts instantly.
    pub cached_points: f64,
    /// Points an uncached debrid stream loses: the service must download it first.
    pub uncached_penalty: f64,
    /// Points per tenfold seeders for torrents (10 → 1×, 100 → 2×, 1000 → 3×).
    pub seeders_points: f64,
    /// Points a torrent with no seeders loses; it will most likely never start.
    pub dead_torrent_penalty: f64,
    /// Audio languages in order of preference, as ISO 639-1 codes.
    pub preferred_languages: Vec<String>,
    pub language_points: f64,
    /// Points lost when the release names its languages and none of them is preferred.
    pub wrong_language_penalty: f64,
    /// Size above which points are taken off, in bytes. `None` turns the rule off.
    pub soft_max_size: Option<u64>,
    /// Points lost per GiB above `soft_max_size`.
    pub oversize_points_per_gib: f64,
    /// Files smaller than this are likely fakes or very low quality, in bytes.
    pub min_size: u64,
    pub undersize_penalty: f64,
    pub cam_penalty: f64,
}

impl Default for ScoringRules {
    fn default() -> Self {
        Self {
            preferred_resolution: 2160,
            resolution_points: 60.0,
            hdr_points: 10.0,
            dolby_vision_points: 15.0,
            cached_points: 300.0,
            uncached_penalty: 150.0,
            seeders_points: 40.0,
            dead_torrent_penalty: 400.0,
            preferred_languages: vec!["en".to_string()],
            language_points: 80.0,
            wrong_language_penalty: 120.0,
            soft_max_size: None,
            oversize_points_per_gib: 5.0,
            min_size: 50 * 1024 * 1024,
            undersize_penalty: 100.0,
            cam_penalty: 500.0,
        }
    }
}

/// One rule's contribution to a stream's score.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScorePart {
    pub reason: String,
    pub points: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub total: f64,
    pub parts: Vec<ScorePart>,
}

struct Tally(Vec<ScorePart>);

impl Tally {
    fn add(&mut self, reason: impl Into<String>, points: f64) {
        if points != 0.0 {
            self.0.push(ScorePart {
                reason: reason.into(),
                points: (points * 10.0).round() / 10.0,
            });
        }
    }
}

fn resolution_steps(resolution: u16) -> f64 {
    match resolution {
        r if r >= 2160 => 4.0,
        r if r >= 1440 => 3.5,
        r if r >= 1080 => 3.0,
        r if r >= 720 => 2.0,
        _ => 1.0,
    }
}

fn source_points(source: &str) -> f64 {
    match source {
        "Remux" => 20.0,
        "BluRay" => 15.0,
        "WEB-DL" => 12.0,
        "WEBRip" => 8.0,
        "HDTV" => 0.0,
        "DVD" => -10.0,
        _ => 0.0,
    }
}

fn audio_points(audio: &[String]) -> f64 {
    audio
        .iter()
        .map(|a| match a.as_str() {
            "Atmos" | "TrueHD" | "DTS:X" => 6.0,
            "DTS-HD" => 5.0,
            "DD+" | "DTS" => 3.0,
            "DD" | "FLAC" => 2.0,
            _ => 0.0,
        })
        .fold(0.0, f64::max)
}

fn language_rule(parsed: &ParsedStream, rules: &ScoringRules, tally: &mut Tally) {
    if rules.preferred_languages.is_empty() || parsed.languages.is_empty() {
        // Many addons only mark non-English audio, so silence is not a reason to judge.
        return;
    }
    let best = rules
        .preferred_languages
        .iter()
        .position(|want| parsed.languages.iter().any(|l| l == want));
    match best {
        Some(rank) => {
            // The first preferred language earns full points, later ones a little less each.
            let points = rules.language_points * (1.0 - 0.25 * rank as f64).max(0.25);
            tally.add(
                format!("Audio in {}", rules.preferred_languages[rank]),
                points,
            );
        }
        None => tally.add("Not in a preferred language", -rules.wrong_language_penalty),
    }
}

fn size_rule(parsed: &ParsedStream, rules: &ScoringRules, tally: &mut Tally) {
    let Some(size) = parsed.size_bytes else {
        return;
    };
    if size < rules.min_size {
        tally.add(
            format!("Suspiciously small ({:.0} MB)", size as f64 / MIB),
            -rules.undersize_penalty,
        );
    }
    if let Some(max) = rules.soft_max_size.filter(|max| size > *max) {
        let over = (size - max) as f64 / GIB;
        tally.add(
            format!("{over:.1} GB over the size limit"),
            -over * rules.oversize_points_per_gib,
        );
    }
}

/// Scores one stream against the rules.
pub fn score(stream: &Stream, rules: &ScoringRules) -> Score {
    let parsed = &stream.parsed;
    let mut tally = Tally(Vec::new());

    // Only direct links and torrents can be played in the app; web pages and YouTube go last.
    let playable = stream.url.is_some() || stream.info_hash.is_some();
    if !playable {
        tally.add("Cannot be played in Spaceport", -10_000.0);
    }

    match parsed.cached {
        Some(true) => tally.add("Cached on debrid", rules.cached_points),
        Some(false) => tally.add("Not cached on debrid", -rules.uncached_penalty),
        None => {}
    }

    // Seeders matter only when the app itself downloads the torrent.
    let own_torrent = stream.info_hash.is_some() && stream.url.is_none();
    if own_torrent {
        match parsed.seeders {
            Some(0) => tally.add("No seeders", -rules.dead_torrent_penalty),
            Some(n) => tally.add(
                format!("{n} seeders"),
                (n as f64 + 1.0).log10() * rules.seeders_points,
            ),
            None => tally.add("Unknown seeders", -rules.seeders_points / 2.0),
        }
    }

    match parsed.resolution {
        Some(r) => {
            let capped = r.min(rules.preferred_resolution);
            tally.add(
                if r > rules.preferred_resolution {
                    format!("{r}p (above preferred {}p)", rules.preferred_resolution)
                } else {
                    format!("{r}p")
                },
                resolution_steps(capped) * rules.resolution_points,
            );
        }
        // Unknown resolution is treated like SD-to-720p: it is often a low quality source.
        None => tally.add("Unknown resolution", 1.5 * rules.resolution_points),
    }

    if parsed.hdr.iter().any(|h| h == "DV") {
        tally.add("Dolby Vision", rules.dolby_vision_points);
    }
    if parsed
        .hdr
        .iter()
        .any(|h| h.starts_with("HDR") || h == "HLG")
    {
        tally.add("HDR", rules.hdr_points);
    }

    if let Some(source) = &parsed.source {
        if source == "CAM" {
            tally.add("Camera recording", -rules.cam_penalty);
        } else {
            tally.add(source.clone(), source_points(source));
        }
    }

    tally.add("Audio", audio_points(&parsed.audio));
    language_rule(parsed, rules, &mut tally);
    size_rule(parsed, rules, &mut tally);

    let total = tally.0.iter().map(|p| p.points).sum::<f64>();
    Score {
        total: (total * 10.0).round() / 10.0,
        parts: tally.0,
    }
}

/// A stream with its score, as returned in ranked order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedStream {
    #[serde(flatten)]
    pub stream: Stream,
    pub score: Score,
}

/// Sorts streams best first. Equal scores keep the addons' own order.
pub fn rank(streams: Vec<Stream>, rules: &ScoringRules) -> Vec<RankedStream> {
    let mut ranked: Vec<RankedStream> = streams
        .into_iter()
        .map(|stream| {
            let score = score(&stream, rules);
            RankedStream { stream, score }
        })
        .collect();
    ranked.sort_by(|a, b| b.score.total.total_cmp(&a.score.total));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ranking::parse::parse;

    fn torrent(name: &str, title: &str) -> Stream {
        stream(name, title, None, Some("abc"))
    }

    fn direct(name: &str, title: &str) -> Stream {
        stream(name, title, Some("https://debrid.example/file"), None)
    }

    fn stream(name: &str, title: &str, url: Option<&str>, info_hash: Option<&str>) -> Stream {
        Stream {
            addon_name: "Addon".into(),
            transport_url: "https://a/manifest.json".into(),
            name: Some(name.into()),
            title: Some(title.into()),
            url: url.map(str::to_string),
            info_hash: info_hash.map(str::to_string),
            file_idx: None,
            sources: vec![],
            yt_id: None,
            external_url: None,
            filename: None,
            video_size: None,
            binge_group: None,
            not_web_ready: false,
            parsed: parse(Some(name), Some(title), None, None),
        }
    }

    fn total(s: &Stream) -> f64 {
        score(s, &ScoringRules::default()).total
    }

    fn order(streams: Vec<Stream>, rules: &ScoringRules) -> Vec<String> {
        rank(streams, rules)
            .into_iter()
            .map(|r| r.stream.title.unwrap())
            .collect()
    }

    #[test]
    fn cached_beats_uncached_and_torrents() {
        let cached = direct("[RD+] Torrentio\n1080p", "A 1080p WEB-DL");
        let uncached = direct("[RD download] Torrentio\n1080p", "B 1080p WEB-DL");
        let seeded = torrent("Torrentio\n1080p", "C 1080p WEB-DL 👤 500");
        let ranked = order(vec![uncached, seeded, cached], &ScoringRules::default());
        assert_eq!(ranked[0], "A 1080p WEB-DL");
        assert_eq!(ranked[2], "B 1080p WEB-DL");
    }

    #[test]
    fn more_seeders_rank_higher() {
        let many = torrent("T\n1080p", "Many 1080p 👤 900");
        let few = torrent("T\n1080p", "Few 1080p 👤 4");
        assert!(total(&many) > total(&few));
    }

    #[test]
    fn dead_torrent_sinks() {
        let dead = torrent("T\n2160p", "Dead 2160p BluRay REMUX DV 👤 0");
        let alive = torrent("T\n720p", "Alive 720p WEB-DL 👤 30");
        assert!(total(&alive) > total(&dead));
    }

    #[test]
    fn seeders_do_not_count_for_debrid_links() {
        let a = direct("[TB+] T\n1080p", "A 1080p 👤 1");
        let b = direct("[TB+] T\n1080p", "B 1080p 👤 5000");
        assert_eq!(total(&a), total(&b));
    }

    #[test]
    fn higher_resolution_ranks_higher() {
        let uhd = direct("[TB+] T", "Movie 2160p WEB-DL");
        let fhd = direct("[TB+] T", "Movie 1080p WEB-DL");
        let hd = direct("[TB+] T", "Movie 720p WEB-DL");
        assert!(total(&uhd) > total(&fhd));
        assert!(total(&fhd) > total(&hd));
    }

    #[test]
    fn preferred_resolution_caps_the_bonus() {
        let rules = ScoringRules {
            preferred_resolution: 1080,
            ..ScoringRules::default()
        };
        let uhd = direct("[TB+] T", "Movie 2160p WEB-DL");
        let fhd = direct("[TB+] T", "Movie 1080p WEB-DL");
        assert_eq!(score(&uhd, &rules).total, score(&fhd, &rules).total);
    }

    #[test]
    fn hdr_and_dolby_vision_add_points() {
        let plain = direct("[TB+] T", "Movie 2160p WEB-DL");
        let hdr = direct("[TB+] T", "Movie 2160p WEB-DL HDR10");
        let dv = direct("[TB+] T", "Movie 2160p WEB-DL DV HDR10");
        assert!(total(&hdr) > total(&plain));
        assert!(total(&dv) > total(&hdr));
    }

    #[test]
    fn cam_is_pushed_down_hard() {
        let cam = torrent("T", "New Movie 1080p HDCAM 👤 5000");
        let web = torrent("T", "New Movie 720p WEB-DL 👤 20");
        assert!(total(&web) > total(&cam));
    }

    #[test]
    fn preferred_language_wins() {
        let rules = ScoringRules {
            preferred_languages: vec!["de".into(), "en".into()],
            ..ScoringRules::default()
        };
        let german = direct("[TB+] T", "Dark 1080p WEB-DL 🇩🇪");
        let english = direct("[TB+] T", "Dark 1080p WEB-DL 🇬🇧");
        let french = direct("[TB+] T", "Dark 1080p WEB-DL 🇫🇷");
        let unknown = direct("[TB+] T", "Dark 1080p WEB-DL");
        let ranked = order(vec![french, unknown, english, german], &rules);
        assert_eq!(
            ranked,
            [
                "Dark 1080p WEB-DL 🇩🇪",
                "Dark 1080p WEB-DL 🇬🇧",
                "Dark 1080p WEB-DL",
                "Dark 1080p WEB-DL 🇫🇷"
            ]
        );
    }

    #[test]
    fn no_language_preference_is_neutral() {
        let rules = ScoringRules {
            preferred_languages: vec![],
            ..ScoringRules::default()
        };
        let a = direct("[TB+] T", "Film 1080p 🇫🇷");
        let b = direct("[TB+] T", "Film 1080p");
        assert_eq!(score(&a, &rules).total, score(&b, &rules).total);
    }

    #[test]
    fn soft_size_limit_takes_points_per_gib() {
        let rules = ScoringRules {
            soft_max_size: Some(10 * 1024 * 1024 * 1024),
            ..ScoringRules::default()
        };
        let small = direct("[TB+] T", "Movie 2160p 💾 8 GB");
        let big = direct("[TB+] T", "Movie 2160p 💾 30 GB");
        let s_small = score(&small, &rules).total;
        let s_big = score(&big, &rules).total;
        assert_eq!(s_small - s_big, 100.0);
    }

    #[test]
    fn tiny_files_are_suspicious() {
        let tiny = direct("[TB+] T", "Movie 1080p 💾 12 MB");
        let normal = direct("[TB+] T", "Movie 1080p 💾 2 GB");
        assert!(total(&normal) - total(&tiny) >= 100.0);
    }

    #[test]
    fn unplayable_streams_go_last() {
        let mut web = direct("Web", "Watch 2160p REMUX");
        web.url = None;
        web.external_url = Some("https://example.org".into());
        let torrent_sd = torrent("T", "Old 480p 👤 3");
        let ranked = order(vec![web, torrent_sd], &ScoringRules::default());
        assert_eq!(ranked[0], "Old 480p 👤 3");
    }

    #[test]
    fn equal_scores_keep_addon_order() {
        let first = direct("[TB+] T", "First 1080p");
        let second = direct("[TB+] T", "Second 1080p");
        let ranked = order(vec![first, second], &ScoringRules::default());
        assert_eq!(ranked, ["First 1080p", "Second 1080p"]);
    }

    #[test]
    fn reasons_explain_the_total() {
        let s = score(
            &torrent("Torrentio\n4k", "Movie 2160p BluRay REMUX Atmos 👤 99 🇬🇧"),
            &ScoringRules::default(),
        );
        let sum: f64 = s.parts.iter().map(|p| p.points).sum();
        assert!((sum - s.total).abs() < 0.11);
        let reasons: Vec<&str> = s.parts.iter().map(|p| p.reason.as_str()).collect();
        assert!(reasons.contains(&"99 seeders"));
        assert!(reasons.contains(&"2160p"));
        assert!(reasons.contains(&"Remux"));
        assert!(reasons.contains(&"Audio in en"));
    }

    #[test]
    fn real_dark_results_rank_sensibly() {
        // Owner's TorBox results for Dark S01E01, all cached; with German preferred, the
        // well-sourced German 4K release should lead and an English-only one should trail.
        let rules = ScoringRules {
            preferred_languages: vec!["de".into()],
            ..ScoringRules::default()
        };
        let german_4k = direct(
            "[TB+] Torrentio\n4k",
            "Dark S01E01 Secrets 2160p NF WEB-DL DUAL DDP5 1 Atmos H 265-Kitsune\n🇩🇪",
        );
        let german_1080 = direct(
            "[TB+] Torrentio\n1080p",
            "Dark.S01.DUAL.1080p.WEBRip.x265-KONTRAST\n👤 62 💾 1.24 GB\nDual Audio / 🇩🇪",
        );
        let english_4k = direct("[TB+] Torrentio\n4k", "Dark.S01E01.2160p.WEB-DL\n🇬🇧");
        let ranked = order(vec![english_4k, german_1080, german_4k], &rules);
        assert!(ranked[0].contains("Kitsune"));
        assert!(ranked[2].contains("🇬🇧"));
    }
}

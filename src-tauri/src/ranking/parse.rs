//! Reads quality details out of the free text addons put in a stream's `name` and `title`.
//!
//! Addons have no shared format, so every field is optional and looked for with patterns that
//! match the common release naming conventions (`2160p`, `x265`, `DDP5.1`, `-GROUP`, …) and
//! the emoji markers many addons use (`👤` seeders, `💾` size, flags for languages).

use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedStream {
    /// Vertical resolution: 2160, 1440, 1080, 720, 576, 480 or 360.
    pub resolution: Option<u16>,
    /// HDR formats, best first: "DV", "HDR10+", "HDR10", "HDR", "HLG".
    pub hdr: Vec<String>,
    /// Video codec: "AV1", "HEVC", "AVC", "VP9" or "XviD".
    pub codec: Option<String>,
    pub bit_depth: Option<u8>,
    /// Audio formats, best first, e.g. "Atmos", "TrueHD", "DD+", "AAC".
    pub audio: Vec<String>,
    /// Channel layout such as "5.1".
    pub channels: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    pub release_group: Option<String>,
    /// Release source: "Remux", "BluRay", "WEB-DL", "WEBRip", "HDTV", "DVD" or "CAM".
    pub source: Option<String>,
    /// Whether a debrid service already has the file. `None` when the addon does not say.
    pub cached: Option<bool>,
    /// Debrid service code from the addon label, e.g. "RD", "TB", "AD", "PM".
    pub debrid: Option<String>,
    /// Audio languages as ISO 639-1 codes, in the order found.
    pub languages: Vec<String>,
    /// The release says it carries more than one audio language.
    pub multi_audio: bool,
}

fn re(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .expect("valid pattern")
}

macro_rules! pattern {
    ($name:ident, $pattern:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| re($pattern));
    };
}

pattern!(RES_2160, r"\b(?:2160p|4k|uhd)\b");
pattern!(RES_1440, r"\b1440p\b");
pattern!(RES_1080, r"\b(?:1080[pi]|fhd)\b");
pattern!(RES_720, r"\b720p\b");
pattern!(RES_576, r"\b576p\b");
pattern!(RES_480, r"\b480p\b");
pattern!(RES_360, r"\b360p\b");

pattern!(DV, r"\b(?:dv|dovi|dolby[ .]?vision)\b");
pattern!(HDR10_PLUS, r"\bhdr10(?:\+|plus)");
pattern!(HDR10, r"\bhdr10\b");
pattern!(HDR, r"\bhdr\b");
pattern!(HLG, r"\bhlg\b");

pattern!(AV1, r"\bav1\b");
pattern!(HEVC, r"\b(?:hevc|[xh][ .]?265)\b");
pattern!(AVC, r"\b(?:avc|[xh][ .]?264)\b");
pattern!(VP9, r"\bvp9\b");
pattern!(XVID, r"\b(?:xvid|divx)\b");

pattern!(BIT_DEPTH, r"\b(8|10|12)[ .-]?bits?\b");

pattern!(ATMOS, r"\batmos\b");
pattern!(TRUEHD, r"\btrue[ .-]?hd\b");
pattern!(DTS_X, r"\bdts[ .:-]?x\b");
pattern!(DTS_HD, r"\bdts[ .-]?hd\b");
pattern!(DTS, r"\bdts\b");
pattern!(DDP, r"\b(?:ddp|dd\+|e[ .-]?ac[ .-]?3)");
pattern!(DD, r"\b(?:dd|ac[ .-]?3)(?:\d|\b)");
pattern!(AAC, r"\baac(?:\d|\b)");
pattern!(FLAC, r"\bflac\b");
pattern!(OPUS, r"\bopus\b");
pattern!(MP3, r"\bmp3\b");

pattern!(
    CHANNELS,
    r"(?:\b|ddp|dd|aac|ac3|eac3|dts|truehd|flac|opus|atmos|ma)([1-9])[ .]([0-2])\b"
);
pattern!(CHANNELS_CH, r"\b([2678])ch\b");

pattern!(
    SIZE,
    r"([0-9]+(?:[.,][0-9]+)?)\s?(tib|tb|gib|gb|mib|mb|kib|kb)\b"
);
pattern!(
    SEEDERS,
    r"(?:👤|👥|🌱|\bseeders?\b|\bseeds?\b)\s*[:=]?\s*([0-9]+)"
);

pattern!(REMUX, r"\bremux\b");
pattern!(BLURAY, r"\b(?:blu[ .-]?ray|bdrip|brrip|bdremux)\b");
pattern!(WEB_DL, r"\bweb[ .-]?dl\b|\bweb\b");
pattern!(WEBRIP, r"\bweb[ .-]?rip\b");
pattern!(HDTV, r"\bhdtv\b");
pattern!(DVD, r"\bdvd(?:rip|r|9|5)?\b");
pattern!(
    CAM,
    r"\b(?:hd[ .-]?cam|camrip|cam|telesync|hd[ .-]?ts|telecine|hd[ .-]?tc)\b"
);

pattern!(DEBRID_TAG, r"\[([a-z]{2,4})(\+|⚡|\s*download)\]");
pattern!(MULTI, r"\bmulti\b|\bdual[ .-]?audio\b|\bdual\b");

pattern!(VIDEO_EXT, r"\.(?:mkv|mp4|avi|ts|m2ts|webm|mov|wmv)$");
pattern!(TRAILING_BRACKETS, r"(?:\s*\[[^\]]*\])+$");
pattern!(DASH_GROUP, r"-([A-Za-z0-9]+)$");
pattern!(LEADING_BRACKET, r"^\[([A-Za-z0-9][A-Za-z0-9 ._-]*)\]");
pattern!(LAST_BRACKET, r"\[([A-Za-z0-9][A-Za-z0-9._-]*)\]$");

/// Language words found in release names, with the ISO 639-1 code they stand for.
const LANGUAGE_WORDS: &[(&str, &str)] = &[
    ("english|eng", "en"),
    ("german|ger|deutsch", "de"),
    ("french|fre|fra|truefrench|vff|vf2", "fr"),
    ("spanish|spa|esp|castellano|latino", "es"),
    ("italian|ita", "it"),
    ("japanese|jap|jpn", "ja"),
    ("korean|kor", "ko"),
    ("russian|rus", "ru"),
    ("portuguese|por", "pt"),
    ("hindi|hin", "hi"),
    ("chinese|chi|mandarin|cantonese", "zh"),
    ("dutch", "nl"),
    ("polish|pol", "pl"),
    ("turkish|tur", "tr"),
    ("arabic|ara", "ar"),
];

static LANGUAGE_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    LANGUAGE_WORDS
        .iter()
        .map(|(words, code)| (re(&format!(r"\b(?:{words})\b")), *code))
        .collect()
});

/// Country flag (as an ISO 3166 code) to the main language spoken there.
const FLAG_LANGUAGES: &[(&str, &str)] = &[
    ("GB", "en"),
    ("US", "en"),
    ("AU", "en"),
    ("CA", "en"),
    ("DE", "de"),
    ("AT", "de"),
    ("FR", "fr"),
    ("ES", "es"),
    ("MX", "es"),
    ("AR", "es"),
    ("IT", "it"),
    ("JP", "ja"),
    ("KR", "ko"),
    ("RU", "ru"),
    ("UA", "uk"),
    ("PT", "pt"),
    ("BR", "pt"),
    ("IN", "hi"),
    ("CN", "zh"),
    ("TW", "zh"),
    ("HK", "zh"),
    ("NL", "nl"),
    ("PL", "pl"),
    ("TR", "tr"),
    ("SA", "ar"),
    ("SE", "sv"),
    ("NO", "no"),
    ("DK", "da"),
    ("FI", "fi"),
    ("CZ", "cs"),
    ("HU", "hu"),
    ("GR", "el"),
    ("IL", "he"),
    ("TH", "th"),
    ("VN", "vi"),
    ("ID", "id"),
    ("RO", "ro"),
];

/// Words that follow a dash in release names but are not release groups.
const NOT_GROUPS: &[&str] = &[
    "dl", "rip", "audio", "hd", "ma", "x", "eng", "ger", "ita", "fre", "spa", "jpn", "x264",
    "x265", "h264", "h265", "hevc", "avc", "web", "sub", "subs", "dub", "dubbed", "multi", "dual",
    "complete", "remux", "uhd", "hdr", "dv",
];

/// Site tags that end some torrent names but name the uploader site, not the group.
const SITE_TAGS: &[&str] = &["rartv", "rarbg", "eztv", "ettv", "tgx", "eztv.re", "ethd"];

/// Decodes flag emoji (pairs of regional indicator symbols) into country codes.
fn flag_countries(text: &str) -> Vec<String> {
    let indicators: Vec<char> = text
        .chars()
        .filter(|c| ('\u{1F1E6}'..='\u{1F1FF}').contains(c))
        .collect();
    indicators
        .chunks_exact(2)
        .map(|pair| {
            pair.iter()
                .map(|c| char::from(b'A' + (*c as u32 - 0x1F1E6) as u8))
                .collect()
        })
        .collect()
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|v| v == value) {
        list.push(value.to_string());
    }
}

fn resolution(text: &str) -> Option<u16> {
    [
        (&RES_2160, 2160),
        (&RES_1440, 1440),
        (&RES_1080, 1080),
        (&RES_720, 720),
        (&RES_576, 576),
        (&RES_480, 480),
        (&RES_360, 360),
    ]
    .into_iter()
    .find(|(pattern, _)| pattern.is_match(text))
    .map(|(_, value)| value)
}

fn hdr(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if DV.is_match(text) {
        found.push("DV".to_string());
    }
    let plus = HDR10_PLUS.is_match(text);
    if plus {
        found.push("HDR10+".to_string());
    }
    // `\bhdr10\b` also matches the start of "HDR10+", so look for a plain one separately.
    let plain_hdr10 = HDR10
        .find_iter(text)
        .any(|m| !text[m.end()..].starts_with('+'));
    if plain_hdr10 {
        found.push("HDR10".to_string());
    }
    if found.iter().all(|f| f == "DV") && HDR.is_match(text) {
        found.push("HDR".to_string());
    }
    if HLG.is_match(text) {
        found.push("HLG".to_string());
    }
    found
}

fn codec(text: &str) -> Option<String> {
    [
        (&AV1, "AV1"),
        (&HEVC, "HEVC"),
        (&AVC, "AVC"),
        (&VP9, "VP9"),
        (&XVID, "XviD"),
    ]
    .into_iter()
    .find(|(pattern, _)| pattern.is_match(text))
    .map(|(_, name)| name.to_string())
}

fn audio(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let dts_x = DTS_X.is_match(text);
    let dts_hd = DTS_HD.is_match(text);
    // `\bdd\b` also matches the start of "DD+", so only count a DD not followed by '+'.
    let plain_dd = DD.find_iter(text).any(|m| !text[m.end()..].starts_with('+'));
    let checks: [(bool, &str); 11] = [
        (ATMOS.is_match(text), "Atmos"),
        (TRUEHD.is_match(text), "TrueHD"),
        (dts_x, "DTS:X"),
        (dts_hd, "DTS-HD"),
        (!dts_x && !dts_hd && DTS.is_match(text), "DTS"),
        (DDP.is_match(text), "DD+"),
        (plain_dd, "DD"),
        (AAC.is_match(text), "AAC"),
        (FLAC.is_match(text), "FLAC"),
        (OPUS.is_match(text), "Opus"),
        (MP3.is_match(text), "MP3"),
    ];
    for (present, name) in checks {
        if present {
            found.push(name.to_string());
        }
    }
    found
}

fn channels(text: &str) -> Option<String> {
    if let Some(c) = CHANNELS.captures(text) {
        let (main, lfe) = (&c[1], &c[2]);
        if matches!(main, "1" | "2" | "5" | "6" | "7") {
            return Some(format!("{main}.{lfe}"));
        }
    }
    CHANNELS_CH.captures(text).map(|c| {
        match &c[1] {
            "2" => "2.0",
            "6" => "5.1",
            _ => "7.1",
        }
        .to_string()
    })
}

fn size_bytes(text: &str) -> Option<u64> {
    // Prefer the size after the 💾 marker when present; other numbers may be bitrates or packs.
    let marked = text.find('💾').map(|i| &text[i..]);
    let caps = marked
        .and_then(|t| SIZE.captures(t))
        .or_else(|| SIZE.captures(text))?;
    let value: f64 = caps[1].replace(',', ".").parse().ok()?;
    let unit = caps[2].to_ascii_lowercase();
    let factor: f64 = match unit.as_str() {
        "tb" | "tib" => 1024f64.powi(4),
        "gb" | "gib" => 1024f64.powi(3),
        "mb" | "mib" => 1024f64.powi(2),
        _ => 1024.0,
    };
    Some((value * factor).round() as u64)
}

fn seeders(text: &str) -> Option<u32> {
    SEEDERS.captures(text).and_then(|c| c[1].parse().ok())
}

fn source(text: &str) -> Option<String> {
    let checks: [(&LazyLock<Regex>, &str); 7] = [
        (&REMUX, "Remux"),
        (&BLURAY, "BluRay"),
        (&WEBRIP, "WEBRip"),
        (&WEB_DL, "WEB-DL"),
        (&HDTV, "HDTV"),
        (&DVD, "DVD"),
        (&CAM, "CAM"),
    ];
    // A camera recording is what matters most when present, whatever else the name claims.
    if CAM.is_match(text) {
        return Some("CAM".to_string());
    }
    checks
        .into_iter()
        .find(|(pattern, _)| pattern.is_match(text))
        .map(|(_, name)| name.to_string())
}

/// A plausible release group, not a codec or language word after a dash.
fn valid_group(group: &str) -> bool {
    !group.chars().all(|c| c.is_ascii_digit())
        && !NOT_GROUPS.contains(&group.to_ascii_lowercase().as_str())
}

/// Looks for the group in one release or file name line.
fn group_in_line(line: &str) -> Option<String> {
    let line = VIDEO_EXT.replace(line.trim(), "");
    let without_tags = TRAILING_BRACKETS.replace(&line, "");
    if let Some(c) = DASH_GROUP.captures(without_tags.trim()) {
        if valid_group(&c[1]) {
            return Some(c[1].to_string());
        }
    }
    // Anime releases put the group first: "[SubsPlease] Title - 01 (1080p)".
    if let Some(c) = LEADING_BRACKET.captures(&line) {
        return Some(c[1].trim().to_string());
    }
    // Otherwise a final "[QxR]" tag, unless it is a site name, a quality or a checksum.
    let tag = LAST_BRACKET.captures(line.trim())?[1].to_string();
    let lower = tag.to_ascii_lowercase();
    let is_checksum = tag.len() == 8 && tag.chars().all(|c| c.is_ascii_hexdigit());
    let is_quality = resolution(&tag).is_some() || source(&tag).is_some();
    (!SITE_TAGS.contains(&lower.as_str()) && !is_checksum && !is_quality && valid_group(&tag))
        .then_some(tag)
}

fn release_group(lines: &[&str]) -> Option<String> {
    lines.iter().find_map(|line| group_in_line(line))
}

fn languages(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for country in flag_countries(text) {
        if let Some((_, lang)) = FLAG_LANGUAGES.iter().find(|(c, _)| *c == country) {
            push_unique(&mut found, lang);
        }
    }
    for (pattern, code) in LANGUAGE_PATTERNS.iter() {
        if pattern.is_match(text) {
            push_unique(&mut found, code);
        }
    }
    found
}

fn debrid(name: &str) -> (Option<String>, Option<bool>) {
    if let Some(c) = DEBRID_TAG.captures(name) {
        let service = c[1].to_ascii_uppercase();
        let cached = !c[2].to_ascii_lowercase().contains("download");
        return (Some(service), Some(cached));
    }
    if name.contains('⚡') {
        return (None, Some(true));
    }
    if name.contains('⏳') {
        return (None, Some(false));
    }
    (None, None)
}

/// Parses one stream's labels. `video_size` (from `behaviorHints`) wins over a size in the text.
pub fn parse(
    name: Option<&str>,
    title: Option<&str>,
    filename: Option<&str>,
    video_size: Option<u64>,
) -> ParsedStream {
    let name = name.unwrap_or_default();
    let title = title.unwrap_or_default();
    let filename = filename.unwrap_or_default();
    // Underscores join words in some names; treat them as separators.
    let text = format!("{name}\n{title}\n{filename}").replace('_', " ");

    // Release and file names are the title lines that are not the stats line (👤 / 💾 / ⚙️).
    let mut name_lines: Vec<&str> = title
        .lines()
        .filter(|l| !l.contains('👤') && !l.contains('💾') && !l.contains('⚙'))
        .map(|l| l.trim_start_matches(|c: char| !c.is_ascii() || c.is_whitespace()))
        .filter(|l| l.chars().any(|c| c.is_ascii_alphanumeric()))
        .collect();
    if !filename.is_empty() {
        name_lines.push(filename);
    }

    let (debrid, cached) = debrid(name);
    ParsedStream {
        resolution: resolution(&text),
        hdr: hdr(&text),
        codec: codec(&text),
        bit_depth: BIT_DEPTH.captures(&text).and_then(|c| c[1].parse().ok()),
        audio: audio(&text),
        channels: channels(&text),
        size_bytes: video_size.or_else(|| size_bytes(&text)),
        seeders: seeders(&text),
        release_group: release_group(&name_lines),
        source: source(&text),
        cached,
        debrid,
        languages: languages(&text),
        multi_audio: MULTI.is_match(&text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    fn gb(bytes: Option<u64>) -> f64 {
        (bytes.unwrap() as f64 / GB * 100.0).round() / 100.0
    }

    fn p(name: &str, title: &str) -> ParsedStream {
        parse(Some(name), Some(title), None, None)
    }

    // The first eight cases are Torrentio + TorBox results for "Dark" S01E01 shared by the owner.

    #[test]
    fn real_dark_1_german_atmos_ntb() {
        let s = p(
            "[TB+] Torrentio\n4k",
            "Dark.S01.GERMAN.2160p.NF.WEBRip.DDP5.1.Atmos.x264-NTb[rartv]\n\
             Dark.S01E01.Secrets.2160p.NF.WEBRip.DDP5.1.x264-NTb.mkv\n\
             👤 72 💾 10.42 GB ⚙️ RARBG\n🇩🇪",
        );
        assert_eq!(s.resolution, Some(2160));
        assert_eq!(s.codec.as_deref(), Some("AVC"));
        assert_eq!(s.audio, ["Atmos", "DD+"]);
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.seeders, Some(72));
        assert_eq!(gb(s.size_bytes), 10.42);
        assert_eq!(s.release_group.as_deref(), Some("NTb"));
        assert_eq!(s.source.as_deref(), Some("WEBRip"));
        assert_eq!(s.cached, Some(true));
        assert_eq!(s.debrid.as_deref(), Some("TB"));
        assert_eq!(s.languages, ["de"]);
    }

    #[test]
    fn real_dark_2_complete_series_no_group() {
        let s = p(
            "[TB+] Torrentio\n4k",
            "Dark.Complete.Series.2017-2020.2160p\nDark.S01E01.2160p.mkv\n\
             👤 17 💾 11.47 GB ⚙️ ThePirateBay\n🇩🇪",
        );
        assert_eq!(s.resolution, Some(2160));
        assert_eq!(s.codec, None);
        assert_eq!(s.seeders, Some(17));
        assert_eq!(gb(s.size_bytes), 11.47);
        assert_eq!(s.release_group, None);
        assert_eq!(s.languages, ["de"]);
    }

    #[test]
    fn real_dark_3_nvenc_10bit_joy() {
        let s = p(
            "[TB+] Torrentio\n4k",
            "DARK (2018) Season 01 S01 (2160p WEB-DL NVENC HEVC 10bit AAC 5.1 English German Joy) [UTR]\n\
             Dark S01E01 Secrets  (2160p NV 10bit Joy).mkv\n\
             👤 7 💾 2.42 GB ⚙️ 1337x\n🇬🇧 / 🇩🇪",
        );
        assert_eq!(s.resolution, Some(2160));
        assert_eq!(s.codec.as_deref(), Some("HEVC"));
        assert_eq!(s.bit_depth, Some(10));
        assert_eq!(s.audio, ["AAC"]);
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.source.as_deref(), Some("WEB-DL"));
        assert_eq!(s.release_group.as_deref(), Some("UTR"));
        assert_eq!(s.languages, ["en", "de"]);
    }

    #[test]
    fn real_dark_4_dual_ddp_atmos_kitsune() {
        let s = p(
            "[TB+] Torrentio\n4k",
            "Dark S01E01 Secrets 2160p NF WEB-DL DUAL DDP5 1 Atmos H 265-Kitsune\n\
             👤 3 💾 5.98 GB ⚙️ ThePirateBay\nDual Audio",
        );
        assert_eq!(s.codec.as_deref(), Some("HEVC"));
        assert_eq!(s.audio, ["Atmos", "DD+"]);
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.release_group.as_deref(), Some("Kitsune"));
        assert!(s.multi_audio);
        assert_eq!(s.seeders, Some(3));
    }

    #[test]
    fn real_dark_5_dual_audio_psa() {
        let s = p(
            "[TB+] Torrentio\n1080p",
            "Dark.S01.COMPLETE.DUAL-AUDIO.GER-ENG.1080p.10bit.WEBRip.6CH.x265\n\
             Dark.S01E01.Secrets.DUAL-AUDIO.GER-ENG.1080p.10bit.WEBRip.6CH.x265.HEVC-PSA.mkv\n\
             👤 493 💾 715.1 MB ⚙️ ThePirateBay\nDual Audio / 🇬🇧 / 🇩🇪",
        );
        assert_eq!(s.resolution, Some(1080));
        assert_eq!(s.codec.as_deref(), Some("HEVC"));
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.seeders, Some(493));
        assert_eq!(
            s.size_bytes,
            Some((715.1f64 * 1024.0 * 1024.0).round() as u64)
        );
        assert_eq!(s.release_group.as_deref(), Some("PSA"));
        assert_eq!(s.languages, ["en", "de"]);
        assert!(s.multi_audio);
    }

    #[test]
    fn real_dark_6_season_pack_qxr() {
        let s = p(
            "[TB+] Torrentio\n1080p",
            "Dark (2017) Season 1-3 S01-S03 (1080p NF WEB-DL x265 HEVC 10bit EAC3 5.1 German Ghost) [QxR]\n\
             Season 1/Dark (2017) - S01E01 - Secrets (1080p NF WEB-DL x265 Ghost).mkv\n\
             👤 178 💾 1.81 GB ⚙️ 1337x\n🇩🇪",
        );
        assert_eq!(s.codec.as_deref(), Some("HEVC"));
        assert_eq!(s.audio, ["DD+"]);
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.release_group.as_deref(), Some("QxR"));
        assert_eq!(s.languages, ["de"]);
        assert_eq!(gb(s.size_bytes), 1.81);
    }

    #[test]
    fn real_dark_7_rapta() {
        let s = p(
            "[TB+] Torrentio\n1080p",
            "Dark - Season 1 S01 - 2017 - 1080p - NF WEB-DL - AAC5.1 x264-Rapta\n\
             Dark - S01E01 - Geheimnisse.mkv\n👤 99 💾 804.39 MB ⚙️ 1337x",
        );
        assert_eq!(s.codec.as_deref(), Some("AVC"));
        assert_eq!(s.audio, ["AAC"]);
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.release_group.as_deref(), Some("Rapta"));
        assert_eq!(s.source.as_deref(), Some("WEB-DL"));
    }

    #[test]
    fn real_dark_8_kontrast() {
        let s = p(
            "[TB+] Torrentio\n1080p",
            "Dark.S01.DUAL.1080p.WEBRip.x265-KONTRAST\nDark.S01E01.DUAL.1080p.WEBRip.x265-KONTRAST.mkv\n\
             👤 62 💾 1.24 GB ⚙️ 1337x\nDual Audio / 🇩🇪",
        );
        assert_eq!(s.release_group.as_deref(), Some("KONTRAST"));
        assert_eq!(s.seeders, Some(62));
        assert!(s.multi_audio);
        assert_eq!(s.languages, ["de"]);
    }

    // The remaining cases follow common formats of popular stream addons and release names.

    #[test]
    fn remux_dv_hdr10_truehd() {
        let s = p(
            "Torrentio\n4k DV | HDR",
            "Oppenheimer.2023.2160p.UHD.BluRay.REMUX.DV.HDR10.HEVC.TrueHD.Atmos.7.1-FGT\n\
             👤 512 💾 78.3 GB ⚙️ TorrentGalaxy",
        );
        assert_eq!(s.resolution, Some(2160));
        assert_eq!(s.hdr, ["DV", "HDR10"]);
        assert_eq!(s.source.as_deref(), Some("Remux"));
        assert_eq!(s.audio, ["Atmos", "TrueHD"]);
        assert_eq!(s.channels.as_deref(), Some("7.1"));
        assert_eq!(s.release_group.as_deref(), Some("FGT"));
        assert_eq!(s.cached, None);
        assert_eq!(s.debrid, None);
    }

    #[test]
    fn hdr10_plus_is_not_plain_hdr10() {
        let s = p(
            "Torrentio\n4k HDR10+",
            "Movie.2024.2160p.AMZN.WEB-DL.DDP5.1.HDR10+.H.265-FLUX",
        );
        assert_eq!(s.hdr, ["HDR10+"]);
        assert_eq!(s.codec.as_deref(), Some("HEVC"));
        assert_eq!(s.release_group.as_deref(), Some("FLUX"));
    }

    #[test]
    fn dolby_vision_spelled_out() {
        let s = p(
            "Addon 4K",
            "Movie 2022 2160p Dolby Vision HDR WEB-DL DD+ 5.1 x265",
        );
        assert_eq!(s.hdr, ["DV", "HDR"]);
        assert_eq!(s.audio, ["DD+"]);
    }

    #[test]
    fn uncached_real_debrid() {
        let s = p(
            "[RD download] Torrentio\n1080p",
            "The.Bear.S03E01.1080p.WEB.h264-ETHEL\n👤 220 💾 2.1 GB ⚙️ EZTV",
        );
        assert_eq!(s.cached, Some(false));
        assert_eq!(s.debrid.as_deref(), Some("RD"));
        assert_eq!(s.codec.as_deref(), Some("AVC"));
        assert_eq!(s.source.as_deref(), Some("WEB-DL"));
        assert_eq!(s.release_group.as_deref(), Some("ETHEL"));
    }

    #[test]
    fn cached_all_debrid_and_premiumize() {
        assert_eq!(
            p("[AD+] Torrentio\n720p", "x").debrid.as_deref(),
            Some("AD")
        );
        let pm = p("[PM+] Torrentio\n1080p", "x");
        assert_eq!((pm.debrid.as_deref(), pm.cached), (Some("PM"), Some(true)));
    }

    #[test]
    fn lightning_marks_cached() {
        let s = p(
            "[TB⚡] Comet 2160p",
            "Dune.Part.Two.2024.2160p.WEB-DL.DDP5.1.Atmos.DV.HDR.H.265-FLUX\n💾 18.5 GB 🔎 TorBox",
        );
        assert_eq!(s.cached, Some(true));
        assert_eq!(s.debrid.as_deref(), Some("TB"));
        assert_eq!(s.hdr, ["DV", "HDR"]);
        assert_eq!(gb(s.size_bytes), 18.5);
    }

    #[test]
    fn hourglass_marks_uncached() {
        let s = p("MediaFusion ⏳ 1080p", "Movie.2021.1080p.BluRay.x264");
        assert_eq!(s.cached, Some(false));
        assert_eq!(s.source.as_deref(), Some("BluRay"));
    }

    #[test]
    fn hdtv_720p() {
        let s = p(
            "Torrentio\n720p",
            "Show.S05E03.720p.HDTV.x264-KILLERS[eztv]\n👤 35 💾 680 MB",
        );
        assert_eq!(s.resolution, Some(720));
        assert_eq!(s.source.as_deref(), Some("HDTV"));
        assert_eq!(s.release_group.as_deref(), Some("KILLERS"));
    }

    #[test]
    fn cam_release() {
        let s = p(
            "Torrentio\n720p",
            "New.Movie.2024.720p.HDCAM.x264-XYZ\n👤 2000",
        );
        assert_eq!(s.source.as_deref(), Some("CAM"));
        assert_eq!(s.seeders, Some(2000));
    }

    #[test]
    fn telesync_release() {
        let s = p("Addon", "New Movie 2024 1080p HD-TS x264");
        assert_eq!(s.source.as_deref(), Some("CAM"));
        assert_eq!(s.resolution, Some(1080));
    }

    #[test]
    fn anime_leading_group_and_checksum() {
        let s = p(
            "Torrentio\n1080p",
            "[SubsPlease] Sousou no Frieren - 15 (1080p) [8E6F4E1A].mkv\n👤 845 💾 1.4 GB ⚙️ Nyaa\n🇯🇵",
        );
        assert_eq!(s.release_group.as_deref(), Some("SubsPlease"));
        assert_eq!(s.languages, ["ja"]);
        assert_eq!(s.resolution, Some(1080));
    }

    #[test]
    fn anime_group_with_dash() {
        let s = p(
            "Torrentio\n1080p",
            "[Erai-raws] Jujutsu Kaisen - 01 [1080p][Multiple Subtitle][ENG].mkv",
        );
        assert_eq!(s.release_group.as_deref(), Some("Erai-raws"));
        assert_eq!(s.languages, ["en"]);
    }

    #[test]
    fn anime_dual_audio_flac() {
        let s = p(
            "Torrentio\n1080p",
            "[Judas] Attack on Titan S04 (BD 1080p HEVC x265 10bit Dual-Audio FLAC 2.0)",
        );
        assert_eq!(s.audio, ["FLAC"]);
        assert_eq!(s.channels.as_deref(), Some("2.0"));
        assert!(s.multi_audio);
        assert_eq!(s.release_group.as_deref(), Some("Judas"));
    }

    #[test]
    fn yts_style() {
        let s = p(
            "Torrentio\n1080p",
            "The Matrix (1999) [1080p] [BluRay] [5.1] [YTS.MX]\n👤 1500 💾 2.13 GB ⚙️ YTS",
        );
        assert_eq!(s.source.as_deref(), Some("BluRay"));
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.release_group.as_deref(), Some("YTS.MX"));
    }

    #[test]
    fn dts_hd_ma_is_not_a_group() {
        let s = p(
            "Torrentio\n1080p",
            "The.Matrix.1999.1080p.BluRay.x264.DTS-HD.MA.5.1-SWTYBLZ",
        );
        assert_eq!(s.audio, ["DTS-HD"]);
        assert_eq!(s.channels.as_deref(), Some("5.1"));
        assert_eq!(s.release_group.as_deref(), Some("SWTYBLZ"));
    }

    #[test]
    fn dts_x_and_plain_dts() {
        assert_eq!(p("a", "Movie.2160p.BluRay.DTS-X.7.1").audio, ["DTS:X"]);
        assert_eq!(p("a", "Movie.1080p.BluRay.DTS.5.1-GRP").audio, ["DTS"]);
    }

    #[test]
    fn web_dl_without_group() {
        let s = p("Addon", "Movie.2023.1080p.WEB-DL");
        assert_eq!(s.release_group, None);
        assert_eq!(s.source.as_deref(), Some("WEB-DL"));
    }

    #[test]
    fn av1_opus() {
        let s = p("Addon 1080p", "Show.S01E02.1080p.WEB.AV1.Opus.5.1-GRP");
        assert_eq!(s.codec.as_deref(), Some("AV1"));
        assert_eq!(s.audio, ["Opus"]);
    }

    #[test]
    fn xvid_dvdrip_480p() {
        let s = p("Addon", "Old.Movie.1995.480p.DVDRip.XviD.AC3-NoGroup");
        assert_eq!(s.resolution, Some(480));
        assert_eq!(s.codec.as_deref(), Some("XviD"));
        assert_eq!(s.source.as_deref(), Some("DVD"));
        assert_eq!(s.audio, ["DD"]);
    }

    #[test]
    fn multi_french() {
        let s = p(
            "Addon 1080p",
            "Film.2020.MULTi.TRUEFRENCH.1080p.BluRay.x264.AC3-NoTag",
        );
        assert!(s.multi_audio);
        assert_eq!(s.languages, ["fr"]);
    }

    #[test]
    fn italian_and_english_words() {
        let s = p("Addon", "Film 2021 1080p WEBRip ITA ENG AC3 5.1 x265-Group");
        assert_eq!(s.languages, ["en", "it"]);
        assert_eq!(s.release_group.as_deref(), Some("Group"));
    }

    #[test]
    fn spanish_latino_flag() {
        let s = p("Addon", "Pelicula 2022 1080p WEB-DL Latino 🇲🇽");
        assert_eq!(s.languages, ["es"]);
    }

    #[test]
    fn seeders_written_out() {
        assert_eq!(
            p("a", "Movie 1080p | Seeders: 42 | 1.5 GB").seeders,
            Some(42)
        );
        assert_eq!(p("a", "Movie 1080p 🌱 9").seeders, Some(9));
    }

    #[test]
    fn size_from_behavior_hints_wins() {
        let s = parse(
            Some("Addon"),
            Some("Movie 1080p 💾 1.0 GB"),
            Some("Movie.1080p.mkv"),
            Some(1_234_567_890),
        );
        assert_eq!(s.size_bytes, Some(1_234_567_890));
    }

    #[test]
    fn size_in_tb_and_comma_decimal() {
        assert_eq!(gb(p("a", "Pack 💾 1.5 TB").size_bytes), 1536.0);
        assert_eq!(gb(p("a", "Film 💾 2,5 GiB").size_bytes), 2.5);
    }

    #[test]
    fn group_from_filename_hint() {
        let s = parse(
            Some("Addon"),
            Some("Some Show S01E01"),
            Some("Some.Show.S01E01.1080p.WEB.H264-SuccessfulCrab.mkv"),
            None,
        );
        assert_eq!(s.release_group.as_deref(), Some("SuccessfulCrab"));
        assert_eq!(s.codec.as_deref(), Some("AVC"));
    }

    #[test]
    fn hlg_and_12bit() {
        let s = p("a", "Nature.2160p.HLG.12bit.HEVC");
        assert_eq!(s.hdr, ["HLG"]);
        assert_eq!(s.bit_depth, Some(12));
    }

    #[test]
    fn plain_text_gives_nothing() {
        let s = p("Trailer", "Official trailer");
        assert_eq!(s, ParsedStream::default());
    }
}

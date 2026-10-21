//! Language codes as they appear in settings (ISO 639-1, "de"), media files and subtitle addons
//! (ISO 639-2, "ger" or "deu"), with English names for the UI.

/// (ISO 639-1, ISO 639-2/B, ISO 639-2/T, English name)
const LANGUAGES: &[(&str, &str, &str, &str)] = &[
    ("ar", "ara", "ara", "Arabic"),
    ("bg", "bul", "bul", "Bulgarian"),
    ("cs", "cze", "ces", "Czech"),
    ("da", "dan", "dan", "Danish"),
    ("de", "ger", "deu", "German"),
    ("el", "gre", "ell", "Greek"),
    ("en", "eng", "eng", "English"),
    ("es", "spa", "spa", "Spanish"),
    ("et", "est", "est", "Estonian"),
    ("fa", "per", "fas", "Persian"),
    ("fi", "fin", "fin", "Finnish"),
    ("fr", "fre", "fra", "French"),
    ("he", "heb", "heb", "Hebrew"),
    ("hi", "hin", "hin", "Hindi"),
    ("hr", "hrv", "hrv", "Croatian"),
    ("hu", "hun", "hun", "Hungarian"),
    ("id", "ind", "ind", "Indonesian"),
    ("it", "ita", "ita", "Italian"),
    ("ja", "jpn", "jpn", "Japanese"),
    ("ko", "kor", "kor", "Korean"),
    ("lt", "lit", "lit", "Lithuanian"),
    ("lv", "lav", "lav", "Latvian"),
    ("ms", "may", "msa", "Malay"),
    ("nl", "dut", "nld", "Dutch"),
    ("no", "nor", "nor", "Norwegian"),
    ("pl", "pol", "pol", "Polish"),
    ("pt", "por", "por", "Portuguese"),
    ("ro", "rum", "ron", "Romanian"),
    ("ru", "rus", "rus", "Russian"),
    ("sk", "slo", "slk", "Slovak"),
    ("sl", "slv", "slv", "Slovenian"),
    ("sr", "srp", "srp", "Serbian"),
    ("sv", "swe", "swe", "Swedish"),
    ("th", "tha", "tha", "Thai"),
    ("tr", "tur", "tur", "Turkish"),
    ("uk", "ukr", "ukr", "Ukrainian"),
    ("vi", "vie", "vie", "Vietnamese"),
    ("zh", "chi", "zho", "Chinese"),
];

fn find(code: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    let code = code.trim().to_ascii_lowercase();
    // Tags like "pt-BR" or "en_US" are matched by their language part.
    let base = code.split(['-', '_']).next().unwrap_or_default();
    LANGUAGES
        .iter()
        .find(|(one, b, t, _)| base == *one || base == *b || base == *t)
}

/// The ISO 639-1 code for any known form ("ger", "deu", "de-AT" give "de").
pub fn normalize(code: &str) -> Option<&'static str> {
    find(code).map(|l| l.0)
}

/// An English name for a code, or the code itself in capitals when unknown.
pub fn name(code: &str) -> String {
    match find(code) {
        Some(l) => l.3.to_string(),
        None => code.trim().to_uppercase(),
    }
}

/// Every form mpv may see in a file for these languages, for its `alang` / `slang` options,
/// e.g. ["de", "en"] gives "de,ger,deu,en,eng".
pub fn mpv_list(codes: &[String]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for code in codes {
        if let Some((one, b, t, _)) = find(code) {
            for c in [*one, *b, *t] {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
    }
    out.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_all_forms() {
        assert_eq!(normalize("de"), Some("de"));
        assert_eq!(normalize("ger"), Some("de"));
        assert_eq!(normalize("DEU"), Some("de"));
        assert_eq!(normalize("pt-BR"), Some("pt"));
        assert_eq!(normalize("en_US"), Some("en"));
        assert_eq!(normalize("xx"), None);
    }

    #[test]
    fn names() {
        assert_eq!(name("eng"), "English");
        assert_eq!(name("ja"), "Japanese");
        assert_eq!(name("tlh"), "TLH");
    }

    #[test]
    fn mpv_language_list() {
        assert_eq!(mpv_list(&["de".into(), "en".into()]), "de,ger,deu,en,eng");
        assert_eq!(mpv_list(&["xx".into()]), "");
    }
}

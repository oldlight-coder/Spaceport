//! Forgiving title matching for search: how close a title is to what the user typed, and looser
//! forms of a query to send to addons whose own search wants the words spelled right.

/// Titles scoring below this are not offered as matches.
pub const MATCH_THRESHOLD: f64 = 0.6;

/// Lower case, common accents removed, "&" read as "and", punctuation as spaces.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ç' | 'č' | 'ć' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ē' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ñ' | 'ń' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
            'ý' | 'ÿ' => 'y',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            'ß' => 's',
            other => other,
        };
        if c == '&' {
            out.push_str(" and ");
        } else if c.is_alphanumeric() {
            out.push(c);
        } else if c != '\'' && c != '’' {
            // "Grey's" reads as "greys", other punctuation separates words.
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Edits (insert, delete, substitute, swap neighbours) to turn `a` into `b`.
fn distance(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = best;
        }
    }
    d[n][m]
}

/// 1 for equal strings down to 0 for nothing in common.
fn similarity(a: &[char], b: &[char]) -> f64 {
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    1.0 - distance(a, b) as f64 / longest as f64
}

/// How well one typed word matches one title word. A word still being typed (`partial`) also
/// matches the start of a longer word.
fn word_match(typed: &[char], word: &[char], partial: bool) -> f64 {
    let whole = similarity(typed, word);
    if !partial || word.len() <= typed.len() {
        return whole;
    }
    let start = similarity(typed, &word[..typed.len()]);
    whole.max(start * 0.97)
}

/// How close `title` is to the typed `query`, from 0 to 1. The last typed word may be
/// unfinished, as in autocomplete.
pub fn score(query: &str, title: &str) -> f64 {
    let (q, t) = (normalize(query), normalize(title));
    if q.is_empty() || t.is_empty() {
        return 0.0;
    }
    if q == t {
        return 1.0;
    }
    let chars = |s: &str| s.chars().collect::<Vec<_>>();
    let typed: Vec<Vec<char>> = q.split(' ').map(chars).collect();
    let words: Vec<Vec<char>> = t.split(' ').map(chars).collect();
    // Each typed word against its best title word, weighted by length.
    let mut total = 0.0;
    let mut weight = 0.0;
    for (i, word) in typed.iter().enumerate() {
        let partial = i + 1 == typed.len();
        let best = words
            .iter()
            .map(|w| word_match(word, w, partial))
            .fold(0.0, f64::max);
        let w = word.len() as f64;
        total += best * w;
        weight += w;
    }
    let by_words = total / weight;
    // Title words nobody typed count against it a little ("Bad" is not "Breaking Bad").
    let extra = words.len().saturating_sub(typed.len()) as f64;
    let by_words = by_words - (extra * 0.04).min(0.2);
    let squash = |s: &str| s.replace(' ', "").chars().collect::<Vec<_>>();
    let whole = similarity(&squash(&q), &squash(&t));
    let starts = if t.starts_with(&q) { 0.95 } else { 0.0 };
    by_words.max(whole).max(starts).clamp(0.0, 1.0)
}

/// Looser forms of a query, best first, for addons whose search needs exact words: the query,
/// its words cut to their first four letters, and its longest word alone (shortened). A typo
/// past the fourth letter of each word then still finds the title.
pub fn variants(query: &str) -> Vec<String> {
    let q = normalize(query);
    let mut out = vec![q.clone()];
    let words: Vec<&str> = q.split(' ').filter(|w| !w.is_empty()).collect();
    let cut = |w: &str| w.chars().take(4).collect::<String>();
    let shortened = words
        .iter()
        .map(|w| {
            if w.chars().count() > 4 {
                cut(w)
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let longest = words.iter().copied().max_by_key(|w| w.chars().count());
    let alone = longest.filter(|w| w.chars().count() > 4).map(cut);
    for v in [Some(shortened), alone].into_iter().flatten() {
        if v.chars().count() >= 3 && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_titles() {
        assert_eq!(
            normalize("  Amélie & Co.: Part II "),
            "amelie and co part ii"
        );
        assert_eq!(normalize("Grey's Anatomy"), "greys anatomy");
        assert_eq!(
            normalize("Frieren: Beyond Journey’s End"),
            "frieren beyond journeys end"
        );
    }

    #[test]
    fn typos_still_match() {
        assert!(score("breakng bad", "Breaking Bad") > 0.85);
        assert!(score("the mandolorian", "The Mandalorian") > 0.85);
        assert!(score("intersteller", "Interstellar") > 0.8);
        assert!(score("game of throne", "Game of Thrones") > 0.85);
    }

    #[test]
    fn unfinished_words_match_like_autocomplete() {
        assert!(score("breaking b", "Breaking Bad") > 0.9);
        assert!(score("stranger thi", "Stranger Things") > 0.9);
    }

    #[test]
    fn unrelated_titles_do_not() {
        assert!(score("breaking bad", "Bad Boys") < MATCH_THRESHOLD);
        assert!(score("interstellar", "Inside Out") < MATCH_THRESHOLD);
        assert!(score("dark", "The Dark Knight") < score("dark", "Dark"));
    }

    #[test]
    fn closer_titles_rank_higher() {
        let q = "the office";
        assert!(score(q, "The Office") > score(q, "The Office Mix-Up"));
        assert!(score("breakng bad", "Breaking Bad") > score("breakng bad", "Bad Sisters"));
    }

    #[test]
    fn looser_query_forms() {
        assert_eq!(variants("Breakng Bad"), ["breakng bad", "brea bad", "brea"]);
        assert_eq!(
            variants("the mandolorian"),
            ["the mandolorian", "the mand", "mand"]
        );
        assert_eq!(variants("dune"), ["dune"]);
    }
}

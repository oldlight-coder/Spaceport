//! The search assistant: results from every search catalog for a query and its looser forms,
//! merged and ranked by how close each title is to what was typed.

use serde::Serialize;

use crate::fuzzy;
use crate::stremio::catalog::{CatalogRef, MetaPreview};

/// One title found, with the addon it came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    #[serde(flatten)]
    pub meta: MetaPreview,
    pub transport_url: String,
    pub addon_name: String,
    pub score: f64,
}

/// What one catalog answered for one form of the query (`variant` 0 is the query as typed).
pub struct Answer<'a> {
    pub catalog: &'a CatalogRef,
    pub variant: usize,
    pub metas: Vec<MetaPreview>,
}

/// Titles close enough to `query`, best first, each once. A title found by the query as typed
/// gets a small lead over one only a looser form found; ties keep the addons' order.
pub fn rank(query: &str, answers: &[Answer], limit: usize) -> Vec<SearchHit> {
    let mut hits: Vec<(SearchHit, usize)> = Vec::new();
    let mut order = 0;
    for answer in answers {
        for meta in &answer.metas {
            order += 1;
            let mut score = fuzzy::score(query, &meta.name);
            if answer.variant == 0 {
                score = (score + 0.03).min(1.0);
            }
            if score < fuzzy::MATCH_THRESHOLD {
                continue;
            }
            match hits.iter_mut().find(|(h, _)| h.meta.id == meta.id) {
                Some((hit, _)) if hit.score >= score => {}
                Some((hit, _)) => hit.score = score,
                None => hits.push((
                    SearchHit {
                        meta: meta.clone(),
                        transport_url: answer.catalog.transport_url.clone(),
                        addon_name: answer.catalog.addon_name.clone(),
                        score,
                    },
                    order,
                )),
            }
        }
    }
    hits.sort_by(|(a, ao), (b, bo)| b.score.total_cmp(&a.score).then(ao.cmp(bo)));
    hits.into_iter().take(limit).map(|(h, _)| h).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(name: &str) -> CatalogRef {
        CatalogRef {
            transport_url: format!("https://{name}/manifest.json"),
            addon_name: name.into(),
            kind: "series".into(),
            id: "search".into(),
            name: "Search".into(),
            genres: vec![],
            genre_required: false,
            supports_skip: false,
        }
    }

    fn meta(id: &str, name: &str) -> MetaPreview {
        MetaPreview {
            id: id.into(),
            kind: "series".into(),
            name: name.into(),
            poster: None,
            poster_shape: None,
            release_info: None,
        }
    }

    #[test]
    fn merges_ranks_and_drops_poor_matches() {
        let (a, b) = (catalog("a"), catalog("b"));
        let answers = [
            // The typed query found nothing close; the looser "brea bad" did.
            Answer {
                catalog: &a,
                variant: 0,
                metas: vec![meta("tt9", "Bad Boys")],
            },
            Answer {
                catalog: &a,
                variant: 1,
                metas: vec![meta("tt2", "Bad Sisters"), meta("tt1", "Breaking Bad")],
            },
            Answer {
                catalog: &b,
                variant: 1,
                metas: vec![meta("tt1", "Breaking Bad")],
            },
        ];
        let hits = rank("breakng bad", &answers, 10);
        let ids: Vec<&str> = hits.iter().map(|h| h.meta.id.as_str()).collect();
        assert_eq!(ids, ["tt1"]);
        assert_eq!(hits[0].addon_name, "a");
    }

    #[test]
    fn keeps_the_limit_and_the_addon_order_on_ties() {
        let a = catalog("a");
        let answers = [Answer {
            catalog: &a,
            variant: 0,
            metas: vec![
                meta("tt1", "Dark"),
                meta("tt2", "Dark"),
                meta("tt3", "Dark"),
            ],
        }];
        let hits = rank("dark", &answers, 2);
        let ids: Vec<&str> = hits.iter().map(|h| h.meta.id.as_str()).collect();
        assert_eq!(ids, ["tt1", "tt2"]);
    }
}

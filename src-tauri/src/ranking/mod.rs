//! Spaceport's own stream ranking: parsing addon labels, scoring and filtering.

pub mod filters;
pub mod parse;
pub mod score;

use serde::Serialize;

use crate::stremio::streams::{AddonOutcome, StreamResults};
use filters::{rejection, StreamFilters};
use score::{rank, RankedStream, ScoringRules};

/// A stream the user's filters removed, with the reason.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenStream {
    #[serde(flatten)]
    pub stream: RankedStream,
    pub hidden_because: String,
}

/// Stream results in ranked order, best first, with the filtered-out ones kept apart.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedResults {
    pub streams: Vec<RankedStream>,
    pub hidden: Vec<HiddenStream>,
    pub addons: Vec<AddonOutcome>,
}

pub fn rank_results(
    results: StreamResults,
    rules: &ScoringRules,
    filters: &StreamFilters,
) -> RankedResults {
    let mut streams = Vec::new();
    let mut hidden = Vec::new();
    for ranked in rank(results.streams, rules) {
        match rejection(&ranked.stream, filters) {
            Some(reason) => hidden.push(HiddenStream {
                stream: ranked,
                hidden_because: reason,
            }),
            None => streams.push(ranked),
        }
    }
    RankedResults {
        streams,
        hidden,
        addons: results.addons,
    }
}

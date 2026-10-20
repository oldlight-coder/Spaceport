//! Spaceport's own stream ranking: parsing addon labels, scoring and filtering.

pub mod parse;
pub mod score;

use serde::Serialize;

use crate::stremio::streams::{AddonOutcome, StreamResults};
use score::{rank, RankedStream, ScoringRules};

/// Stream results in ranked order, best first.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedResults {
    pub streams: Vec<RankedStream>,
    pub addons: Vec<AddonOutcome>,
}

pub fn rank_results(results: StreamResults, rules: &ScoringRules) -> RankedResults {
    RankedResults {
        streams: rank(results.streams, rules),
        addons: results.addons,
    }
}

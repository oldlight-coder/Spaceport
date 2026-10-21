//! Which stream to play next when one fails.
//!
//! The UI hands over the ranked candidates; the player starts the first and moves down the list
//! when a stream does not start, errors out, or ends long before its known duration (a dead link
//! often looks like an early end of file). When playback had already begun, the next stream
//! resumes at the same position.

use std::time::{Duration, Instant};

use serde::Deserialize;

/// How long a stream may take to start before the next one is tried.
pub const START_TIMEOUT: Duration = Duration::from_secs(30);

/// An end of file this far before the known duration counts as a failure, in seconds.
const EARLY_END_MARGIN: f64 = 120.0;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub url: String,
    /// Short label shown when switching, e.g. "[TB+] Torrentio 4k".
    pub label: String,
}

/// What the player should do after an event.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Load this candidate, starting at `start` seconds when set.
    Load {
        url: String,
        label: String,
        start: Option<f64>,
    },
    /// Every candidate failed.
    GiveUp { tried: usize, last_error: String },
    /// Nothing to do.
    Wait,
}

#[derive(Debug)]
pub struct PlayQueue {
    candidates: Vec<Candidate>,
    index: usize,
    loaded: bool,
    started_at: Instant,
    position: Option<f64>,
    duration: Option<f64>,
}

impl PlayQueue {
    /// Starts with the first candidate. Returns `None` for an empty list.
    pub fn new(candidates: Vec<Candidate>, now: Instant) -> Option<(Self, Action)> {
        let first = candidates.first()?.clone();
        let queue = Self {
            candidates,
            index: 0,
            loaded: false,
            started_at: now,
            position: None,
            duration: None,
        };
        Some((
            queue,
            Action::Load {
                url: first.url,
                label: first.label,
                start: None,
            },
        ))
    }

    pub fn current_label(&self) -> &str {
        &self.candidates[self.index].label
    }

    pub fn labels(&self) -> Vec<String> {
        self.candidates.iter().map(|c| c.label.clone()).collect()
    }

    pub fn index(&self) -> usize {
        self.index
    }

    /// The user picked another source: play it from the current position.
    pub fn select(&mut self, index: usize, now: Instant) -> Option<Action> {
        let next = self.candidates.get(index)?.clone();
        self.index = index;
        self.loaded = false;
        self.started_at = now;
        Some(Action::Load {
            url: next.url,
            label: next.label,
            start: self.position.filter(|p| *p > 1.0),
        })
    }

    pub fn on_loaded(&mut self) {
        self.loaded = true;
    }

    pub fn on_position(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds >= 0.0 {
            self.position = Some(seconds);
        }
    }

    pub fn on_duration(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.duration = Some(seconds);
        }
    }

    /// The current stream failed: move on to the next one.
    pub fn on_failed(&mut self, error: &str, now: Instant) -> Action {
        // Resume where playback got to, if it got anywhere.
        let start = self.position.filter(|p| *p > 1.0);
        if self.index + 1 >= self.candidates.len() {
            return Action::GiveUp {
                tried: self.candidates.len(),
                last_error: error.to_string(),
            };
        }
        self.index += 1;
        self.loaded = false;
        self.started_at = now;
        let next = self.candidates[self.index].clone();
        Action::Load {
            url: next.url,
            label: next.label,
            start,
        }
    }

    /// The stream ended. Ending far before the known duration means the link died.
    pub fn on_end_of_file(&mut self, now: Instant) -> Action {
        match (self.position, self.duration) {
            (Some(position), Some(duration)) if position + EARLY_END_MARGIN < duration => {
                self.on_failed("the stream stopped early", now)
            }
            _ => Action::Wait,
        }
    }

    /// Called regularly: a stream that has not started in time is treated as failed.
    pub fn on_tick(&mut self, now: Instant) -> Action {
        if !self.loaded && now.duration_since(self.started_at) >= START_TIMEOUT {
            self.on_failed("the stream did not start in time", now)
        } else {
            Action::Wait
        }
    }

    /// Whether the end of file was the real end, so the UI can treat it as finished.
    pub fn finished_normally(&self) -> bool {
        !matches!(
            (self.position, self.duration),
            (Some(p), Some(d)) if p + EARLY_END_MARGIN < d
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(n: usize) -> Vec<Candidate> {
        (1..=n)
            .map(|i| Candidate {
                url: format!("https://debrid.example/{i}"),
                label: format!("Stream {i}"),
            })
            .collect()
    }

    fn load(url: &str, start: Option<f64>) -> Action {
        let n = url.rsplit('/').next().unwrap();
        Action::Load {
            url: url.to_string(),
            label: format!("Stream {n}"),
            start,
        }
    }

    #[test]
    fn empty_list_plays_nothing() {
        assert!(PlayQueue::new(vec![], Instant::now()).is_none());
    }

    #[test]
    fn starts_with_the_top_candidate() {
        let (_, action) = PlayQueue::new(candidates(3), Instant::now()).unwrap();
        assert_eq!(action, load("https://debrid.example/1", None));
    }

    #[test]
    fn failure_before_start_moves_to_next_from_the_beginning() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(3), now).unwrap();
        assert_eq!(
            q.on_failed("HTTP 404", now),
            load("https://debrid.example/2", None)
        );
        assert_eq!(q.current_label(), "Stream 2");
    }

    #[test]
    fn failure_mid_playback_resumes_at_same_position() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_loaded();
        q.on_duration(5400.0);
        q.on_position(1234.5);
        assert_eq!(
            q.on_failed("connection reset", now),
            load("https://debrid.example/2", Some(1234.5))
        );
    }

    #[test]
    fn gives_up_after_the_last_candidate() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_failed("a", now);
        assert_eq!(
            q.on_failed("b", now),
            Action::GiveUp {
                tried: 2,
                last_error: "b".into()
            }
        );
    }

    #[test]
    fn start_timeout_counts_as_failure() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        assert_eq!(q.on_tick(now + Duration::from_secs(5)), Action::Wait);
        assert_eq!(
            q.on_tick(now + START_TIMEOUT),
            load("https://debrid.example/2", None)
        );
        // The clock restarts for the new candidate.
        assert_eq!(
            q.on_tick(now + START_TIMEOUT + Duration::from_secs(5)),
            Action::Wait
        );
    }

    #[test]
    fn loaded_streams_do_not_time_out() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_loaded();
        assert_eq!(q.on_tick(now + START_TIMEOUT * 10), Action::Wait);
    }

    #[test]
    fn early_end_of_file_is_a_dead_link() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_loaded();
        q.on_duration(3000.0);
        q.on_position(600.0);
        assert!(!q.finished_normally());
        assert_eq!(
            q.on_end_of_file(now),
            load("https://debrid.example/2", Some(600.0))
        );
    }

    #[test]
    fn real_end_of_file_is_not_a_failure() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_loaded();
        q.on_duration(3000.0);
        q.on_position(2990.0);
        assert!(q.finished_normally());
        assert_eq!(q.on_end_of_file(now), Action::Wait);
    }

    #[test]
    fn unknown_duration_end_is_not_a_failure() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_position(100.0);
        assert_eq!(q.on_end_of_file(now), Action::Wait);
    }

    #[test]
    fn switching_source_keeps_position() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(3), now).unwrap();
        q.on_loaded();
        q.on_position(300.0);
        assert_eq!(
            q.select(2, now),
            Some(load("https://debrid.example/3", Some(300.0)))
        );
        assert_eq!(q.index(), 2);
        assert_eq!(q.select(9, now), None);
        assert_eq!(q.labels().len(), 3);
    }

    #[test]
    fn ignores_bad_numbers() {
        let now = Instant::now();
        let (mut q, _) = PlayQueue::new(candidates(2), now).unwrap();
        q.on_position(f64::NAN);
        q.on_duration(-1.0);
        assert_eq!(
            q.on_failed("x", now),
            load("https://debrid.example/2", None)
        );
    }
}

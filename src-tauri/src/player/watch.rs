//! Keeps track of what is being watched and when to report progress.
//!
//! Progress is reported every 30 seconds of playback, when the video is paused, when it ends
//! and when the player closes. Watch time only counts real playback: seeking forward does not
//! add to it.

use std::time::{Duration, Instant};

use crate::progress::TitleInfo;

/// How often progress is reported while a video plays.
pub const REPORT_INTERVAL: Duration = Duration::from_secs(30);

/// A position jump larger than this between two updates is a seek, not playback.
const MAX_PLAYBACK_STEP: f64 = 5.0;

/// The title and video being played.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchTarget {
    pub title: TitleInfo,
    /// The video id: an episode id like "tt0903747:1:2", or the title id for a movie.
    pub video_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WatchReport {
    pub target: WatchTarget,
    /// Seconds.
    pub position: f64,
    pub duration: f64,
    /// Seconds actually played since the previous report.
    pub watched_seconds: f64,
    /// The video played to its end.
    pub finished: bool,
}

#[derive(Debug)]
pub struct WatchTracker {
    target: WatchTarget,
    started: bool,
    position: f64,
    duration: f64,
    last_position: Option<f64>,
    watched: f64,
    last_report: Instant,
}

impl WatchTracker {
    pub fn new(target: WatchTarget, now: Instant) -> Self {
        Self {
            target,
            started: false,
            position: 0.0,
            duration: 0.0,
            last_position: None,
            watched: 0.0,
            last_report: now,
        }
    }

    pub fn video_id(&self) -> &str {
        &self.target.video_id
    }

    pub fn on_loaded(&mut self) {
        self.started = true;
    }

    pub fn on_position(&mut self, seconds: f64) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        if let Some(last) = self.last_position {
            let step = seconds - last;
            if step > 0.0 && step <= MAX_PLAYBACK_STEP {
                self.watched += step;
            }
        }
        self.last_position = Some(seconds);
        self.position = seconds;
    }

    pub fn on_duration(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.duration = seconds;
        }
    }

    /// Whether the periodic report is due.
    pub fn due(&self, now: Instant, paused: bool) -> bool {
        self.started && !paused && now.duration_since(self.last_report) >= REPORT_INTERVAL
    }

    /// The progress to save, if the video ever started. Resets the watch time counter.
    pub fn report(&mut self, now: Instant, finished: bool) -> Option<WatchReport> {
        if !self.started {
            return None;
        }
        let position = if finished && self.duration > 0.0 {
            self.duration
        } else {
            self.position
        };
        let report = WatchReport {
            target: self.target.clone(),
            position,
            duration: self.duration,
            watched_seconds: self.watched,
            finished,
        };
        self.watched = 0.0;
        self.last_report = now;
        Some(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> WatchTarget {
        WatchTarget {
            title: TitleInfo {
                id: "tt1".into(),
                kind: "series".into(),
                name: "Show".into(),
                poster: None,
                video_ids: vec![],
                genres: vec![],
            },
            video_id: "tt1:1:1".into(),
        }
    }

    #[test]
    fn nothing_to_report_before_start() {
        let now = Instant::now();
        let mut w = WatchTracker::new(target(), now);
        w.on_position(10.0);
        assert!(!w.due(now + REPORT_INTERVAL, false));
        assert_eq!(w.report(now, false), None);
    }

    #[test]
    fn counts_only_real_playback() {
        let now = Instant::now();
        let mut w = WatchTracker::new(target(), now);
        w.on_loaded();
        w.on_duration(3000.0);
        for t in [0.0, 1.0, 2.0, 3.0] {
            w.on_position(t);
        }
        // A seek forward is not watch time.
        w.on_position(600.0);
        w.on_position(601.5);
        let report = w.report(now, false).unwrap();
        assert_eq!(report.position, 601.5);
        assert_eq!(report.watched_seconds, 4.5);
        assert_eq!(report.duration, 3000.0);
        // The counter starts again after a report.
        assert_eq!(w.report(now, false).unwrap().watched_seconds, 0.0);
    }

    #[test]
    fn periodic_report_while_playing() {
        let now = Instant::now();
        let mut w = WatchTracker::new(target(), now);
        w.on_loaded();
        assert!(!w.due(now + Duration::from_secs(10), false));
        assert!(w.due(now + REPORT_INTERVAL, false));
        assert!(!w.due(now + REPORT_INTERVAL, true));
        w.report(now + REPORT_INTERVAL, false);
        assert!(!w.due(now + REPORT_INTERVAL + Duration::from_secs(1), false));
    }

    #[test]
    fn finished_reports_the_full_length() {
        let now = Instant::now();
        let mut w = WatchTracker::new(target(), now);
        w.on_loaded();
        w.on_duration(3000.0);
        w.on_position(2995.0);
        let report = w.report(now, true).unwrap();
        assert!(report.finished);
        assert_eq!(report.position, 3000.0);
    }
}

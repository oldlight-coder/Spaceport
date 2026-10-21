//! Which downloaded torrents to delete when the cache grows past the user's limit.

use std::time::{Duration, Instant};

/// A torrent watched this recently counts as in use even with no stream open, so that the
/// short gaps between mpv's requests (for example while seeking) never free it.
pub const RECENT_USE: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct CacheEntry<K> {
    pub key: K,
    /// Bytes downloaded so far.
    pub size: u64,
    pub last_used: Instant,
    /// Streams currently reading from it.
    pub open_streams: usize,
}

impl<K> CacheEntry<K> {
    fn in_use(&self, now: Instant) -> bool {
        self.open_streams > 0 || now.duration_since(self.last_used) < RECENT_USE
    }
}

/// The torrents to delete, least recently used first, until the total fits in `limit`.
/// Torrents in use are never chosen, even if they alone exceed the limit.
pub fn evictions<K: Clone>(entries: &[CacheEntry<K>], limit: u64, now: Instant) -> Vec<K> {
    let mut total: u64 = entries.iter().map(|e| e.size).sum();
    if total <= limit {
        return Vec::new();
    }
    let mut idle: Vec<&CacheEntry<K>> = entries.iter().filter(|e| !e.in_use(now)).collect();
    idle.sort_by_key(|e| e.last_used);
    let mut chosen = Vec::new();
    for entry in idle {
        if total <= limit {
            break;
        }
        total = total.saturating_sub(entry.size);
        chosen.push(entry.key.clone());
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1024 * 1024 * 1024;

    fn entry(
        key: &'static str,
        gb: u64,
        minutes_ago: u64,
        open: usize,
        now: Instant,
    ) -> CacheEntry<&'static str> {
        CacheEntry {
            key,
            size: gb * GB,
            last_used: now - Duration::from_secs(minutes_ago * 60),
            open_streams: open,
        }
    }

    #[test]
    fn under_the_limit_keeps_everything() {
        let now = Instant::now() + Duration::from_secs(3 * 3600);
        let entries = [entry("a", 3, 30, 0, now), entry("b", 4, 60, 0, now)];
        assert!(evictions(&entries, 10 * GB, now).is_empty());
    }

    #[test]
    fn deletes_least_recently_used_first() {
        let now = Instant::now() + Duration::from_secs(3 * 3600);
        let entries = [
            entry("newer", 4, 10, 0, now),
            entry("oldest", 5, 50, 0, now),
            entry("older", 3, 30, 0, now),
        ];
        // 12 GB against 8 GB: deleting the oldest (5 GB) is enough.
        assert_eq!(evictions(&entries, 8 * GB, now), ["oldest"]);
        // Against 4 GB the two oldest go.
        assert_eq!(evictions(&entries, 4 * GB, now), ["oldest", "older"]);
    }

    #[test]
    fn never_deletes_what_is_playing() {
        let now = Instant::now() + Duration::from_secs(3 * 3600);
        let entries = [
            entry("playing", 30, 90, 1, now),
            entry("idle", 2, 60, 0, now),
        ];
        // The playing torrent alone is over the limit; only the idle one can go.
        assert_eq!(evictions(&entries, 10 * GB, now), ["idle"]);
    }

    #[test]
    fn recently_used_counts_as_in_use() {
        let now = Instant::now() + Duration::from_secs(3 * 3600);
        let entries = [
            entry("just-seeked", 6, 1, 0, now),
            entry("old", 6, 30, 0, now),
        ];
        assert_eq!(evictions(&entries, 5 * GB, now), ["old"]);
    }
}

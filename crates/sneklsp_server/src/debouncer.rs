use std::collections::HashMap;
use std::time::{Duration, Instant};

use lsp_types::Uri;

const DEFAULT_DEBOUNCE_MS: u64 = 50;

pub struct Debouncer {
    pending: HashMap<Uri, PendingWork>,
    delay: Duration,
}

struct PendingWork {
    scheduled_at: Instant,
    version: i32,
}

impl Debouncer {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            delay: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
        }
    }

    #[inline]
    pub fn with_delay_ms(ms: u64) -> Self {
        Self {
            pending: HashMap::new(),
            delay: Duration::from_millis(ms),
        }
    }

    pub fn schedule(&mut self, uri: Uri, version: i32) {
        self.pending.insert(
            uri,
            PendingWork {
                scheduled_at: Instant::now(),
                version,
            },
        );
    }

    pub fn cancel(&mut self, uri: &Uri) {
        self.pending.remove(uri);
    }

    pub fn take_ready(&mut self) -> Vec<(Uri, i32)> {
        let now = Instant::now();
        let mut ready = Vec::new();

        self.pending.retain(|uri, work| {
            if now.duration_since(work.scheduled_at) >= self.delay {
                ready.push((uri.clone(), work.version));
                false
            } else {
                true
            }
        });

        ready
    }
}

impl Default for Debouncer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn uri(s: &str) -> Uri {
        s.parse().unwrap()
    }

    #[test]
    fn no_ready_immediately() {
        let mut deb = Debouncer::with_delay_ms(100);
        deb.schedule(uri("file:///a.py"), 1);
        assert!(deb.take_ready().is_empty());
    }

    #[test]
    fn ready_after_delay() {
        let mut deb = Debouncer::with_delay_ms(10);
        deb.schedule(uri("file:///a.py"), 1);

        sleep(Duration::from_millis(15));

        let ready = deb.take_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].1, 1);
    }

    #[test]
    fn reschedule_resets_timer() {
        let mut debouncer = Debouncer::with_delay_ms(50);
        debouncer.schedule(uri("file:///a.py"), 1);

        sleep(Duration::from_millis(30));
        debouncer.schedule(uri("file:///a.py"), 2);

        sleep(Duration::from_millis(30));
        assert!(debouncer.take_ready().is_empty());

        sleep(Duration::from_millis(25));
        let ready = debouncer.take_ready();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].1, 2);
    }
}

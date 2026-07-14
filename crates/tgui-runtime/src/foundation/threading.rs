use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const JOIN_POLL_INTERVAL: Duration = Duration::from_millis(2);

pub(crate) fn join_with_timeout(handle: JoinHandle<()>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;

    loop {
        if handle.is_finished() {
            let _ = handle.join();
            return true;
        }

        let now = Instant::now();
        if now >= deadline {
            return false;
        }

        thread::sleep(JOIN_POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_with_timeout_joins_finished_thread() {
        let handle = thread::spawn(|| {});

        assert!(join_with_timeout(handle, Duration::from_millis(50)));
    }

    #[test]
    fn join_with_timeout_detaches_unfinished_thread() {
        let handle = thread::spawn(|| thread::sleep(Duration::from_millis(50)));

        assert!(!join_with_timeout(handle, Duration::from_millis(1)));
    }
}

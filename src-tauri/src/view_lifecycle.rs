//! Monotonic leases for unused webviews. The native control window is independent.
pub const IDLE_MS: u64 = 5 * 60_000;
#[derive(Clone, Copy, Default)]
pub struct Lease {
    hidden_since: Option<u64>,
    retiring: bool,
}
impl Lease {
    pub fn update(&mut self, exists: bool, needed: bool, now: u64) {
        if !exists {
            if !self.retiring {
                self.hidden_since = None;
            }
            return;
        }
        if needed {
            self.hidden_since = None;
        } else if !self.retiring {
            self.hidden_since.get_or_insert(now);
        }
    }
    pub fn deadline(&self) -> Option<u64> {
        (!self.retiring)
            .then_some(self.hidden_since)
            .flatten()
            .map(|at| at.saturating_add(IDLE_MS))
    }
    pub fn take_due(&mut self, now: u64) -> bool {
        if self.deadline().is_some_and(|at| now >= at) {
            self.retiring = true;
            true
        } else {
            false
        }
    }
    pub fn retiring(&self) -> bool {
        self.retiring
    }
    pub fn destroyed(&mut self) {
        *self = Self::default();
    }
    pub fn failed(&mut self, now: u64) {
        self.retiring = false;
        self.hidden_since = Some(now);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_an_existing_unused_view_expires_after_five_minutes() {
        let mut l = Lease::default();
        l.update(false, false, 0);
        assert_eq!(l.deadline(), None);
        l.update(true, false, 1000);
        l.update(true, false, 10_000);
        assert_eq!(l.deadline(), Some(301_000));
        assert!(!l.take_due(300_999));
        assert!(l.take_due(301_000));
        assert!(!l.take_due(400_000));
    }
    #[test]
    fn reopen_cancels_idle_but_waits_for_an_in_progress_destroy() {
        let mut l = Lease::default();
        l.update(true, false, 0);
        l.update(true, true, IDLE_MS - 1);
        assert!(!l.take_due(IDLE_MS));
        l.update(true, false, IDLE_MS);
        assert!(l.take_due(2 * IDLE_MS));
        l.update(true, true, 2 * IDLE_MS + 1);
        assert!(l.retiring());
        l.destroyed();
        assert!(!l.retiring());
        assert_eq!(l.deadline(), None);
    }
    #[test]
    fn failed_retirement_retries_after_another_idle_interval() {
        let mut l = Lease::default();
        l.update(true, false, 0);
        assert!(l.take_due(IDLE_MS));
        l.failed(IDLE_MS);
        assert_eq!(l.deadline(), Some(2 * IDLE_MS));
    }
}

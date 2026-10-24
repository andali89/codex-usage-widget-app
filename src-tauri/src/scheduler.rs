use std::collections::{HashMap, VecDeque};

pub struct ResetObservation {
    pub key: String,
    pub deadline: Option<i64>,
    pub remaining: Option<f64>,
    pub highest_threshold: u8,
}
struct ResetCheck {
    deadline: Option<i64>,
    remaining: Option<f64>,
    low_observed: bool,
    recovery_since: Option<i64>,
    checks: u8,
    next_due: Option<u64>,
    active: bool,
    last_seen: u64,
}
fn reset_due(deadline: Option<i64>, wall: i64, now: u64) -> Option<u64> {
    deadline
        .map(|at| now.saturating_add(at.saturating_add(2000).saturating_sub(wall).max(0) as u64))
}
impl ResetCheck {
    fn new(observation: &ResetObservation, wall: i64, now: u64) -> Self {
        Self {
            deadline: observation.deadline,
            remaining: observation.remaining,
            low_observed: observation
                .remaining
                .is_some_and(|r| r <= f64::from(observation.highest_threshold)),
            recovery_since: None,
            checks: 0,
            next_due: reset_due(observation.deadline, wall, now),
            active: observation.deadline.is_some(),
            last_seen: now,
        }
    }
    fn observe(&mut self, incoming: &ResetObservation, wall: i64, now: u64) {
        let known_cycle = self.deadline.is_some_and(|old| {
            wall >= old
                && incoming
                    .deadline
                    .is_some_and(|next| next > wall && next > old)
                && self
                    .remaining
                    .zip(incoming.remaining)
                    .is_some_and(|(old_r, r)| r > old_r)
        });
        let sustained = if self.low_observed
            && incoming.remaining.is_some_and(|r| {
                r >= f64::from(incoming.highest_threshold.saturating_add(5).min(100))
            }) {
            let first = *self.recovery_since.get_or_insert(wall);
            wall.saturating_sub(first) >= 30_000
        } else {
            self.recovery_since = None;
            false
        };
        if known_cycle || sustained {
            *self = Self::new(incoming, wall, now);
            return;
        }
        if incoming.deadline.is_some() && self.deadline != incoming.deadline {
            self.deadline = incoming.deadline;
            // A correction reschedules the remaining budget; it never rearms it.
            self.next_due = if self.checks < 3 {
                reset_due(incoming.deadline, wall, now)
            } else {
                None
            };
        }
        self.active = incoming.deadline.is_some();
        self.remaining = incoming.remaining;
        self.last_seen = now;
        self.low_observed |= incoming
            .remaining
            .is_some_and(|r| r <= f64::from(incoming.highest_threshold));
    }
}

// Pure policy in monotonic milliseconds, so tests advance a fake clock without sleeping.
pub struct Scheduler {
    pub in_flight: bool,
    pub due_ms: Option<u64>,
    last_start_ms: Option<u64>,
    failures: usize,
    reset_checks: HashMap<String, ResetCheck>,
    entropy: u64,
    unexpected_exits: VecDeque<u64>,
    circuit_open: bool,
}

impl Scheduler {
    pub fn new(seed: u64) -> Self {
        Self {
            in_flight: false,
            due_ms: Some(0),
            last_start_ms: None,
            failures: 0,
            reset_checks: HashMap::new(),
            entropy: seed.max(1),
            unexpected_exits: VecDeque::new(),
            circuit_open: false,
        }
    }
    fn minimum(&self) -> u64 {
        self.last_start_ms.map_or(0, |n| n.saturating_add(5000))
    }
    pub fn request(&mut self, now: u64) {
        // Manual/open requests join the current read. Only an invalidating hint
        // (finish(dirty=true)) can require a coalesced follow-up.
        if !self.in_flight && !self.circuit_open {
            self.due_ms = Some(self.due_ms.unwrap_or(u64::MAX).min(now.max(self.minimum())));
        }
    }
    pub fn retry(&mut self, now: u64) {
        self.circuit_open = false;
        self.unexpected_exits.clear();
        self.failures = 0;
        self.request(now);
    }
    pub fn unexpected_exit(&mut self, now: u64) -> bool {
        while self
            .unexpected_exits
            .front()
            .is_some_and(|at| now.saturating_sub(*at) > 300_000)
        {
            self.unexpected_exits.pop_front();
        }
        self.unexpected_exits.push_back(now);
        if self.unexpected_exits.len() >= 5 {
            self.circuit_open = true;
            self.due_ms = None;
            self.reset_checks.clear();
        }
        self.circuit_open
    }
    /// Automatic reminder reconciliation respects existing backoff/auth stop.
    pub fn reconcile(&mut self, now: u64) {
        if self.failures == 0 && self.due_ms.is_some() {
            self.request(now);
        }
    }
    pub fn start_if_due(&mut self, now: u64) -> bool {
        if self.circuit_open {
            return false;
        }
        let reset_due = self
            .reset_checks
            .values()
            .filter(|c| c.active)
            .filter_map(|c| c.next_due)
            .min();
        let due = [self.due_ms, reset_due].into_iter().flatten().min();
        if self.in_flight || due.is_none_or(|n| now < n.max(self.minimum())) {
            return false;
        }
        for check in self.reset_checks.values_mut() {
            if check.active && check.next_due.is_some_and(|n| now >= n) {
                check.checks += 1;
                check.next_due = match check.checks {
                    1 => Some(now + 15000),
                    2 => Some(now + 60000),
                    _ => None,
                };
            }
        }
        self.in_flight = true;
        self.due_ms = None;
        self.last_start_ms = Some(now);
        true
    }
    pub fn finish(
        &mut self,
        now: u64,
        success: bool,
        permanent: bool,
        interval_ms: u64,
        dirty: bool,
    ) {
        self.in_flight = false;
        if permanent || self.circuit_open {
            self.due_ms = None;
            self.reset_checks.clear();
            return;
        }
        let delay = if success {
            self.failures = 0;
            self.jitter(interval_ms, 10)
        } else {
            let base = [5000, 15000, 30000, 60000, 120000, 300000][self.failures.min(5)];
            self.failures += 1;
            self.jitter(base, 20)
        };
        self.due_ms = Some(now + delay);
        if dirty {
            self.request(now);
        }
    }
    pub fn set_cadence(&mut self, now: u64, interval_ms: u64) {
        if !self.in_flight && self.failures == 0 && self.due_ms.is_some() {
            self.due_ms = Some(
                (self.last_start_ms.unwrap_or(now) + self.jitter(interval_ms, 10))
                    .max(now)
                    .max(self.minimum()),
            );
        }
    }
    pub fn update_deadlines(
        &mut self,
        deadlines: impl Iterator<Item = ResetObservation>,
        wall_ms: i64,
        now: u64,
    ) {
        let deadlines = deadlines.collect::<Vec<_>>();
        for check in self.reset_checks.values_mut() {
            check.active = false;
        }
        for observation in &deadlines {
            if let Some(check) = self.reset_checks.get_mut(&observation.key) {
                check.observe(observation, wall_ms, now);
            } else {
                if self.reset_checks.len() >= 128 {
                    if let Some(oldest) = self
                        .reset_checks
                        .iter()
                        .filter(|(key, _)| !deadlines.iter().any(|d| &d.key == *key))
                        .min_by_key(|(_, c)| c.last_seen)
                        .map(|(key, _)| key.clone())
                    {
                        self.reset_checks.remove(&oldest);
                    }
                }
                self.reset_checks.insert(
                    observation.key.clone(),
                    ResetCheck::new(observation, wall_ms, now),
                );
            }
        }
    }
    pub fn rebind_reset_context(&mut self) {
        self.reset_checks.clear();
    }
    pub fn reconcile_clock(&mut self, wall: i64, now: u64) {
        for check in self.reset_checks.values_mut().filter(|c| c.checks == 0) {
            check.next_due = reset_due(check.deadline, wall, now);
        }
    }
    pub fn next_deadline(&self) -> Option<u64> {
        if self.in_flight {
            return None;
        }
        self.due_ms
            .into_iter()
            .chain(
                self.reset_checks
                    .values()
                    .filter(|c| c.active)
                    .filter_map(|c| c.next_due),
            )
            .min()
            .map(|n| n.max(self.minimum()))
    }
    fn jitter(&mut self, base: u64, percent: u64) -> u64 {
        self.entropy ^= self.entropy << 13;
        self.entropy ^= self.entropy >> 7;
        self.entropy ^= self.entropy << 17;
        let range = base * percent / 100;
        base - range + self.entropy % (range * 2 + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reset(deadline: i64, remaining: f64) -> ResetObservation {
        ResetObservation {
            key: "window".into(),
            deadline: Some(deadline),
            remaining: Some(remaining),
            highest_threshold: 20,
        }
    }
    #[test]
    fn overlaps_join_one_read_and_minimum_spacing() {
        let mut s = Scheduler::new(42);
        assert!(s.start_if_due(0));
        for _ in 0..20 {
            s.request(1);
            assert!(!s.start_if_due(1));
        }
        s.finish(1000, true, false, 120000, false);
        assert!(!s.start_if_due(5000));
        s.request(5000);
        assert!(s.start_if_due(5000));
        s.finish(6000, true, false, 120000, false);
        assert!(!s.start_if_due(10000));
    }
    #[test]
    fn reminder_reconciliation_respects_backoff_and_auth_stop() {
        let mut s = Scheduler::new(42);
        assert!(s.start_if_due(0));
        s.finish(1000, false, false, 120000, false);
        let due = s.next_deadline();
        s.reconcile(2000);
        assert_eq!(s.next_deadline(), due);
        assert!(s.start_if_due(due.unwrap()));
        s.finish(20_000, false, true, 120000, false);
        s.reconcile(30_000);
        assert_eq!(s.next_deadline(), None);
    }
    #[test]
    fn five_unexpected_exits_open_circuit_until_explicit_retry() {
        let mut s = Scheduler::new(42);
        for i in 0..5 {
            assert_eq!(s.unexpected_exit(i * 60_000), i == 4);
        }
        assert_eq!(s.next_deadline(), None);
        s.request(300_000);
        s.reconcile(300_000);
        s.set_cadence(300_000, 30_000);
        assert!(!s.start_if_due(300_000));
        s.finish(300_000, true, false, 30_000, true);
        assert_eq!(s.next_deadline(), None);
        s.retry(300_000);
        assert!(s.start_if_due(300_000));
        assert!(!s.unexpected_exit(300_001));
    }
    #[test]
    fn circuit_uses_a_rolling_monotonic_five_minute_window() {
        let mut s = Scheduler::new(42);
        for i in 0..4 {
            assert!(!s.unexpected_exit(i * 60_000));
        }
        assert!(!s.unexpected_exit(300_001)); // the first exit has aged out
        assert!(s.unexpected_exit(300_002));
        s.retry(400_000);
        assert!(!s.unexpected_exit(400_000));
    }
    #[test]
    fn authentication_stops_polling_until_explicit_retry() {
        let mut s = Scheduler::new(1);
        s.start_if_due(0);
        s.finish(1, false, true, 120000, false);
        assert!(!s.start_if_due(1_000_000));
        s.request(1_000_000);
        assert!(s.start_if_due(1_000_000));
    }
    #[test]
    fn unchanged_reset_has_only_three_checks() {
        let mut s = Scheduler::new(1);
        s.due_ms = None;
        s.update_deadlines([reset(1000, 0.0)].into_iter(), 0, 0);
        assert!(!s.start_if_due(2999));
        for at in [3000, 18000, 78000] {
            assert!(s.start_if_due(at));
            s.finish(at, true, false, 600000, false);
            s.update_deadlines([reset(1000, 0.0)].into_iter(), at as i64, at);
        }
        assert!(!s.start_if_due(79000));
        assert_eq!(s.reset_checks["window"].checks, 3);
    }
    #[test]
    fn corrections_and_missing_windows_do_not_rearm_an_exhausted_episode() {
        let mut s = Scheduler::new(42);
        s.due_ms = None;
        s.update_deadlines([reset(1000, 0.0)].into_iter(), 0, 0);
        for at in [3000, 18_000, 78_000] {
            assert!(s.start_if_due(at));
            s.finish(at, true, false, 600_000, false);
        }
        assert_eq!(s.reset_checks["window"].checks, 3);
        s.update_deadlines([reset(2000, 0.0)].into_iter(), 90_000, 90_000);
        assert_eq!(s.reset_checks["window"].checks, 3);
        assert!(s.reset_checks["window"].next_due.is_none());
        s.update_deadlines(std::iter::empty(), 100_000, 100_000);
        s.update_deadlines([reset(3000, 0.0)].into_iter(), 110_000, 110_000);
        assert_eq!(s.reset_checks["window"].checks, 3);
        assert!(s.reset_checks["window"].next_due.is_none());
        s.update_deadlines([reset(200_000, 80.0)].into_iter(), 120_000, 120_000);
        assert_eq!(s.reset_checks["window"].checks, 0);
        assert_eq!(s.reset_checks["window"].next_due, Some(202_000));
    }
    #[test]
    fn a_deadline_correction_reschedules_only_the_remaining_budget() {
        let mut s = Scheduler::new(42);
        s.due_ms = None;
        s.update_deadlines([reset(1000, 0.0)].into_iter(), 0, 0);
        assert!(s.start_if_due(3000));
        s.finish(3000, true, false, 600_000, false);
        s.update_deadlines([reset(20_000, 0.0)].into_iter(), 4000, 4000);
        assert_eq!(s.reset_checks["window"].checks, 1);
        assert_eq!(s.reset_checks["window"].next_due, Some(22_000));
        s.update_deadlines(std::iter::empty(), 5000, 5000);
        assert!(!s.start_if_due(22_000));
        s.update_deadlines([reset(20_000, 0.0)].into_iter(), 23_000, 23_000);
        assert!(s.start_if_due(23_000));
        assert_eq!(s.reset_checks["window"].checks, 2);
        s.rebind_reset_context();
        assert!(s.reset_checks.is_empty());
    }
    #[test]
    fn sustained_recovery_with_no_new_reset_timestamp_can_confirm_a_cycle() {
        let mut s = Scheduler::new(42);
        s.update_deadlines([reset(1000, 0.0)].into_iter(), 0, 0);
        s.reset_checks.get_mut("window").unwrap().checks = 3;
        s.update_deadlines([reset(1000, 25.0)].into_iter(), 30_000, 30_000);
        assert_eq!(s.reset_checks["window"].checks, 3);
        s.update_deadlines([reset(1000, 25.0)].into_iter(), 59_999, 59_999);
        assert_eq!(s.reset_checks["window"].checks, 3);
        s.update_deadlines([reset(1000, 25.0)].into_iter(), 60_000, 60_000);
        assert_eq!(s.reset_checks["window"].checks, 0);
    }
    #[test]
    fn clock_jumps_recompute_future_utc_deadlines_without_rearming_consumed_checks() {
        let mut s = Scheduler::new(42);
        s.due_ms = None;
        s.update_deadlines([reset(100_000, 0.0)].into_iter(), 0, 0);
        s.reconcile_clock(200_000, 10_000);
        assert_eq!(s.reset_checks["window"].next_due, Some(10_000));
        assert!(s.start_if_due(10_000));
        s.finish(10_000, true, false, 600_000, false);
        let next = s.reset_checks["window"].next_due;
        s.reconcile_clock(0, 11_000);
        assert_eq!(s.reset_checks["window"].next_due, next);
        s.reset_checks.get_mut("window").unwrap().checks = 3;
        s.reset_checks.get_mut("window").unwrap().next_due = None;
        s.reconcile_clock(500_000, 12_000);
        assert!(s.reset_checks["window"].next_due.is_none());
    }
    #[test]
    fn backoff_bounds_and_dirty_reconciliation() {
        let mut s = Scheduler::new(2);
        for base in [5000, 15000, 30000, 60000, 120000, 300000, 300000] {
            s.finish(0, false, false, 120000, false);
            assert!((base * 8 / 10..=base * 12 / 10).contains(&s.due_ms.unwrap()));
        }
        s.finish(1000000, true, false, 120000, true);
        assert_eq!(s.due_ms, Some(1000000));
    }
    #[test]
    fn cadence_changes_preserve_auth_stop_and_transient_backoff() {
        let mut s = Scheduler::new(2);
        s.start_if_due(0);
        s.finish(100, true, false, 120000, false);
        s.set_cadence(200, 30000);
        assert!((27000..=33000).contains(&s.due_ms.unwrap()));
        s.finish(200, false, false, 120000, false);
        let backoff = s.due_ms;
        s.set_cadence(300, 30000);
        assert_eq!(s.due_ms, backoff);
        s.finish(300, false, true, 120000, false);
        s.set_cadence(400, 30000);
        assert_eq!(s.due_ms, None);
    }
}

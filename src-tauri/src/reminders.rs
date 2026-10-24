//! Serialized reminder policy. No native submission is allowed before `claim` is committed.
//! The clock, local calendar and transport are inputs so tests never send notifications.
use crate::model::Usage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;
const GRACE: i64 = 15 * MINUTE;
const LAST_TIME: i64 = 253_402_300_799_000;
fn valid_time(t: i64) -> bool {
    (0..=LAST_TIME).contains(&t)
}
fn valid_candidate(c: &Candidate) -> bool {
    c.rule_key.len() <= 1024
        && c.binding.as_ref().is_none_or(|b| b.len() <= 512)
        && c.label.len() <= 256
        && (1..=5).contains(&c.event_keys.len())
        && c.event_keys.iter().all(|s| s.len() <= 1024)
        && c.thresholds.len() <= 5
        && c.thresholds.iter().all(|t| *t <= 99)
        && c.remaining
            .is_none_or(|r| r.is_finite() && (0.0..=100.0).contains(&r))
        && valid_time(c.eligible_at)
        && valid_time(c.observed_at)
        && c.expires_at.is_none_or(valid_time)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Rule {
    pub low: bool,
    pub thresholds: Vec<u8>,
    pub approaching: bool,
    pub lead_minutes: u8,
    pub recovery: bool,
}
impl Default for Rule {
    fn default() -> Self {
        Self {
            low: true,
            thresholds: vec![20, 10, 0],
            approaching: false,
            lead_minutes: 5,
            recovery: true,
        }
    }
}
impl Rule {
    fn validate(&self) -> bool {
        self.thresholds.len() <= 5
            && self.thresholds.iter().all(|n| *n <= 99)
            && self.thresholds.windows(2).all(|p| p[0] > p[1])
            && (1..=60).contains(&self.lead_minutes)
    }
    fn highest(&self) -> u8 {
        self.thresholds.first().copied().unwrap_or(20)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct QuietHours {
    pub enabled: bool,
    pub start_minute: u16,
    pub end_minute: u16,
    pub weekdays: Vec<u8>,
}
impl Default for QuietHours {
    fn default() -> Self {
        Self {
            enabled: false,
            start_minute: 1320,
            end_minute: 420,
            weekdays: (0..7).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Schedule {
    pub id: String,
    pub revision: u64,
    pub enabled: bool,
    pub label: String,
    pub timing: Timing,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Timing {
    Once { at_ms: i64 },
    // Recurrences deliberately follow Windows. Fixed IANA zones are optional in the spec.
    Weekly { minute: u16, weekdays: Vec<u8> },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Preferences {
    pub enabled: bool,
    pub defaults: Rule,
    // Keys bind the opaque account context and stable schema window key, never a label.
    pub window_rules: BTreeMap<String, Rule>,
    pub quiet_hours: QuietHours,
    pub schedules: Vec<Schedule>,
    pub silent: bool,
    pub detailed: bool,
    pub suppress_fullscreen: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            defaults: Rule::default(),
            window_rules: BTreeMap::new(),
            quiet_hours: QuietHours::default(),
            schedules: Vec::new(),
            silent: true,
            detailed: false,
            suppress_fullscreen: true,
        }
    }
}
fn weekdays_valid(days: &[u8]) -> bool {
    !days.is_empty()
        && days.len() <= 7
        && days.iter().all(|d| *d < 7)
        && days.iter().collect::<BTreeSet<_>>().len() == days.len()
}
impl Preferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.defaults.validate()
            || self.window_rules.len() > 128
            || self
                .window_rules
                .iter()
                .any(|(k, r)| k.len() > 512 || !r.validate())
        {
            return Err("invalid-reminder-rule");
        }
        let q = &self.quiet_hours;
        if q.start_minute >= 1440
            || q.end_minute >= 1440
            || (q.enabled && (q.start_minute == q.end_minute || !weekdays_valid(&q.weekdays)))
        {
            return Err("invalid-quiet-hours");
        }
        let mut ids = BTreeSet::new();
        if self.schedules.len() > 20
            || self.schedules.iter().any(|s| {
                s.id.is_empty()
                    || s.id.len() > 80
                    || !s.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                    || !ids.insert(&s.id)
                    || s.label.len() > 120
                    || s.label.contains(['\0', '\n', '\r'])
                    || match &s.timing {
                        Timing::Once { at_ms } => !(0..=253_402_300_799_000).contains(at_ms),
                        Timing::Weekly { minute, weekdays } => {
                            *minute >= 1440 || !weekdays_valid(weekdays)
                        }
                    }
            })
        {
            return Err("invalid-reminder-schedule");
        }
        Ok(())
    }
    pub fn validate_edit(&self, old: &Self, now: i64) -> Result<(), &'static str> {
        self.validate()?;
        for s in &self.schedules {
            if let Some(previous) = old.schedules.iter().find(|p| p.id == s.id) {
                if previous != s && s.revision <= previous.revision {
                    return Err("schedule-revision-conflict");
                }
            }
            if let Timing::Once { at_ms } = s.timing {
                if s.enabled && at_ms <= now && !old.schedules.iter().any(|p| p == s) {
                    return Err("schedule-must-be-future");
                }
            }
        }
        Ok(())
    }
    fn rule(&self, binding: &str) -> &Rule {
        self.window_rules.get(binding).unwrap_or(&self.defaults)
    }
}

/// Local day is Gregorian days since 1970-01-01; weekday is Monday=0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalMinute {
    pub day: i64,
    pub minute: u16,
}
impl LocalMinute {
    pub fn weekday(self) -> u8 {
        (self.day + 3).rem_euclid(7) as u8
    }
    fn ordinal(self) -> i64 {
        self.day * 1440 + i64::from(self.minute)
    }
}
pub fn quiet_at(q: &QuietHours, local: LocalMinute) -> bool {
    if !q.enabled {
        return false;
    }
    let day = local.weekday();
    if q.start_minute < q.end_minute {
        q.weekdays.contains(&day) && (q.start_minute..q.end_minute).contains(&local.minute)
    } else if local.minute >= q.start_minute {
        q.weekdays.contains(&day)
    } else {
        local.minute < q.end_minute && q.weekdays.contains(&((day + 6) % 7))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Episode {
    pub number: u64,
    pub handled: BTreeSet<u8>,
    pub approaching_handled: bool,
    pub low_observed: bool,
    pub remaining: Option<f64>,
    pub deadline: Option<i64>,
    pub recovery_since: Option<i64>,
    pub observed_at: i64,
    pub rebase: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Low,
    Approaching,
    Recovery,
    Availability,
    Scheduled,
    Test,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Candidate {
    pub rule_key: String,
    pub event_keys: Vec<String>,
    pub category: Category,
    pub binding: Option<String>,
    pub episode: u64,
    pub thresholds: Vec<u8>,
    pub label: String,
    pub remaining: Option<f64>,
    pub eligible_at: i64,
    pub observed_at: i64,
    pub expires_at: Option<i64>,
    pub snapshot_revision: u64,
    pub suppression: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Claimed,
    Submitted,
    Failed,
    OutcomeUnknown,
    Missed,
    Cancelled,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Delivery {
    pub id: u64,
    pub events: Vec<Candidate>,
    pub at_ms: i64,
    pub status: Status,
    pub reason: Option<String>,
    pub attempts: u8,
    pub retry_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Durable {
    pub account_salt: Option<String>,
    pub episodes: BTreeMap<String, Episode>,
    pub pending: BTreeMap<String, Candidate>,
    pub history: Vec<Delivery>,
    // History display can be cleared without touching these high-water marks or episodes.
    pub schedule_high_water: BTreeMap<String, i64>,
    pub schedule_scanned_at: Option<i64>,
    pub snoozed_until: Option<i64>,
    pub rule_snoozes: BTreeMap<String, i64>,
    pub sequence: u64,
    pub last_claim_at: Option<i64>,
    pub delivery_paused: bool,
}
impl Durable {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self
            .account_salt
            .as_ref()
            .is_some_and(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("reminder-storage-invalid");
        }
        if self.episodes.len() > 512
            || self.pending.len() > 1024
            || self.history.len() > 1000
            || self.rule_snoozes.len() > 1024
            || self.schedule_high_water.len() > 1000
        {
            return Err("reminder-storage-limit");
        }
        if self.episodes.values().any(|e| {
            e.remaining
                .is_some_and(|r| !r.is_finite() || !(0.0..=100.0).contains(&r))
                || e.handled.iter().any(|t| *t > 99)
                || !valid_time(e.observed_at)
                || e.deadline.is_some_and(|t| !valid_time(t))
                || e.recovery_since.is_some_and(|t| !valid_time(t))
                || e.number > 1_000_000_000
        }) {
            return Err("reminder-storage-invalid");
        }
        if self.sequence > u64::MAX - 1024
            || self.episodes.keys().any(|k| k.len() > 512)
            || self.pending.values().any(|c| !valid_candidate(c))
            || self.history.iter().any(|d| {
                !valid_time(d.at_ms)
                    || d.attempts > 3
                    || d.events.len() > 1024
                    || d.events.iter().any(|c| !valid_candidate(c))
                    || d.retry_at.is_some_and(|t| !valid_time(t))
            })
            || self
                .schedule_high_water
                .iter()
                .any(|(k, t)| k.len() > 256 || !valid_time(*t))
            || self
                .rule_snoozes
                .iter()
                .any(|(k, t)| k.len() > 1024 || !valid_time(*t))
            || [
                self.last_claim_at,
                self.schedule_scanned_at,
                self.snoozed_until,
            ]
            .into_iter()
            .flatten()
            .any(|t| !valid_time(t))
        {
            return Err("reminder-storage-invalid");
        }
        Ok(())
    }
    pub fn recover(&mut self, now: i64) {
        self.delivery_paused = true;
        self.pending.clear();
        self.schedule_scanned_at = Some(now);
        for episode in self.episodes.values_mut() {
            episode.rebase = true;
        }
    }
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
    fn prune(&mut self, now: i64) {
        self.history.retain(|d| d.at_ms >= now - 30 * DAY);
        if self.history.len() > 1000 {
            self.history.drain(..self.history.len() - 1000);
        }
        self.episodes.retain(|_, e| e.observed_at >= now - 30 * DAY);
        self.rule_snoozes.retain(|_, until| *until > now);
    }
}

#[derive(Clone)]
pub struct Engine {
    pub data: Durable,
    current_context: Option<String>,
    silent_initial: bool,
    seen_context: bool,
    last_allowed: Option<bool>,
    last_sample: Option<(String, i64)>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub delivery_paused: bool,
    pub snoozed_until: Option<i64>,
    pub pending_count: usize,
    pub history: Vec<Delivery>,
    pub diagnostic: Option<String>,
    pub identity_scope: String,
}
#[derive(Clone, Copy, Default)]
pub struct Suppression {
    pub locked: bool,
    pub fullscreen: bool,
}
// Queued plugin calls remain unknown until an actual OS result can be established.
#[allow(dead_code)]
pub enum Outcome {
    Submitted,
    DefinitelyNotSubmitted,
    Unknown,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Snooze {
        choice: SnoozeChoice,
        rule: Option<String>,
    },
    ClearHistory,
    Test,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnoozeChoice {
    FifteenMinutes,
    OneHour,
    TomorrowNine,
    Resume,
}

pub fn binding(account: &str, window: &str) -> String {
    format!("{account}|{window}")
}
impl Engine {
    pub fn mark_stale(&mut self) {
        self.last_sample = None;
    }
    pub fn invalidate_context(&mut self) {
        self.data
            .pending
            .retain(|_, c| c.category == Category::Scheduled);
        self.current_context = None;
        self.last_sample = None;
        self.last_allowed = None;
    }
    pub fn snapshot(&self, diagnostic: Option<String>) -> Snapshot {
        let history = self
            .data
            .history
            .iter()
            .filter_map(|d| {
                let mut visible = d.clone();
                visible.events.retain(|c| {
                    matches!(c.category, Category::Scheduled | Category::Test)
                        || self.current_context.as_ref().is_some_and(|ctx| {
                            c.binding
                                .as_ref()
                                .is_some_and(|b| b.starts_with(&format!("{ctx}|")))
                                || c.category == Category::Availability
                                    && c.rule_key.starts_with(&format!("{ctx}/"))
                        })
                });
                (!visible.events.is_empty()).then_some(visible)
            })
            .collect();
        Snapshot {
            delivery_paused: self.data.delivery_paused,
            snoozed_until: self.data.snoozed_until,
            pending_count: self.data.pending.len(),
            history,
            diagnostic,
            identity_scope:
                "Account ID when provided; silent restart baseline; raw identifiers never stored"
                    .into(),
        }
    }
    pub fn next_wake(&self, now: i64) -> i64 {
        let mut next = (now.div_euclid(MINUTE) + 1) * MINUTE;
        for at in [
            self.data
                .pending
                .values()
                .map(|c| c.eligible_at + 5000)
                .min(),
            self.data.last_claim_at.map(|n| n + MINUTE),
            self.data.snoozed_until,
        ]
        .into_iter()
        .flatten()
        {
            if at > now {
                next = next.min(at);
            }
        }
        for at in self
            .data
            .history
            .iter()
            .filter_map(|d| d.retry_at)
            .chain(self.data.rule_snoozes.values().copied())
            .chain(self.data.pending.values().filter_map(|c| c.expires_at))
        {
            if at > now {
                next = next.min(at);
            }
        }
        next
    }
    pub fn needs_revalidation(&self, now: i64) -> bool {
        self.data.pending.values().any(|c| {
            c.category != Category::Scheduled
                && c.suppression.is_none()
                && self
                    .last_sample
                    .as_ref()
                    .is_none_or(|(_, at)| now - at > MINUTE)
        })
    }
    pub fn new(mut data: Durable, enabled_at_start: bool) -> Self {
        // A claimed attempt may have reached Windows. A restart must never retry it.
        for d in &mut data.history {
            if d.status == Status::Claimed {
                d.status = Status::OutcomeUnknown;
                d.reason = Some("interrupted-after-claim".into());
                d.retry_at = None;
            }
        }
        data.pending
            .retain(|_, c| c.category == Category::Scheduled);
        Self {
            data,
            current_context: None,
            silent_initial: enabled_at_start,
            seen_context: false,
            last_allowed: None,
            last_sample: None,
        }
    }
    pub fn configure(
        &mut self,
        old: &Preferences,
        new: &Preferences,
        now: i64,
        local_now: Option<LocalMinute>,
    ) {
        if old == new {
            return;
        }
        self.data.pending.retain(|_, c| {
            if !new.enabled {
                return false;
            }
            if let Some(b) = &c.binding {
                old.rule(b) == new.rule(b)
            } else {
                old.schedules
                    .iter()
                    .filter(|s| s.enabled)
                    .any(|s| new.schedules.contains(s) && c.rule_key == schedule_key(s))
            }
        });
        for (key, episode) in &mut self.data.episodes {
            if old.rule(key) != new.rule(key) || !old.enabled && new.enabled {
                episode.rebase = true;
            }
        }
        if self.data.delivery_paused && !old.enabled && new.enabled {
            self.data.delivery_paused = false;
            self.data.schedule_scanned_at = Some(now);
            self.silent_initial = true;
            for s in &new.schedules {
                let high = match s.timing {
                    Timing::Once { at_ms } if at_ms <= now => Some(at_ms),
                    Timing::Weekly { .. } => local_now.map(LocalMinute::ordinal),
                    _ => None,
                };
                if let Some(high) = high {
                    self.data
                        .schedule_high_water
                        .entry(schedule_key(s))
                        .and_modify(|h| *h = (*h).max(high))
                        .or_insert(high);
                }
            }
        }
        // A disabled/edited schedule never replays the interval preceding its enablement.
        if old.schedules != new.schedules || !new.enabled || !old.enabled {
            self.data.schedule_scanned_at = Some(now);
        }
        self.last_sample = None;
        for d in &mut self.data.history {
            if d.retry_at.is_some()
                && (!new.enabled
                    || d.events.iter().any(|c| {
                        c.binding
                            .as_ref()
                            .is_some_and(|b| old.rule(b) != new.rule(b))
                            || c.category == Category::Scheduled
                                && !new
                                    .schedules
                                    .iter()
                                    .any(|s| s.enabled && c.rule_key == schedule_key(s))
                    }))
            {
                d.retry_at = None;
                d.status = Status::Cancelled;
                d.reason = Some("settings-changed".into());
            }
        }
    }
    pub fn observe(
        &mut self,
        prefs: &Preferences,
        usage: &Usage,
        identity_valid: bool,
        revision: u64,
        now: i64,
    ) {
        if !prefs.enabled || usage.freshness != "fresh" {
            return;
        }
        let Some(context) = usage
            .account_context_key
            .as_deref()
            .filter(|_| identity_valid)
        else {
            self.data
                .pending
                .retain(|_, c| c.category == Category::Scheduled);
            self.current_context = None;
            return;
        };
        let Some(at) = usage
            .last_success_at_ms
            .filter(|at| now >= *at && now - at <= MINUTE)
        else {
            return;
        };
        if self
            .last_sample
            .as_ref()
            .is_some_and(|(c, t)| c == context && *t == at)
        {
            return;
        }
        let changed = self.current_context.as_deref() != Some(context);
        let silent = changed && (self.silent_initial || self.seen_context);
        if changed {
            self.data
                .pending
                .retain(|_, c| c.category == Category::Scheduled);
            self.current_context = Some(context.into());
            self.last_allowed = None;
        }
        self.seen_context = true;
        self.last_sample = Some((context.into(), at));
        let present = usage
            .windows
            .iter()
            .filter(|w| w.quality == "valid" && !w.last_known && w.validated_at_ms == Some(at))
            .map(|w| binding(context, &w.key))
            .collect::<BTreeSet<_>>();
        self.data
            .pending
            .retain(|_, c| c.binding.as_ref().is_none_or(|b| present.contains(b)));
        let known_windows = self
            .data
            .episodes
            .keys()
            .any(|b| b.starts_with(&format!("{context}|")));
        for w in usage
            .windows
            .iter()
            .filter(|w| w.quality == "valid" && !w.last_known && w.validated_at_ms == Some(at))
        {
            let Some(remaining) = w
                .remaining_percent
                .filter(|r| r.is_finite() && (0.0..=100.0).contains(r))
            else {
                continue;
            };
            let b = binding(context, &w.key);
            let rule = prefs.rule(&b);
            let existing = self.data.episodes.contains_key(&b);
            let silent_baseline = silent || known_windows && !existing;
            let e = self.data.episodes.entry(b.clone()).or_default();
            let old_number = e.number;
            let deadline_cycle = e.deadline.is_some_and(|old| {
                at >= old
                    && w.resets_at_ms.is_some_and(|next| next > at && next > old)
                    && e.remaining.is_some_and(|old_r| remaining > old_r)
            });
            let recovery_line = f64::from(rule.highest().saturating_add(5).min(100));
            let sustained = if remaining >= recovery_line {
                let first = *e.recovery_since.get_or_insert(at);
                at - first >= 30_000 && e.low_observed
            } else {
                e.recovery_since = None;
                false
            };
            let cycle = existing && (deadline_cycle || sustained);
            let recovered_low = cycle && e.low_observed;
            if cycle {
                e.number += 1;
                e.handled.clear();
                e.approaching_handled = false;
                e.low_observed = false;
                e.recovery_since = None;
                self.data.pending.retain(|_, c| {
                    c.binding.as_ref() != Some(&b) || c.category == Category::Recovery
                });
            }
            let rebase = silent_baseline || e.rebase || cycle;
            let satisfied = rule
                .thresholds
                .iter()
                .copied()
                .filter(|t| remaining <= f64::from(*t))
                .collect::<Vec<_>>();
            if rebase {
                e.handled.extend(&satisfied);
                e.rebase = false;
            }
            // A silent initial low sample establishes monitoring but never fabricates recovery.
            if remaining <= f64::from(rule.highest()) && !silent_baseline {
                e.low_observed = true;
            }
            let thresholds = if rule.low {
                satisfied
                    .iter()
                    .copied()
                    .filter(|t| !e.handled.contains(t))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let number = e.number;
            let approach = rule.approaching
                && !e.approaching_handled
                && remaining <= f64::from(rule.highest())
                && w.resets_at_ms
                    .is_some_and(|d| d > at && d - at <= i64::from(rule.lead_minutes) * MINUTE);
            e.remaining = Some(remaining);
            e.deadline = w.resets_at_ms;
            e.observed_at = at;
            if let Some(recovery) = self.data.pending.get_mut(&format!("{b}/recovery")) {
                recovery.remaining = Some(remaining);
                recovery.snapshot_revision = revision;
            }
            if remaining < recovery_line {
                self.data.pending.remove(&format!("{b}/recovery"));
            }
            let make = |category: Category, episode: u64, thresholds: Vec<u8>, expires_at| {
                let tag = match category {
                    Category::Low => "low",
                    Category::Approaching => "approaching",
                    Category::Recovery => "recovery",
                    _ => unreachable!(),
                };
                let key = format!("{b}/{tag}");
                let event_keys = if category == Category::Low {
                    thresholds
                        .iter()
                        .map(|t| format!("{b}/{episode}/low/{t}"))
                        .collect()
                } else {
                    vec![format!("{b}/{episode}/{tag}")]
                };
                Candidate {
                    rule_key: key,
                    event_keys,
                    category,
                    binding: Some(b.clone()),
                    episode,
                    thresholds,
                    label: w.label.clone(),
                    remaining: Some(remaining),
                    eligible_at: now,
                    observed_at: at,
                    expires_at,
                    snapshot_revision: revision,
                    suppression: None,
                }
            };
            if !thresholds.is_empty() {
                self.queue(make(Category::Low, number, thresholds, None));
            } else {
                self.data.pending.remove(&format!("{b}/low"));
            }
            if approach && !silent_baseline {
                self.queue(make(
                    Category::Approaching,
                    number,
                    Vec::new(),
                    w.resets_at_ms,
                ));
            } else {
                self.data.pending.remove(&format!("{b}/approaching"));
            }
            if recovered_low && rule.recovery && !silent {
                self.queue(make(
                    Category::Recovery,
                    old_number,
                    Vec::new(),
                    Some(now + GRACE),
                ));
            }
        }
        if self.last_allowed == Some(false) && usage.ordinary_usage_allowed == Some(true) && !silent
        {
            let key = format!("{context}/availability/{at}");
            self.queue(Candidate {
                rule_key: format!("{context}/availability"),
                event_keys: vec![key],
                category: Category::Availability,
                binding: None,
                episode: 0,
                thresholds: Vec::new(),
                label: "Ordinary usage available again".into(),
                remaining: None,
                eligible_at: now,
                observed_at: at,
                expires_at: Some(now + GRACE),
                snapshot_revision: revision,
                suppression: None,
            });
        }
        self.last_allowed = usage.ordinary_usage_allowed;
        if self.last_allowed != Some(true) {
            self.data
                .pending
                .retain(|_, c| c.category != Category::Availability);
        }
    }
    fn queue(&mut self, mut c: Candidate) {
        if let Some(old) = self.data.pending.get(&c.rule_key) {
            c.eligible_at = old.eligible_at;
        }
        self.data.pending.insert(c.rule_key.clone(), c);
    }
    /// Sample local calendar minutes, not a fixed UTC offset. Forward DST gaps use the
    /// first observed valid minute; a repeated hour cannot pass the local high-water mark.
    pub fn schedules(
        &mut self,
        prefs: &Preferences,
        now: i64,
        local_at: impl Fn(i64) -> Option<LocalMinute>,
        revision: u64,
    ) {
        if !prefs.enabled || !prefs.schedules.iter().any(|s| s.enabled) {
            return;
        }
        let previous = self.data.schedule_scanned_at.unwrap_or(now - GRACE - 1);
        if now <= previous {
            return;
        }
        if self.data.schedule_scanned_at.is_none() || now - previous >= MINUTE {
            self.data.schedule_scanned_at = Some(now);
        }
        if !prefs.enabled {
            return;
        }
        for s in prefs.schedules.iter().filter(|s| s.enabled) {
            let key = schedule_key(s);
            match &s.timing {
                Timing::Once { at_ms } => {
                    if *at_ms <= now
                        && self
                            .data
                            .schedule_high_water
                            .get(&key)
                            .is_none_or(|h| *h < *at_ms)
                    {
                        self.data.schedule_high_water.insert(key.clone(), *at_ms);
                        let candidate = scheduled(s, &key, *at_ms, *at_ms, revision);
                        if now - *at_ms <= GRACE {
                            self.queue(candidate);
                        } else {
                            self.record_missed(candidate, now);
                        }
                    }
                }
                Timing::Weekly { minute, weekdays } => {
                    let start = previous.max(now - GRACE - MINUTE).div_euclid(MINUTE) * MINUTE;
                    let mut before = local_at(start - MINUTE);
                    for utc in (start..=now).step_by(MINUTE as usize) {
                        if let Some(local) = local_at(utc) {
                            let due = local.day * 1440 + i64::from(*minute);
                            let crossed = before.is_some_and(|b| {
                                b.ordinal() < due
                                    && local.ordinal() >= due
                                    && local.ordinal() - b.ordinal() <= 180
                            });
                            if utc > previous
                                && weekdays.contains(&local.weekday())
                                && crossed
                                && self
                                    .data
                                    .schedule_high_water
                                    .get(&key)
                                    .is_none_or(|h| *h < due)
                            {
                                self.data.schedule_high_water.insert(key.clone(), due);
                                let candidate = scheduled(s, &key, due, utc, revision);
                                if now - utc <= GRACE {
                                    self.queue(candidate);
                                } else {
                                    self.record_missed(candidate, now);
                                }
                            }
                            before = Some(local);
                        }
                    }
                    // Compact a long sleep's older occurrences into one missed record;
                    // a high-water mark suppresses all earlier intended local times.
                    if previous < now - GRACE {
                        if let Some(local) = local_at(now) {
                            for day in (local.day - 7..=local.day).rev() {
                                let intended = LocalMinute {
                                    day,
                                    minute: *minute,
                                };
                                let due = intended.ordinal();
                                if due <= local.ordinal() && weekdays.contains(&intended.weekday())
                                {
                                    if self
                                        .data
                                        .schedule_high_water
                                        .get(&key)
                                        .is_none_or(|h| *h < due)
                                    {
                                        self.data.schedule_high_water.insert(key.clone(), due);
                                        self.record_missed(
                                            scheduled(s, &key, due, now - GRACE - 1, revision),
                                            now,
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fn record_missed(&mut self, c: Candidate, now: i64) {
        self.data.sequence += 1;
        self.data.history.push(Delivery {
            id: self.data.sequence,
            events: vec![c],
            at_ms: now,
            status: Status::Missed,
            reason: Some("catch-up-expired".into()),
            attempts: 0,
            retry_at: None,
        });
    }
    pub fn snooze(&mut self, rule: Option<String>, until: Option<i64>) {
        if let Some(rule) = rule {
            if let Some(until) = until {
                self.data.rule_snoozes.insert(rule, until);
            } else {
                self.data.rule_snoozes.remove(&rule);
            }
        } else {
            self.data.snoozed_until = until;
        }
    }
    /// Caller must persist the mutated document before handing this delivery to Windows.
    /// A false transport gate never creates a claim or implies that a banner was seen.
    pub fn claim(
        &mut self,
        prefs: &Preferences,
        now: i64,
        local: Option<LocalMinute>,
        suppression: Suppression,
        transport_ready: bool,
    ) -> Option<Delivery> {
        self.data.prune(now);
        let expired = self
            .data
            .pending
            .values()
            .filter(|c| c.expires_at.is_some_and(|e| now >= e))
            .cloned()
            .collect::<Vec<_>>();
        for c in expired {
            self.data.pending.remove(&c.rule_key);
            if c.category == Category::Scheduled {
                self.record_missed(c, now);
            }
        }
        if !prefs.enabled || self.data.delivery_paused {
            return None;
        }
        let reason = if suppression.locked {
            Some("session-locked")
        } else if prefs.suppress_fullscreen && suppression.fullscreen {
            Some("fullscreen")
        } else if self.data.snoozed_until.is_some_and(|d| d > now) {
            Some("snoozed")
        } else if local.is_none() {
            Some("local-time-unavailable")
        } else if local.is_some_and(|t| quiet_at(&prefs.quiet_hours, t)) {
            Some("quiet-hours")
        } else {
            None
        };
        let mut eligible = Vec::new();
        for c in self.data.pending.values_mut() {
            let block = reason.or_else(|| {
                self.data
                    .rule_snoozes
                    .get(&c.rule_key)
                    .filter(|d| **d > now)
                    .map(|_| "rule-snoozed")
            });
            c.suppression = block.map(str::to_string);
            let quota = matches!(
                c.category,
                Category::Low | Category::Approaching | Category::Recovery | Category::Availability
            );
            let fresh = self.last_sample.as_ref().is_some_and(|(_, at)| {
                now >= *at
                    && now - at <= MINUTE
                    && c.binding.as_ref().is_none_or(|b| {
                        self.data
                            .episodes
                            .get(b)
                            .is_some_and(|e| e.observed_at == *at)
                    })
            });
            if block.is_none() && (!quota || fresh) {
                eligible.push(c.clone());
            }
        }
        if eligible.is_empty()
            || !transport_ready
            || self.data.last_claim_at.is_some_and(|t| now - t < MINUTE)
            || eligible
                .iter()
                .map(|c| c.eligible_at)
                .min()
                .is_none_or(|t| now - t < 5000)
        {
            return None;
        }
        self.data.sequence += 1;
        for c in &eligible {
            self.data.pending.remove(&c.rule_key);
            if let Some(e) = c
                .binding
                .as_ref()
                .and_then(|b| self.data.episodes.get_mut(b))
            {
                if e.number == c.episode {
                    e.handled.extend(&c.thresholds);
                    if c.category == Category::Approaching {
                        e.approaching_handled = true;
                    }
                }
            }
        }
        let delivery = Delivery {
            id: self.data.sequence,
            events: eligible,
            at_ms: now,
            status: Status::Claimed,
            reason: None,
            attempts: 1,
            retry_at: None,
        };
        self.data.last_claim_at = Some(now);
        self.data.history.push(delivery.clone());
        self.data.prune(now);
        Some(delivery)
    }
    /// Only definitive pre-submission failures can advance the same consumed attempt.
    /// Every candidate must still pass current policy, identity and freshness checks.
    pub fn retry(
        &mut self,
        prefs: &Preferences,
        now: i64,
        local: Option<LocalMinute>,
        suppression: Suppression,
        transport_ready: bool,
    ) -> Option<Delivery> {
        if !prefs.enabled
            || self.data.delivery_paused
            || !transport_ready
            || suppression.locked
            || prefs.suppress_fullscreen && suppression.fullscreen
            || self.data.snoozed_until.is_some_and(|t| t > now)
            || local.is_none_or(|l| quiet_at(&prefs.quiet_hours, l))
            || self.data.last_claim_at.is_some_and(|t| now - t < MINUTE)
        {
            return None;
        }
        let index = self.data.history.iter().position(|d| {
            d.status == Status::Failed && d.attempts < 3 && d.retry_at.is_some_and(|t| now >= t)
        })?;
        let valid = self.data.history[index].events.iter().all(|c| {
            if c.expires_at.is_some_and(|e| now >= e)
                || self
                    .data
                    .rule_snoozes
                    .get(&c.rule_key)
                    .is_some_and(|t| *t > now)
            {
                return false;
            }
            if c.category == Category::Scheduled {
                return prefs
                    .schedules
                    .iter()
                    .any(|s| s.enabled && schedule_key(s) == c.rule_key);
            }
            let Some((_, sample_at)) = &self.last_sample else {
                return false;
            };
            if now - sample_at > MINUTE {
                return false;
            }
            if c.category == Category::Availability {
                return self.last_allowed == Some(true);
            }
            let Some(b) = &c.binding else {
                return false;
            };
            if self
                .current_context
                .as_ref()
                .is_none_or(|ctx| !b.starts_with(&format!("{ctx}|")))
            {
                return false;
            }
            let Some(e) = self.data.episodes.get(b) else {
                return false;
            };
            if e.observed_at != *sample_at {
                return false;
            }
            let rule = prefs.rule(b);
            match c.category {
                Category::Low => {
                    rule.low
                        && e.number == c.episode
                        && e.remaining
                            .is_some_and(|r| c.thresholds.iter().any(|t| r <= f64::from(*t)))
                }
                Category::Approaching => {
                    rule.approaching
                        && e.number == c.episode
                        && e.deadline == c.expires_at
                        && e.deadline.is_some_and(|d| {
                            d > now && d - now <= i64::from(rule.lead_minutes) * MINUTE
                        })
                }
                Category::Recovery => rule.recovery && e.number == c.episode + 1,
                _ => false,
            }
        });
        if !valid {
            let d = &mut self.data.history[index];
            d.retry_at = None;
            d.status = Status::Cancelled;
            d.reason = Some("retry-condition-no-longer-valid".into());
            return None;
        }
        let d = &mut self.data.history[index];
        d.attempts += 1;
        d.status = Status::Claimed;
        d.at_ms = now;
        d.retry_at = None;
        self.data.last_claim_at = Some(now);
        Some(d.clone())
    }
    pub fn finish(&mut self, id: u64, outcome: Outcome, now: i64) {
        if let Some(d) = self.data.history.iter_mut().find(|d| d.id == id) {
            match outcome {
                Outcome::Submitted => {
                    d.status = Status::Submitted;
                    d.reason = None;
                    d.retry_at = None;
                }
                Outcome::Unknown => {
                    d.status = Status::OutcomeUnknown;
                    d.reason = Some("transport-outcome-unknown".into());
                    d.retry_at = None;
                }
                Outcome::DefinitelyNotSubmitted => {
                    d.status = Status::Failed;
                    d.reason = Some("definitely-not-submitted".into());
                    d.retry_at = match d.attempts {
                        _ if d.events.iter().any(|c| c.category == Category::Test) => None,
                        1 => Some(now + 30_000),
                        2 => Some(now + 120_000),
                        _ => None,
                    };
                }
            }
        }
    }

    /// Explicit user test, with no quota eligibility or automatic retries. The caller
    /// commits this claim before invoking native delivery, just like automatic claims.
    pub fn test_claim(&mut self, now: i64, revision: u64) -> Result<Delivery, &'static str> {
        if self.data.delivery_paused {
            return Err("reminder-delivery-paused");
        }
        if self
            .data
            .last_claim_at
            .is_some_and(|at| now.saturating_sub(at) < MINUTE)
        {
            return Err("notification-test-cooldown");
        }
        if self.data.sequence >= u64::MAX - 1024 {
            return Err("reminder-storage-limit");
        }
        self.data.sequence += 1;
        let key = format!("manual-test/{}", self.data.sequence);
        let delivery = Delivery {
            id: self.data.sequence,
            events: vec![Candidate {
                rule_key: key.clone(),
                event_keys: vec![key],
                category: Category::Test,
                binding: None,
                episode: 0,
                thresholds: vec![],
                label: "Notification test".into(),
                remaining: None,
                eligible_at: now,
                observed_at: now,
                expires_at: None,
                snapshot_revision: revision,
                suppression: None,
            }],
            at_ms: now,
            status: Status::Claimed,
            reason: None,
            attempts: 1,
            retry_at: None,
        };
        self.data.last_claim_at = Some(now);
        self.data.history.push(delivery.clone());
        self.data.prune(now);
        Ok(delivery)
    }
}
fn schedule_key(s: &Schedule) -> String {
    format!("schedule/{}/{}", s.id, s.revision)
}
fn scheduled(s: &Schedule, key: &str, occurrence: i64, at: i64, revision: u64) -> Candidate {
    Candidate {
        rule_key: key.into(),
        event_keys: vec![format!("{key}/{occurrence}")],
        category: Category::Scheduled,
        binding: None,
        episode: 0,
        thresholds: Vec::new(),
        label: s.label.clone(),
        remaining: None,
        eligible_at: at,
        observed_at: at,
        expires_at: Some(at + GRACE),
        snapshot_revision: revision,
        suppression: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::QuotaWindow;
    fn prefs() -> Preferences {
        Preferences {
            enabled: true,
            ..Default::default()
        }
    }
    fn sample(at: i64, remaining: f64, deadline: Option<i64>) -> Usage {
        Usage {
            account_context_key: Some("opaque-account".into()),
            last_success_at_ms: Some(at),
            freshness: "fresh".into(),
            windows: vec![QuotaWindow {
                key: "codex/primary/300".into(),
                limit_id: "codex".into(),
                label: "5-hour".into(),
                used_percent: Some(100.0 - remaining),
                remaining_percent: Some(remaining),
                duration_minutes: Some(300),
                resets_at_ms: deadline,
                validated_at_ms: Some(at),
                quality: "valid".into(),
                last_known: false,
            }],
            ..Default::default()
        }
    }
    fn local(_: i64) -> Option<LocalMinute> {
        Some(LocalMinute {
            day: 0,
            minute: 600,
        })
    }
    fn claim(e: &mut Engine, p: &Preferences, at: i64) -> Delivery {
        e.claim(p, at, local(at), Suppression::default(), true)
            .unwrap()
    }
    #[test]
    fn thresholds_use_raw_values_and_coalesce_all_crossings() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 0.4, None), true, 1, 1000);
        assert!(e
            .claim(&p, 5999, local(0), Suppression::default(), true)
            .is_none());
        let d = claim(&mut e, &p, 6000);
        assert_eq!(d.events.len(), 1);
        assert_eq!(d.events[0].thresholds, vec![20, 10]);
        e.finish(d.id, Outcome::Submitted, 6000);
        e.observe(&p, &sample(70_000, 0.0, None), true, 2, 70_000);
        let d = claim(&mut e, &p, 75_000);
        assert_eq!(d.events[0].thresholds, vec![0]);
        e.observe(&p, &sample(140_000, 0.0, None), true, 3, 140_000);
        assert!(e.data.pending.is_empty());
    }
    #[test]
    fn manual_tests_have_durable_claims_without_quota_episodes_or_automatic_retries() {
        let mut engine = Engine::new(Durable::default(), false);
        let claim = engine.test_claim(1000, 1).unwrap();
        assert_eq!(claim.events[0].category, Category::Test);
        assert!(claim.events[0].binding.is_none());
        assert!(engine.data.episodes.is_empty());
        assert!(engine.data.schedule_high_water.is_empty());
        assert!(engine.data.validate().is_ok());
        assert_eq!(engine.snapshot(None).history.len(), 1);
        let restarted = Engine::new(engine.data.clone(), false);
        assert_eq!(
            restarted.snapshot(None).history[0].status,
            Status::OutcomeUnknown
        );
        assert_eq!(
            engine.test_claim(60_999, 2).unwrap_err(),
            "notification-test-cooldown"
        );
        engine.finish(claim.id, Outcome::DefinitelyNotSubmitted, 2000);
        assert!(engine.data.history[0].retry_at.is_none());
        assert_eq!(engine.data.history[0].status, Status::Failed);
        let next = engine.test_claim(61_000, 2).unwrap();
        assert_ne!(next.id, claim.id);
        engine.finish(next.id, Outcome::Unknown, 61_001);
        assert_eq!(engine.data.history[1].status, Status::OutcomeUnknown);
        engine.data.delivery_paused = true;
        assert_eq!(
            engine.test_claim(200_000, 3).unwrap_err(),
            "reminder-delivery-paused"
        );
    }
    #[test]
    fn correction_does_not_rearm_but_sustained_recovery_does() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 8.0, Some(90_000)), true, 1, 1000);
        claim(&mut e, &p, 6000);
        e.observe(&p, &sample(70_000, 24.0, Some(180_000)), true, 2, 70_000);
        e.observe(&p, &sample(80_000, 8.0, Some(240_000)), true, 3, 80_000);
        assert!(e.data.pending.is_empty());
        e.observe(&p, &sample(100_000, 40.0, None), true, 4, 100_000);
        e.observe(&p, &sample(129_999, 40.0, None), true, 5, 129_999);
        assert!(e.data.pending.is_empty());
        e.observe(&p, &sample(130_000, 40.0, None), true, 6, 130_000);
        assert_eq!(
            e.data.pending.values().next().unwrap().category,
            Category::Recovery
        );
        assert_eq!(e.data.episodes.values().next().unwrap().number, 1);
    }
    #[test]
    fn reset_requires_a_post_deadline_sample_later_deadline_and_increase() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 0.0, Some(90_000)), true, 1, 1000);
        claim(&mut e, &p, 6000);
        e.observe(&p, &sample(80_000, 1.0, Some(180_000)), true, 2, 80_000);
        assert_eq!(e.data.episodes.values().next().unwrap().number, 0);
        e.observe(&p, &sample(180_000, 1.0, Some(260_000)), true, 3, 180_000);
        assert_eq!(e.data.episodes.values().next().unwrap().number, 0);
        e.observe(&p, &sample(270_000, 80.0, Some(360_000)), true, 4, 270_000);
        assert_eq!(e.data.episodes.values().next().unwrap().number, 1);
    }
    #[test]
    fn restart_claim_is_unknown_and_baseline_and_edits_are_silent() {
        let mut p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 8.0, None), true, 1, 1000);
        claim(&mut e, &p, 6000);
        let bytes = serde_json::to_vec(&e.data).unwrap();
        let mut restarted = Engine::new(serde_json::from_slice(&bytes).unwrap(), true);
        assert_eq!(restarted.data.history[0].status, Status::OutcomeUnknown);
        restarted.observe(&p, &sample(70_000, 0.0, None), true, 2, 70_000);
        assert!(restarted.data.pending.is_empty());
        let old = p.clone();
        p.defaults.thresholds = vec![30, 25, 0];
        restarted.configure(&old, &p, 80_000, local(80_000));
        restarted.observe(&p, &sample(90_000, 20.0, None), true, 3, 90_000);
        assert!(restarted.data.pending.is_empty());
        restarted.data.clear_history();
        assert!(!restarted
            .data
            .episodes
            .values()
            .next()
            .unwrap()
            .handled
            .is_empty());
    }
    #[test]
    fn suppression_is_bounded_and_resume_needs_fresh_quota() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        for i in 1..10 {
            e.observe(&p, &sample(i * 1000, 15.0, None), true, i as u64, i * 1000);
        }
        assert_eq!(e.data.pending.len(), 1);
        e.snooze(None, Some(100_000));
        assert!(e
            .claim(&p, 10_000, local(0), Suppression::default(), true)
            .is_none());
        assert!(e
            .claim(&p, 100_001, local(0), Suppression::default(), true)
            .is_none());
        e.observe(&p, &sample(101_000, 5.0, None), true, 10, 101_000);
        let d = claim(&mut e, &p, 101_000);
        assert_eq!(d.events[0].thresholds, vec![20, 10]);
    }
    #[test]
    fn identity_loss_and_missing_windows_suspend_candidates() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 1.0, None), true, 1, 1000);
        e.observe(&p, &sample(2000, 1.0, None), false, 2, 2000);
        assert!(e.data.pending.is_empty());
        e.observe(&p, &sample(3000, 1.0, None), true, 3, 3000);
        assert!(e.data.pending.is_empty());
    }
    #[test]
    fn quiet_hours_belong_to_start_day_and_validate() {
        let q = QuietHours {
            enabled: true,
            start_minute: 22 * 60,
            end_minute: 7 * 60,
            weekdays: vec![0],
        };
        assert!(quiet_at(
            &q,
            LocalMinute {
                day: 4,
                minute: 23 * 60
            }
        )); // Monday
        assert!(quiet_at(
            &q,
            LocalMinute {
                day: 5,
                minute: 6 * 60
            }
        ));
        assert!(!quiet_at(
            &q,
            LocalMinute {
                day: 5,
                minute: 23 * 60
            }
        ));
        let mut p = prefs();
        p.quiet_hours = q;
        p.quiet_hours.end_minute = p.quiet_hours.start_minute;
        assert!(p.validate().is_err());
    }
    #[test]
    fn schedule_dst_gap_fold_and_clock_rollback_deduplicate_intended_time() {
        let mut p = prefs();
        p.schedules = vec![Schedule {
            id: "test".into(),
            revision: 1,
            enabled: true,
            label: "Check in".into(),
            timing: Timing::Weekly {
                minute: 150,
                weekdays: vec![3],
            },
        }];
        let mut e = Engine::new(
            Durable {
                schedule_scanned_at: Some(0),
                ..Default::default()
            },
            true,
        );
        // 01:59 -> 03:00: nonexistent 02:30 runs at 03:00.
        let gap = |t: i64| {
            Some(LocalMinute {
                day: 0,
                minute: if t < MINUTE { 119 } else { 180 },
            })
        };
        e.schedules(&p, MINUTE, gap, 1);
        assert_eq!(e.data.pending.len(), 1);
        let d = claim(&mut e, &p, MINUTE + 5000);
        assert_eq!(d.events[0].event_keys, vec!["schedule/test/1/150"]);
        e.schedules(
            &p,
            2 * MINUTE,
            |_| {
                Some(LocalMinute {
                    day: 0,
                    minute: 120,
                })
            },
            2,
        );
        e.schedules(
            &p,
            3 * MINUTE,
            |t| {
                Some(LocalMinute {
                    day: 0,
                    minute: if t < 3 * MINUTE { 149 } else { 150 },
                })
            },
            3,
        );
        e.schedules(&p, 0, gap, 4);
        assert!(e.data.pending.is_empty());
    }
    #[test]
    fn one_time_catchup_expiry_recovery_and_history_tombstones() {
        let mut p = prefs();
        p.schedules = vec![Schedule {
            id: "test".into(),
            revision: 1,
            enabled: true,
            label: "Check in".into(),
            timing: Timing::Once { at_ms: MINUTE },
        }];
        let mut e = Engine::new(Durable::default(), true);
        e.schedules(&p, 20 * MINUTE, local, 1);
        assert_eq!(e.data.history[0].status, Status::Missed);
        e.data.clear_history();
        e.schedules(&p, 21 * MINUTE, local, 2);
        assert!(e.data.history.is_empty());
        e.data.recover(22 * MINUTE);
        assert!(e.data.delivery_paused);
        let old = p.clone();
        p.enabled = false;
        e.configure(&old, &p, 22 * MINUTE, local(22 * MINUTE));
        let old = p.clone();
        p.enabled = true;
        e.configure(&old, &p, 23 * MINUTE, local(23 * MINUTE));
        assert!(!e.data.delivery_paused);
    }
    #[test]
    fn multiple_windows_coalesce_and_definitive_retries_keep_consumed_keys() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        let mut u = sample(1000, 8.0, None);
        let mut weekly = u.windows[0].clone();
        weekly.key = "codex/secondary/10080".into();
        weekly.label = "Weekly".into();
        u.windows.push(weekly);
        e.observe(&p, &u, true, 1, 1000);
        let d = claim(&mut e, &p, 6000);
        assert_eq!(d.events.len(), 2);
        let keys = d
            .events
            .iter()
            .flat_map(|c| c.event_keys.clone())
            .collect::<Vec<_>>();
        e.finish(d.id, Outcome::DefinitelyNotSubmitted, 6000);
        assert_eq!(e.data.history[0].retry_at, Some(36_000));
        assert!(e
            .retry(&p, 36_000, local(0), Suppression::default(), true)
            .is_none()); // global cooldown
        u.last_success_at_ms = Some(66_000);
        for w in &mut u.windows {
            w.validated_at_ms = Some(66_000);
        }
        e.observe(&p, &u, true, 2, 66_000);
        assert!(e.data.pending.is_empty());
        let retry = e
            .retry(&p, 66_000, local(0), Suppression::default(), true)
            .unwrap();
        assert_eq!(retry.id, d.id);
        assert_eq!(retry.attempts, 2);
        assert_eq!(
            keys,
            retry
                .events
                .iter()
                .flat_map(|c| c.event_keys.clone())
                .collect::<Vec<_>>()
        );
        e.finish(d.id, Outcome::DefinitelyNotSubmitted, 66_000);
        assert_eq!(e.data.history[0].retry_at, Some(186_000));
        e.observe(&p, &sample(186_000, 8.0, None), true, 3, 186_000); // missing weekly cancels retry
        assert!(e
            .retry(&p, 186_000, local(0), Suppression::default(), true)
            .is_none());
        assert_eq!(e.data.history[0].status, Status::Cancelled);
    }
    #[test]
    fn ambiguous_submission_never_retries_and_stale_data_never_claims() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 8.0, None), true, 1, 1000);
        e.mark_stale();
        assert!(e
            .claim(&p, 6000, local(0), Suppression::default(), true)
            .is_none());
        e.observe(&p, &sample(70_000, 8.0, None), true, 2, 70_000);
        let d = claim(&mut e, &p, 70_000);
        e.finish(d.id, Outcome::Unknown, 70_000);
        assert_eq!(e.data.history[0].status, Status::OutcomeUnknown);
        assert!(e.data.history[0].retry_at.is_none());
    }
    #[test]
    fn retries_stop_after_two_and_other_account_history_is_not_published() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 8.0, None), true, 1, 1000);
        let d = claim(&mut e, &p, 6000);
        e.finish(d.id, Outcome::DefinitelyNotSubmitted, 6000);
        for at in [66_000, 186_000] {
            e.observe(&p, &sample(at, 8.0, None), true, 2, at);
            let d = e
                .retry(&p, at, local(0), Suppression::default(), true)
                .unwrap();
            e.finish(d.id, Outcome::DefinitelyNotSubmitted, at);
        }
        assert_eq!(e.data.history[0].attempts, 3);
        assert!(e.data.history[0].retry_at.is_none());
        assert_eq!(e.snapshot(None).history.len(), 1);
        let mut other = sample(250_000, 3.0, None);
        other.account_context_key = Some("another-opaque-context".into());
        e.observe(&p, &other, true, 3, 250_000);
        assert!(e.snapshot(None).history.is_empty());
        assert!(e.data.pending.is_empty());
    }
    #[test]
    fn preferences_reject_ambiguous_rules_past_dates_and_unversioned_edits() {
        let mut p = prefs();
        p.defaults.thresholds = vec![10, 20];
        assert!(p.validate().is_err());
        p.defaults.thresholds = vec![20, 20];
        assert!(p.validate().is_err());
        p.defaults.thresholds = vec![99, 50, 0];
        assert!(p.validate().is_ok());
        let old = p.clone();
        p.schedules = vec![Schedule {
            id: "one".into(),
            revision: 1,
            enabled: true,
            label: "Check".into(),
            timing: Timing::Once { at_ms: 50 },
        }];
        assert_eq!(p.validate_edit(&old, 100), Err("schedule-must-be-future"));
        assert!(p.validate_edit(&old, 0).is_ok());
        let old = p.clone();
        p.schedules[0].label = "Edited".into();
        assert_eq!(p.validate_edit(&old, 0), Err("schedule-revision-conflict"));
    }
    #[test]
    fn mapping_changes_silently_rebase_and_recovery_has_a_fallback_threshold() {
        let p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 8.0, None), true, 1, 1000);
        claim(&mut e, &p, 6000);
        let mut changed = sample(70_000, 0.0, None);
        changed.windows[0].key = "codex/primary/600".into();
        e.observe(&p, &changed, true, 2, 70_000);
        assert!(e.data.pending.is_empty());
        let mut p = prefs();
        p.defaults.thresholds.clear();
        let mut e = Engine::new(Durable::default(), false);
        for (at, r) in [(1000, 5.0), (40_000, 25.0), (70_000, 25.0)] {
            e.observe(&p, &sample(at, r, None), true, 1, at);
        }
        assert_eq!(
            e.data.pending.values().next().unwrap().category,
            Category::Recovery
        );
    }
    #[test]
    fn one_window_recovery_does_not_infer_account_availability() {
        let mut p = prefs();
        p.detailed = true;
        let mut e = Engine::new(Durable::default(), false);
        let mut u = sample(1000, 0.0, Some(50_000));
        u.ordinary_usage_allowed = Some(false);
        let mut weekly = u.windows[0].clone();
        weekly.key = "codex/secondary/10080".into();
        weekly.label = "Weekly".into();
        weekly.resets_at_ms = Some(500_000);
        u.windows.push(weekly);
        e.observe(&p, &u, true, 1, 1000);
        claim(&mut e, &p, 6000);
        u.last_success_at_ms = Some(70_000);
        for w in &mut u.windows {
            w.validated_at_ms = Some(70_000);
        }
        u.windows[0].remaining_percent = Some(80.0);
        u.windows[0].resets_at_ms = Some(180_000);
        e.observe(&p, &u, true, 2, 70_000);
        let d = claim(&mut e, &p, 75_000);
        assert_eq!(d.events.len(), 1);
        assert_eq!(d.events[0].category, Category::Recovery);
        let body = crate::notification::payload(&d, &p, &u, 75_000).body;
        assert!(body.contains("Still exhausted: Weekly"));
        assert!(body.contains("Ordinary usage remains blocked"));
        assert!(!body.contains("Usage available again"));
        u.last_success_at_ms = Some(140_000);
        for w in &mut u.windows {
            w.validated_at_ms = Some(140_000);
        }
        u.ordinary_usage_allowed = Some(true);
        e.observe(&p, &u, true, 3, 140_000);
        let d = claim(&mut e, &p, 145_000);
        assert_eq!(d.events[0].category, Category::Availability);
    }
    #[test]
    fn an_expired_candidate_cannot_skip_a_later_coalescing_window() {
        let mut p = prefs();
        p.defaults.approaching = true;
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 8.0, Some(4000)), true, 1, 1000);
        e.data
            .pending
            .retain(|_, c| c.category == Category::Approaching);
        assert!(e
            .claim(&p, 6000, local(0), Suppression::default(), true)
            .is_none());
        e.observe(&p, &sample(70_000, 0.0, None), true, 2, 70_000);
        assert!(e
            .claim(&p, 70_000, local(0), Suppression::default(), true)
            .is_none());
        assert!(e
            .claim(&p, 75_000, local(0), Suppression::default(), true)
            .is_some());
    }
    #[test]
    fn fullscreen_and_lock_suppression_do_not_depend_on_widget_visibility_settings() {
        let mut p = prefs();
        let mut e = Engine::new(Durable::default(), false);
        e.observe(&p, &sample(1000, 0.0, None), true, 1, 1000);
        assert!(e
            .claim(
                &p,
                6000,
                local(0),
                Suppression {
                    locked: false,
                    fullscreen: true
                },
                true
            )
            .is_none());
        p.suppress_fullscreen = false;
        assert!(e
            .claim(
                &p,
                6000,
                local(0),
                Suppression {
                    locked: true,
                    fullscreen: true
                },
                true
            )
            .is_none());
        assert!(e
            .claim(
                &p,
                6000,
                local(0),
                Suppression {
                    locked: false,
                    fullscreen: true
                },
                true
            )
            .is_some());
    }
}

//! Opt-in, local-only evidence. Never pass protocol payloads or arbitrary text here.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 1_048_576;
const MAX_RECORD: usize = 4096;
const BACKUPS: usize = 4;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub segment: u64,
    pub recording: bool,
    pub directory: String,
    pub error: Option<&'static str>,
    pub dropped: u64,
}

#[derive(Clone)]
struct Configuration {
    segment: u64,
    enabled: bool,
    directory: PathBuf,
}
struct Record {
    config: Arc<Configuration>,
    at: i64,
    elapsed: u64,
    sequence: u64,
    event: &'static str,
    data: Value,
}
enum Message {
    Record(Record),
    Wake,
    Flush(mpsc::Sender<()>),
}

pub struct Recorder {
    enabled: AtomicBool,
    ever_usable: AtomicBool,
    config: Mutex<Arc<Configuration>>,
    sender: OnceLock<mpsc::SyncSender<Message>>,
    status: Mutex<Status>,
    sequence: AtomicU64,
    dropped: AtomicU64,
    started: Instant,
    session: String,
    observer: Mutex<Observer>,
    rates: Mutex<HashMap<String, (Instant, u32)>>,
}

static RECORDER: OnceLock<Arc<Recorder>> = OnceLock::new();
pub fn global() -> &'static Arc<Recorder> {
    RECORDER.get_or_init(Recorder::new)
}
pub fn enabled() -> bool {
    global().enabled.load(Ordering::Relaxed)
}
pub fn event(name: &'static str, data: impl FnOnce() -> Value) {
    if enabled() {
        global().record(name, data());
    }
}
pub fn status() -> Status {
    global().status()
}
static READ_ID: AtomicU64 = AtomicU64::new(0);
pub fn next_read_id() -> u64 {
    READ_ID.fetch_add(1, Ordering::Relaxed) + 1
}

impl Recorder {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(false),
            ever_usable: AtomicBool::new(false),
            config: Mutex::new(Arc::new(Configuration {
                segment: 0,
                enabled: false,
                directory: PathBuf::new(),
            })),
            sender: OnceLock::new(),
            status: Mutex::new(Status::default()),
            sequence: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            started: Instant::now(),
            session: format!("{}-{}", crate::model::now_ms(), std::process::id()),
            observer: Mutex::new(Observer::default()),
            rates: Mutex::new(HashMap::new()),
        })
    }
    pub fn configure(self: &Arc<Self>, enabled: bool, directory: PathBuf) {
        self.enabled.store(false, Ordering::Relaxed);
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        *config = Arc::new(Configuration {
            segment: config.segment + 1,
            enabled,
            directory: directory.join("diagnostics"),
        });
        *self.observer.lock().unwrap_or_else(|e| e.into_inner()) = Observer::default();
        self.rates.lock().unwrap_or_else(|e| e.into_inner()).clear();
        {
            let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            status.enabled = enabled;
            status.segment = config.segment;
            status.recording = false;
            status.error = None;
            status.directory = config.directory.to_string_lossy().into_owned();
        }
        if enabled && self.sender.get().is_none() {
            let (tx, rx) = mpsc::sync_channel(256);
            let recorder = self.clone();
            if std::thread::Builder::new()
                .name("widget-diagnostics".into())
                .spawn(move || recorder.writer(rx))
                .is_ok()
            {
                let _ = self.sender.set(tx);
            } else {
                self.status.lock().unwrap_or_else(|e| e.into_inner()).error =
                    Some("diagnostics-worker");
            }
        }
        self.enabled
            .store(enabled && self.sender.get().is_some(), Ordering::Relaxed);
        drop(config);
        if let Some(sender) = self.sender.get() {
            let _ = sender.try_send(Message::Wake);
        }
        if enabled {
            self.record(
                "recording_baseline",
                json!({"appVersion":env!("CARGO_PKG_VERSION")}),
            );
        }
    }
    pub fn follow_storage(self: &Arc<Self>, enabled: bool, directory: PathBuf) {
        let changed = {
            let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
            config.enabled != enabled || config.directory != directory.join("diagnostics")
        };
        if changed {
            self.configure(enabled, directory);
        }
    }
    pub fn status(&self) -> Status {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        status.recording &= status.enabled;
        status.dropped = self.dropped.load(Ordering::Relaxed);
        status
    }
    fn record(&self, event: &'static str, data: Value) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let config = self
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !config.enabled {
            return;
        }
        if let Some(sender) = self.sender.get() {
            let record = Record {
                config,
                at: crate::model::now_ms(),
                elapsed: self.started.elapsed().as_millis() as u64,
                sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
                event,
                data,
            };
            if sender.try_send(Message::Record(record)).is_err() {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    pub fn flush(&self, timeout: Duration) {
        if let Some(sender) = self.sender.get() {
            let (tx, rx) = mpsc::channel();
            if sender.try_send(Message::Flush(tx)).is_ok() {
                let _ = rx.recv_timeout(timeout);
            }
        }
    }
    fn writer(&self, rx: mpsc::Receiver<Message>) {
        let mut sink: Option<Sink> = None;
        let mut segment = 0;
        let mut failed_segment = None;
        let mut last_flush = Instant::now();
        loop {
            let message = if sink.is_some() {
                rx.recv_timeout(Duration::from_secs(1))
            } else {
                rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            };
            if matches!(message, Err(mpsc::RecvTimeoutError::Disconnected)) {
                break;
            }
            let idle_message = !matches!(&message, Ok(Message::Record(_)));
            let mut failure = None;
            let mut flush_reply = None;
            match message {
                Ok(Message::Record(record)) => {
                    if record.config.segment < segment {
                        self.dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    if segment != record.config.segment {
                        if let Some(mut old) = sink.take() {
                            old.dropped = self.dropped.load(Ordering::Relaxed);
                            if old.finish().is_err() {
                                failure = Some("diagnostics-flush");
                            }
                        }
                        segment = record.config.segment;
                        if failed_segment != Some(segment) {
                            match Sink::open(
                                &record.config,
                                &self.session,
                                self.started.elapsed().as_millis() as u64,
                            ) {
                                Ok(next) => {
                                    sink = Some(next);
                                    self.set_recording(true, None);
                                }
                                Err(_) => failure = Some("diagnostics-open"),
                            }
                        }
                    }
                    if let Some(sink) = sink.as_mut() {
                        let value = json!({"format":1,"ts_unix_ms":record.at,"elapsed_ms":record.elapsed,"session":self.session,
                            "segment":segment,"seq":record.sequence,"event":record.event,"data":record.data});
                        if sink.write(&value).is_err() {
                            failure = Some("diagnostics-write");
                        }
                    } else {
                        self.dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Ok(Message::Flush(reply)) => {
                    if let Some(sink) = sink.as_mut() {
                        if sink.flush().is_err() {
                            failure = Some("diagnostics-flush");
                        }
                    }
                    flush_reply = Some(reply);
                }
                _ => {}
            }
            let desired = self
                .config
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if !desired.enabled && idle_message {
                // Queued events retain their original directory. A later recording
                // baseline opens the new segment; disable closes after queued writes.
                if let Some(mut old) = sink.take() {
                    old.dropped = self.dropped.load(Ordering::Relaxed);
                    if old.finish().is_err() {
                        failure = Some("diagnostics-flush");
                    }
                }
                self.set_recording(false, None);
            }
            if last_flush.elapsed() >= Duration::from_secs(1) {
                if let Some(sink) = sink.as_mut() {
                    let dropped = self.dropped.load(Ordering::Relaxed);
                    if sink.dropped != dropped {
                        if sink
                            .control("events_dropped", json!({"total":dropped}))
                            .is_err()
                        {
                            failure = Some("diagnostics-write");
                        }
                        sink.dropped = dropped;
                    }
                    if sink.flush().is_err() {
                        failure = Some("diagnostics-flush");
                    }
                }
                last_flush = Instant::now();
            }
            if let Some(error) = failure {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                sink = None;
                failed_segment = Some(segment);
                let current = self.config.lock().unwrap_or_else(|e| e.into_inner());
                if current.segment == segment {
                    self.enabled.store(false, Ordering::Relaxed);
                    self.set_recording(false, Some(error));
                }
            }
            if let Some(reply) = flush_reply {
                let _ = reply.send(());
            }
        }
    }
    fn set_recording(&self, recording: bool, error: Option<&'static str>) {
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.recording = recording;
        if error.is_some() {
            status.error = error;
        }
    }
}

struct Sink {
    directory: PathBuf,
    file: Option<BufWriter<File>>,
    bytes: u64,
    session: String,
    segment: u64,
    dropped: u64,
    opened: Instant,
    origin_elapsed: u64,
}
impl Sink {
    fn open(config: &Configuration, session: &str, elapsed: u64) -> std::io::Result<Self> {
        fs::create_dir_all(&config.directory)?;
        let path = config.directory.join("widget.jsonl");
        let bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let file = BufWriter::new(OpenOptions::new().create(true).append(true).open(path)?);
        let mut sink = Self {
            directory: config.directory.clone(),
            file: Some(file),
            bytes,
            session: session.into(),
            segment: config.segment,
            dropped: 0,
            opened: Instant::now(),
            origin_elapsed: elapsed,
        };
        sink.control("segment_start", json!({"elapsed_ms":elapsed}))?;
        Ok(sink)
    }
    fn write(&mut self, value: &Value) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec(value)?;
        if bytes.len() + 1 > MAX_RECORD {
            return Err(std::io::Error::other("record limit"));
        }
        bytes.push(b'\n');
        if self.bytes + bytes.len() as u64 > MAX_BYTES {
            self.rotate()?;
        }
        self.file
            .as_mut()
            .ok_or_else(|| std::io::Error::other("log closed"))?
            .write_all(&bytes)?;
        self.bytes += bytes.len() as u64;
        Ok(())
    }
    fn control(&mut self, event: &str, data: Value) -> std::io::Result<()> {
        self.write(&json!({"format":1,"ts_unix_ms":crate::model::now_ms(),"elapsed_ms":self.origin_elapsed+self.opened.elapsed().as_millis() as u64,"session":self.session,"segment":self.segment,"event":event,"data":data}))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| std::io::Error::other("log closed"))?
            .flush()
    }
    fn finish(&mut self) -> std::io::Result<()> {
        self.control("segment_end", json!({"dropped_total":self.dropped}))?;
        self.flush()
    }
    fn rotate(&mut self) -> std::io::Result<()> {
        self.flush()?;
        drop(self.file.take());
        // Close before rotating so Windows and restrictive filesystems permit rename.
        let oldest = self.directory.join(format!("widget.{BACKUPS}.jsonl"));
        if oldest.exists() {
            fs::remove_file(oldest)?;
        }
        for index in (1..BACKUPS).rev() {
            let path = self.directory.join(format!("widget.{index}.jsonl"));
            if path.exists() {
                fs::rename(
                    path,
                    self.directory.join(format!("widget.{}.jsonl", index + 1)),
                )?;
            }
        }
        fs::rename(
            self.directory.join("widget.jsonl"),
            self.directory.join("widget.1.jsonl"),
        )?;
        self.file = Some(BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.directory.join("widget.jsonl"))?,
        ));
        self.bytes = 0;
        Ok(())
    }
}

#[derive(Default)]
struct Observer {
    signature: Value,
    flags: HashMap<&'static str, u64>,
    ever_usable: bool,
}
impl Observer {
    fn observe(&mut self, u: &crate::model::Usage, now: u64) -> Vec<(&'static str, Value)> {
        let usable = u.windows.iter().any(|w| w.remaining_percent.is_some());
        let pending = u.consecutive_unavailable_reads < 3
            && !["auth-required", "unsupported", "paused"].contains(&u.connection.as_str());
        let flags = [
            ("quota_loading", !usable && pending),
            ("refreshing", u.refresh_in_flight),
            (
                "stale",
                u.freshness == "stale" || u.windows.iter().any(|w| w.last_known),
            ),
            (
                "partial",
                usable
                    && u.windows
                        .iter()
                        .any(|w| w.remaining_percent.is_none() || w.quality != "valid"),
            ),
            ("unavailable", !usable && !pending),
        ];
        let signature = json!({"connection":u.connection,"freshness":u.freshness,"refreshing":u.refresh_in_flight,
            "generation":u.connection_generation,"unavailableReads":u.consecutive_unavailable_reads,"error":u.error_code,
            "windows":u.windows.iter().map(|w|json!({"period":match w.duration_minutes {Some(300)=>"five-hour",Some(10080)=>"weekly",_=>"other"},"present":w.remaining_percent.is_some(),"quality":w.quality,"retained":w.last_known,"missing_reads":w.key.rsplit_once('/').and_then(|(slot,_)|u.missing_window_reads.get(slot)).copied().unwrap_or(0)})).collect::<Vec<_>>()});
        let baseline = self.signature.is_null();
        let mut events = Vec::new();
        if self.signature != signature {
            events.push(("usage_state",json!({"state":signature,"age_ms":u.last_success_at_ms.map(|at|crate::model::now_ms().saturating_sub(at)),"runtime":self.ever_usable})));
            self.signature = signature;
        }
        for (name, active) in flags {
            if active && !self.flags.contains_key(name) {
                self.flags.insert(name, now);
                events.push(("state_enter",json!({"condition":name,"origin":if self.ever_usable{"runtime"}else{"startup"},"left_censored":baseline})));
            }
            if !active {
                if let Some(at) = self.flags.remove(name) {
                    events.push((
                        "state_exit",
                        json!({"condition":name,"duration_ms":now.saturating_sub(at)}),
                    ));
                    if matches!(name, "quota_loading" | "stale")
                        && usable
                        && u.freshness == "fresh"
                        && !u.windows.iter().any(|w| w.last_known)
                    {
                        events.push((
                            "recovery",
                            json!({"condition":name,"duration_ms":now.saturating_sub(at)}),
                        ));
                    }
                }
            }
        }
        self.ever_usable |= usable;
        events
    }
}
pub fn observe(state: &crate::model::AppState) {
    let recorder = global();
    let previously_usable = recorder.ever_usable.load(Ordering::Relaxed);
    if state
        .usage
        .windows
        .iter()
        .any(|w| w.remaining_percent.is_some())
    {
        recorder.ever_usable.store(true, Ordering::Relaxed);
    }
    if !enabled() {
        return;
    }
    let mut observer = recorder.observer.lock().unwrap_or_else(|e| e.into_inner());
    observer.ever_usable |= previously_usable;
    let events = observer.observe(&state.usage, recorder.started.elapsed().as_millis() as u64);
    drop(observer);
    for (name, data) in events {
        recorder.record(name, data);
    }
}

// No untrusted strings, arbitrary objects, paths, errors or account keys in IPC.
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontendKind {
    Mount,
    Unmount,
    SnapshotStart,
    SnapshotEnd,
    SnapshotFailed,
    ListenReady,
    ListenFailed,
    Gap,
    Rejected,
    EpochChanged,
    Display,
    RenderSummary,
    Visibility,
    FrontendError,
    FrontendRejection,
    ClientDropped,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendEvent {
    kind: FrontendKind,
    pub view: u32,
    pub segment: Option<u64>,
    pub elapsed_ms: u64,
    pub revision: Option<u64>,
    pub duration_ms: Option<u64>,
    pub flags: Option<u16>,
    pub count: Option<u32>,
    pub clock_count: Option<u32>,
    pub state_count: Option<u32>,
    pub reason: Option<FrontendReason>,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontendReason {
    Initial,
    Gap,
    Focus,
    Visible,
    Changed,
    Unmount,
    Interval,
}
pub fn frontend(source: &str, events: Vec<FrontendEvent>) -> Result<(), &'static str> {
    if !matches!(source, "main" | "shell" | "preview") {
        return Err("diagnostics-source");
    }
    if events.len() > 32 {
        return Err("diagnostics-batch-limit");
    }
    if !enabled() {
        return Ok(());
    }
    let recorder = global();
    let mut rates = recorder.rates.lock().unwrap_or_else(|e| e.into_inner());
    let rate = rates.entry(source.into()).or_insert((Instant::now(), 0));
    if rate.0.elapsed() >= Duration::from_secs(60) {
        *rate = (Instant::now(), 0);
    }
    if rate.1 + events.len() as u32 > 240 {
        recorder
            .dropped
            .fetch_add(events.len() as u64, Ordering::Relaxed);
        return Ok(());
    }
    rate.1 += events.len() as u32;
    drop(rates);
    for event in &events {
        if event.view > 4
            || event.count.is_some_and(|v| v > 1_000_000)
            || event.elapsed_ms > 31_536_000_000
            || event.duration_ms.is_some_and(|v| v > 31_536_000_000)
            || event.flags.is_some_and(|v| v > 255)
        {
            return Err("diagnostics-event-bounds");
        }
    }
    let segment = recorder.status().segment;
    for event in events {
        if event.segment.is_some_and(|value| value != segment) {
            recorder.dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        recorder.record("frontend", json!({"source":source,"detail":event}));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "widget-diagnostics-{name}-{}-{}",
            std::process::id(),
            crate::storage::new_profile_id()
        ))
    }
    #[test]
    fn disabled_creates_nothing_and_enabled_rotates_bounded_files() {
        let dir = directory("rotation");
        let recorder = Recorder::new();
        recorder.configure(false, dir.clone());
        recorder.record("ignored", json!({}));
        assert!(!dir.exists());
        let config = Configuration {
            segment: 1,
            enabled: true,
            directory: dir.join("diagnostics"),
        };
        let mut sink = Sink::open(&config, "synthetic", 0).unwrap();
        for _ in 0..2000 {
            sink.write(&json!({"padding":"x".repeat(3500)})).unwrap();
        }
        sink.finish().unwrap();
        let entries = fs::read_dir(&config.directory)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), 5);
        for entry in entries {
            assert!(entry.metadata().unwrap().len() <= MAX_BYTES);
        }
        drop(sink);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn state_events_deduplicate_and_measure_overlapping_conditions() {
        let mut observer = Observer::default();
        let mut u = crate::model::Usage {
            connection: "starting".into(),
            freshness: "empty".into(),
            refresh_in_flight: true,
            ..Default::default()
        };
        let events = observer.observe(&u, 100);
        assert_eq!(
            events.iter().filter(|(n, _)| *n == "state_enter").count(),
            2
        );
        assert!(observer.observe(&u, 200).is_empty());
        u.refresh_in_flight = false;
        let events = observer.observe(&u, 700);
        assert!(events
            .iter()
            .any(|(n, d)| *n == "state_exit" && d["duration_ms"] == 600));
        u.windows = crate::model::normalize(
            &json!({"rateLimits":{"primary":{"usedPercent":30,"windowDurationMins":300}}}),
            1,
            &u,
        )
        .windows;
        observer.observe(&u, 800);
        u.windows.clear();
        assert!(observer
            .observe(&u, 900)
            .iter()
            .any(|(n, d)| *n == "state_enter" && d["origin"] == "runtime"));
    }
    #[test]
    fn frontend_rejects_text_and_unknown_fields() {
        assert!(serde_json::from_value::<FrontendEvent>(
            json!({"kind":"frontend_error","view":1,"elapsedMs":3,"message":"secret"})
        )
        .is_err());
        assert!(serde_json::from_value::<FrontendEvent>(
            json!({"kind":"anything","view":1,"elapsedMs":3})
        )
        .is_err());
    }

    #[test]
    fn normalized_state_evidence_excludes_private_values_and_identifiers() {
        let mut usage = crate::model::normalize(
            &json!({"rateLimits":{"limitId":"private-limit","planType":"private-plan","primary":{"usedPercent":37.123,"windowDurationMins":300}},"rateLimitResetCredits":{"availableCount":987}}),
            1,
            &Default::default(),
        );
        usage.account_context_key = Some("private-account-binding".into());
        let evidence = serde_json::to_string(&Observer::default().observe(&usage, 100)).unwrap();
        for private in [
            "private-limit",
            "private-plan",
            "private-account-binding",
            "37.123",
            "62.877",
            "availableCount",
            "remainingPercent",
            "accountContextKey",
        ] {
            assert!(!evidence.contains(private));
        }
        assert!(evidence.contains("five-hour"));
    }

    #[test]
    fn queue_saturation_is_nonblocking_and_counted() {
        let recorder = Recorder::new();
        let (tx, _rx) = mpsc::sync_channel(256);
        recorder.sender.set(tx).ok().unwrap();
        *recorder.config.lock().unwrap() = Arc::new(Configuration {
            segment: 1,
            enabled: true,
            directory: PathBuf::new(),
        });
        recorder.enabled.store(true, Ordering::Relaxed);
        for _ in 0..257 {
            recorder.record("synthetic", json!({}));
        }
        assert_eq!(recorder.dropped.load(Ordering::Relaxed), 1);
        let began = Instant::now();
        recorder.flush(Duration::from_millis(10));
        assert!(began.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn writer_failure_can_retry_and_switch_directories_without_cross_writing() {
        let first = directory("failure");
        let second = directory("switch");
        fs::create_dir_all(&first).unwrap();
        fs::write(first.join("diagnostics"), b"blocked").unwrap();
        let recorder = Recorder::new();
        recorder.configure(true, first.clone());
        recorder.flush(Duration::from_secs(2));
        assert!(recorder.status().error.is_some());
        assert!(!recorder.status().recording);
        fs::remove_file(first.join("diagnostics")).unwrap();
        recorder.configure(true, first.clone());
        recorder.record("first_only", json!({}));
        recorder.flush(Duration::from_secs(2));
        assert!(recorder.status().recording);
        recorder.configure(true, second.clone());
        recorder.record("second_only", json!({}));
        recorder.flush(Duration::from_secs(2));
        recorder.configure(false, second.clone());
        recorder.flush(Duration::from_secs(2));
        let a = fs::read_to_string(first.join("diagnostics/widget.jsonl")).unwrap();
        let b = fs::read_to_string(second.join("diagnostics/widget.jsonl")).unwrap();
        assert!(a.contains("first_only"));
        assert!(!a.contains("second_only"));
        assert!(!b.contains("first_only"));
        assert!(b.contains("second_only"));
        assert!(!recorder.status().recording);
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn failed_rotation_remains_an_error_instead_of_panicking_on_finish() {
        let dir = directory("rotate-failure");
        let config = Configuration {
            segment: 1,
            enabled: true,
            directory: dir.clone(),
        };
        let mut sink = Sink::open(&config, "synthetic", 0).unwrap();
        fs::create_dir_all(dir.join("widget.4.jsonl")).unwrap();
        sink.bytes = MAX_BYTES;
        assert!(sink.write(&json!({"synthetic":true})).is_err());
        assert!(sink.finish().is_err());
        drop(sink);
        fs::remove_dir_all(dir).unwrap();
    }
}

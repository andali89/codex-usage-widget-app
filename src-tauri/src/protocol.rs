//! Read-only Codex protocol adapter. No thread, turn, login, credential-file or redemption APIs.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{mpsc, watch},
};

const FRAME_LIMIT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(15);
const VERSION_TIMEOUT: Duration = Duration::from_secs(2);
const VERSION_OUTPUT_LIMIT: u64 = 4096;

#[derive(Debug)]
pub struct Failure {
    pub code: &'static str,
    pub permanent: bool,
    pub child_exited: bool,
    pub codex_cli_version: Option<String>,
}
impl Failure {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            permanent: false,
            child_exited: code == "codex-exited",
            codex_cli_version: None,
        }
    }
    fn permanent(code: &'static str) -> Self {
        Self {
            code,
            permanent: true,
            child_exited: false,
            codex_cli_version: None,
        }
    }

    pub fn is_incompatible(&self) -> bool {
        matches!(
            self.code,
            "unsupported-account-method"
                | "unsupported-protocol-request"
                | "protocol-frame-too-large"
                | "protocol-malformed-json"
                | "protocol-response-id"
                | "protocol-missing-result"
                | "protocol-invalid-response"
        )
    }
}

pub struct Observation {
    pub raw: Value,
    pub context: String,
    pub dirty: bool,
    pub stable_identity: bool,
}

fn native_candidate(path: &Path) -> bool {
    path.is_absolute()
        && path
            .file_name()
            .is_some_and(|s| s.eq_ignore_ascii_case("codex.exe"))
        && path.is_file()
}

pub fn discover(configured: Option<&str>) -> Result<PathBuf, Failure> {
    if let Some(path) = configured.filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return if native_candidate(&path) {
            Ok(path)
        } else {
            Err(Failure::permanent("select-native-codex-exe"))
        };
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for path in [directory.join("codex.exe"), directory.join("node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe")] {
            if native_candidate(&path) { return Ok(path); }
        }
    }
    Err(Failure::permanent("codex-not-found"))
}

fn command(path: &Path) -> Command {
    let mut command = Command::new(path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    command
}

fn version_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() > VERSION_OUTPUT_LIMIT as usize {
        return None;
    }
    // Version output is diagnostic text, never a parsed compatibility contract.
    let text: String = String::from_utf8_lossy(bytes)
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| {
            !c.is_control() && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect();
    let text: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect();
    (!text.is_empty()).then_some(text)
}

async fn cli_version(path: &Path) -> Option<String> {
    let mut version = command(path);
    version.arg("--version");
    version_from_command(version, VERSION_TIMEOUT).await
}

async fn version_from_command(mut version: Command, timeout: Duration) -> Option<String> {
    // No version text or server stderr is logged. Discard stderr to avoid pipe deadlocks.
    let mut child = version
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    #[cfg(windows)]
    let _job = crate::platform::OwnedJob::attach(&child).ok()?;
    let stdout = child.stdout.take()?;
    let result = tokio::time::timeout(timeout, async {
        let mut bytes = Vec::new();
        stdout
            .take(VERSION_OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .await
            .ok()?;
        if bytes.len() > VERSION_OUTPUT_LIMIT as usize {
            return None;
        }
        let status = child.wait().await.ok()?;
        status.success().then(|| version_text(&bytes)).flatten()
    })
    .await
    .ok()
    .flatten();
    // Also reap a timed-out or over-limit diagnostic child; its result cannot block reads.
    if !child.try_wait().ok().is_some_and(|status| status.is_some()) {
        let _ = child.kill().await;
    }
    result
}

fn response_result(frame: Value, id: u64) -> Result<Value, Failure> {
    if !frame.is_object() || (frame.get("result").is_some() && frame.get("error").is_some()) {
        return Err(Failure::permanent("protocol-invalid-response"));
    }
    if frame.get("id").and_then(Value::as_u64) != Some(id) {
        return Err(Failure::permanent("protocol-response-id"));
    }
    if let Some(error) = frame.get("error") {
        let Some(code) = error.get("code").and_then(Value::as_i64) else {
            return Err(Failure::permanent("protocol-invalid-response"));
        };
        return Err(match code {
            -32601 => Failure::permanent("unsupported-account-method"),
            -32600 | -32602 => Failure::permanent("unsupported-protocol-request"),
            _ => Failure::new("usage-read-failed"),
        });
    }
    let result = frame
        .get("result")
        .ok_or_else(|| Failure::permanent("protocol-missing-result"))?;
    // All three required methods return objects. Nullable quota fields inside them are valid.
    if !result.is_object() {
        return Err(Failure::permanent("protocol-invalid-response"));
    }
    Ok(result.clone())
}

fn validate_usage_response(raw: &Value) -> Result<(), Failure> {
    if !raw.is_object()
        || ["rateLimits", "rateLimitsByLimitId"].iter().any(|key| {
            raw.get(key)
                .is_some_and(|value| !value.is_null() && !value.is_object())
        })
        || raw
            .pointer("/rateLimitsByLimitId/codex")
            .is_some_and(|value| !value.is_null() && !value.is_object())
    {
        return Err(Failure::permanent("protocol-invalid-response"));
    }
    Ok(())
}

async fn read_frame<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<Value>, Failure> {
    // read_until alone allocates without a bound. Inspect the buffered chunks first.
    let mut bytes = Vec::new();
    loop {
        let chunk = reader
            .fill_buf()
            .await
            .map_err(|_| Failure::new("protocol-io"))?;
        if chunk.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(Failure::new("protocol-truncated-frame"))
            };
        }
        let length = chunk
            .iter()
            .position(|b| *b == b'\n')
            .map(|n| n + 1)
            .unwrap_or(chunk.len());
        if bytes.len() + length > FRAME_LIMIT {
            return Err(Failure::permanent("protocol-frame-too-large"));
        }
        let ended = chunk[length - 1] == b'\n';
        bytes.extend_from_slice(&chunk[..length]);
        reader.consume(length);
        if ended {
            return serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| Failure::permanent("protocol-malformed-json"));
        }
    }
}

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: mpsc::Receiver<Result<Value, Failure>>,
    reader: tokio::task::JoinHandle<()>,
    stderr: tokio::task::JoinHandle<()>,
    next_id: u64,
    diagnostic_read_id: u64,
    dirty: bool,
    account_changed: bool,
    #[cfg(windows)]
    _job: crate::platform::OwnedJob,
}

impl Client {
    async fn spawn(path: &Path, proxy_url: Option<&str>) -> Result<Self, Failure> {
        let mut server = command(path);
        crate::proxy::configure_codex(&mut server, proxy_url).map_err(Failure::permanent)?;
        server.args(["app-server", "--listen", "stdio://"]);
        Self::from_command(server).await
    }

    async fn from_command(mut server: Command) -> Result<Self, Failure> {
        let mut child = server.spawn().map_err(|_| Failure::new("codex-launch"))?;
        #[cfg(windows)]
        let job = crate::platform::OwnedJob::attach(&child).map_err(Failure::new)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Failure::new("protocol-io"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Failure::new("protocol-io"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Failure::new("protocol-io"))?;
        let (send, frames) = mpsc::channel(32);
        let reader = tokio::spawn(async move {
            let mut stdout = BufReader::new(stdout);
            loop {
                match read_frame(&mut stdout).await {
                    Ok(Some(value)) => {
                        if send.send(Ok(value)).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = send.send(Err(error)).await;
                        break;
                    }
                }
            }
        });
        // Drain continuously into a fixed buffer. Never retain or log server diagnostics.
        let stderr = tokio::spawn(async move {
            let mut stream = stderr;
            let mut buffer = [0; 4096];
            while let Ok(n) = stream.read(&mut buffer).await {
                if n == 0 {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            frames,
            reader,
            stderr,
            next_id: 1,
            diagnostic_read_id: 0,
            dirty: false,
            account_changed: false,
            #[cfg(windows)]
            _job: job,
        })
    }

    async fn send(&mut self, value: Value) -> Result<(), Failure> {
        let mut bytes = serde_json::to_vec(&value).map_err(|_| Failure::new("protocol-encode"))?;
        bytes.push(b'\n');
        self.stdin
            .as_mut()
            .ok_or_else(|| Failure::new("protocol-closed"))?
            .write_all(&bytes)
            .await
            .map_err(|_| Failure::new("protocol-write"))
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        // All callers are private to this module; this whitelist also makes that boundary testable.
        if !["initialize", "account/read", "account/rateLimits/read"].contains(&method) {
            return Err(Failure::permanent("method-forbidden"));
        }
        let id = self.next_id;
        self.next_id += 1;
        let started = std::time::Instant::now();
        let result=tokio::time::timeout(TIMEOUT, async {
            self.send(json!({"id":id,"method":method,"params":params})).await?;
            loop {
                let frame = self.frames.recv().await.ok_or_else(|| Failure::new("codex-exited"))??;
                if frame.get("method").is_some() {
                    if !frame["method"].is_string() || frame.get("result").is_some() || frame.get("error").is_some() {
                        return Err(Failure::permanent("protocol-invalid-response"));
                    }
                    match frame.get("method").and_then(Value::as_str) {
                        Some("account/updated") => { self.account_changed = true;crate::diagnostics::event("protocol_hint",||json!({"read_id":self.diagnostic_read_id,"trigger":"account_updated"})); },
                        Some("account/rateLimits/updated") => { self.dirty = true;crate::diagnostics::event("protocol_hint",||json!({"read_id":self.diagnostic_read_id,"trigger":"quota_updated"})); },
                        _ => {}
                    }
                    if let Some(request_id) = frame.get("id") {
                        // Never handle approvals, auth-token refresh, or other server work.
                        self.send(json!({"id":request_id,"error":{"code":-32601,"message":"Read-only usage monitor"}})).await?;
                    }
                    continue;
                }
                return response_result(frame, id);
            }
        }).await.unwrap_or_else(|_| Err(Failure::new("protocol-timeout")));
        crate::diagnostics::event(
            "protocol_stage",
            || json!({"read_id":self.diagnostic_read_id,"stage":match method {"initialize"=>"initialize","account/rateLimits/read"=>"quota_read",_ if id<4=>"account_before",_=>"account_after"},"duration_ms":started.elapsed().as_millis() as u64,"error":result.as_ref().err().map(|e|e.code)}),
        );
        result
    }

    async fn transaction(&mut self, salt: &str) -> Result<Observation, Failure> {
        self.request("initialize", json!({"clientInfo":{"name":"codex_usage_widget","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":false,"explicitGatewayOauth":true}})).await?;
        self.send(json!({"method":"initialized"})).await?;
        let before = self
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        let identity = account_identity(&before)?;
        self.account_changed = false;
        self.dirty = false;
        let raw = self.request("account/rateLimits/read", Value::Null).await?;
        validate_usage_response(&raw)?;
        let after = self
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        if identity != account_identity(&after)? || self.account_changed {
            return Err(Failure::new("account-changed-during-read"));
        }
        let account_id = raw
            .get("accountId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 512);
        let stable_identity = account_id.is_some();
        // Installation-salted opaque key. No raw identifier is exposed or persisted.
        // Missing accountId gets a new session key per read and suspends bound reminders.
        let session = format!(
            "{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let binding = account_id.unwrap_or(&session);
        let context = format!(
            "{:x}",
            Sha256::digest(format!("{salt}\0{identity}\0{binding}"))
        );
        Ok(Observation {
            raw,
            context,
            dirty: self.dirty,
            stable_identity,
        })
    }

    async fn shutdown(&mut self) {
        drop(self.stdin.take());
        if tokio::time::timeout(Duration::from_secs(2), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.reader.abort();
        self.stderr.abort();
    }
}

fn account_identity(raw: &Value) -> Result<String, Failure> {
    if !raw.is_object() {
        return Err(Failure::permanent("protocol-invalid-response"));
    }
    let Some(account) = raw.get("account").filter(|a| !a.is_null()) else {
        return Err(Failure::permanent("auth-required"));
    };
    let account_type = account
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::permanent("protocol-invalid-response"))?;
    if account_type != "chatgpt" {
        return Err(Failure::permanent("unsupported-auth-mode"));
    }
    // Email only checks within-read consistency. It is never a persisted account ID.
    Ok(account
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or("session-account")
        .to_owned())
}

pub async fn read_cycle(
    configured: Option<&str>,
    proxy_url: Option<&str>,
    salt: &str,
    mut stop: watch::Receiver<bool>,
    read_id: u64,
) -> Result<Observation, Failure> {
    let cycle_started = std::time::Instant::now();
    let found = discover(configured);
    crate::diagnostics::event(
        "protocol_stage",
        || json!({"read_id":read_id,"stage":"discovery","duration_ms":cycle_started.elapsed().as_millis() as u64,"error":found.as_ref().err().map(|e|e.code)}),
    );
    let path = found?;
    let spawn_started = std::time::Instant::now();
    let mut client = tokio::select! {
        _ = stop.changed() => return Err(Failure::new("shutdown")),
        result = Client::spawn(&path, proxy_url) => {
            crate::diagnostics::event("protocol_stage",||json!({"read_id":read_id,"stage":"launch","duration_ms":spawn_started.elapsed().as_millis() as u64,"error":result.as_ref().err().map(|e|e.code)}));
            result?
        },
    };
    client.diagnostic_read_id = read_id;
    let mut result = tokio::select! {
        _ = stop.changed() => Err(Failure::new("shutdown")),
        result = client.transaction(salt) => result,
    };
    if let Err(error) = &mut result {
        // Count an unexpected exit before our deliberate shutdown. Successful reads
        // and the fresh-process strategy's normal cleanup never trip the circuit.
        if error.code != "shutdown" {
            let exited = client.child.try_wait().ok().flatten().is_some();
            // EOF/broken-pipe reporting can race process-signaling. Observe a brief
            // grace before our shutdown so that the exit is not missed or inferred.
            if exited
                || tokio::time::timeout(Duration::from_millis(100), client.child.wait())
                    .await
                    .is_ok_and(|r| r.is_ok())
            {
                error.child_exited = true;
            }
        }
    }
    let cleanup_started = std::time::Instant::now();
    client.shutdown().await;
    crate::diagnostics::event(
        "protocol_stage",
        || json!({"read_id":read_id,"stage":"cleanup","duration_ms":cleanup_started.elapsed().as_millis() as u64}),
    );
    if let Err(error) = &mut result {
        if error.is_incompatible() {
            error.codex_cli_version = tokio::select! {
                _ = stop.changed() => return Err(Failure::new("shutdown")),
                version = cli_version(&path) => version,
            };
        }
    }
    crate::diagnostics::event(
        "read_complete",
        || json!({"read_id":read_id,"duration_ms":cycle_started.elapsed().as_millis() as u64,"error":result.as_ref().err().map(|e|e.code),"dirty":result.as_ref().is_ok_and(|o|o.dirty)}),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_responses_ignore_additions_and_missing_optional_data() {
        let raw = response_result(json!({"id":1,"result":{"rateLimits":{"primary":null,"secondary":null,"planType":"future-plan"},"futureField":{"anything":true}}}), 1).unwrap();
        validate_usage_response(&raw).unwrap();
        let usage = crate::model::normalize(&raw, 1, &crate::model::Usage::default());
        assert_eq!(usage.plan_type.as_deref(), Some("future-plan"));
        assert!(usage
            .windows
            .iter()
            .all(|window| window.remaining_percent.is_none()));
        for raw in [
            json!({}),
            json!({"rateLimits":null}),
            json!({"rateLimitsByLimitId":{"codex":{}}}),
        ] {
            validate_usage_response(&raw).unwrap();
        }
    }

    #[test]
    fn protocol_failures_are_distinct_from_transient_and_auth_failures() {
        for frame in [
            json!(null),
            json!({"id":2,"result":{}}),
            json!({"id":1}),
            json!({"id":1,"result":null}),
            json!({"id":1,"result":{},"error":{"code":-32000}}),
            json!({"id":1,"error":{}}),
        ] {
            let error = response_result(frame, 1).unwrap_err();
            assert!(error.is_incompatible());
            assert!(error.permanent);
        }
        for code in [-32600, -32601, -32602] {
            assert!(response_result(json!({"id":1,"error":{"code":code}}), 1)
                .unwrap_err()
                .is_incompatible());
        }
        let transient = response_result(json!({"id":1,"error":{"code":-32000}}), 1).unwrap_err();
        assert!(!transient.is_incompatible());
        assert!(!transient.permanent);
        for code in [
            "protocol-timeout",
            "protocol-io",
            "protocol-truncated-frame",
            "codex-exited",
            "auth-required",
            "unsupported-auth-mode",
            "account-changed-during-read",
        ] {
            assert!(!Failure::new(code).is_incompatible());
        }
        for raw in [
            json!([]),
            json!({"rateLimits":"changed shape"}),
            json!({"rateLimitsByLimitId":[]}),
            json!({"rateLimitsByLimitId":{"codex":false}}),
        ] {
            assert!(validate_usage_response(&raw).unwrap_err().is_incompatible());
        }
        assert!(account_identity(&json!({"account":true}))
            .unwrap_err()
            .is_incompatible());
    }

    #[test]
    fn version_text_is_bounded_display_text_without_format_assumptions() {
        for output in [
            "codex-cli 0.163.0",
            "codex-cli 0.164.0",
            "future nightly / custom build",
            "unknown",
            "v999-beta+custom",
        ] {
            assert_eq!(version_text(output.as_bytes()).as_deref(), Some(output));
        }
        assert_eq!(
            version_text(b"  nightly\n\tcustom\0\x07  ").as_deref(),
            Some("nightly custom")
        );
        assert_eq!(
            version_text("\u{202e}nightly\u{2069}".as_bytes()).as_deref(),
            Some("nightly")
        );
        assert!(version_text(b" \n\t").is_none());
        assert!(version_text(&vec![b'a'; 4097]).is_none());
        assert_eq!(
            version_text("\u{754c}".repeat(250).as_bytes())
                .unwrap()
                .chars()
                .count(),
            200
        );
    }

    // Allow for cold PowerShell startup on hosted Windows runners. Production
    // version queries retain VERSION_TIMEOUT; timeout behavior is tested below.
    #[cfg(windows)]
    const POWERSHELL_FIXTURE_TIMEOUT: Duration = Duration::from_secs(15);

    #[cfg(windows)]
    fn powershell(script: &str) -> Command {
        let mut process = command(Path::new("powershell.exe"));
        process.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        process
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn diagnostic_version_query_is_best_effort_and_times_out() {
        assert_eq!(
            version_from_command(
                powershell("Write-Output 'future custom build'; exit 0"),
                POWERSHELL_FIXTURE_TIMEOUT
            )
            .await
            .as_deref(),
            Some("future custom build")
        );
        for script in [
            "Write-Output 'ignored'; exit 2",
            "exit 0",
            "Write-Output ('x' * 4097)",
        ] {
            assert!(
                version_from_command(powershell(script), POWERSHELL_FIXTURE_TIMEOUT)
                    .await
                    .is_none()
            );
        }
        assert!(version_from_command(
            powershell("Start-Sleep -Seconds 30"),
            Duration::from_millis(50)
        )
        .await
        .is_none());
        assert!(version_from_command(
            command(Path::new("nonexistent-codex-version-test.exe")),
            VERSION_TIMEOUT
        )
        .await
        .is_none());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn proxy_environment_reaches_only_the_owned_child() {
        let mut child = powershell("if ($env:HTTPS_PROXY -ne 'http://127.0.0.1:7890' -or $env:HTTP_PROXY -ne 'http://127.0.0.1:7890' -or $env:NO_PROXY -ne 'localhost,127.0.0.1,::1') { exit 2 }; [Console]::WriteLine('child-proxy-configured')");
        let previous = std::env::var_os("HTTPS_PROXY");
        crate::proxy::apply(&mut child, Some("http://127.0.0.1:7890")).unwrap();
        assert_eq!(
            version_from_command(child, POWERSHELL_FIXTURE_TIMEOUT)
                .await
                .as_deref(),
            Some("child-proxy-configured")
        );
        assert_eq!(std::env::var_os("HTTPS_PROXY"), previous);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn read_only_transaction_accepts_future_fields_without_version_negotiation() {
        // A native pipe fixture: only the existing four protocol messages are accepted.
        let mut client = Client::from_command(powershell(r#"
            $ErrorActionPreference = 'Stop'
            while ($null -ne ($line = [Console]::ReadLine())) {
                $request = ConvertFrom-Json -InputObject $line
                switch ($request.method) {
                    'initialize' { $result = @{ futureCapability = $true } }
                    'initialized' { continue }
                    'account/read' { $result = @{ account = @{ type = 'chatgpt'; email = 'synthetic'; planType = 'future-plan' }; requiresOpenaiAuth = $true } }
                    'account/rateLimits/read' { $result = @{ rateLimits = @{ primary = @{ usedPercent = 26; windowDurationMins = 300 }; planType = 'future-plan' }; accountId = 'synthetic'; futureField = $true } }
                    default { exit 3 }
                }
                if ($request.method -ne 'initialized') {
                    [Console]::WriteLine((@{ id = $request.id; result = $result } | ConvertTo-Json -Depth 8 -Compress))
                }
            }
        "#)).await.unwrap();
        let observation = client.transaction("synthetic-only").await;
        client.shutdown().await;
        let observation = observation.unwrap();
        assert!(observation.stable_identity);
        assert!(!observation.dirty);
        let usage = crate::model::normalize(&observation.raw, 1, &crate::model::Usage::default());
        assert_eq!(usage.plan_type.as_deref(), Some("future-plan"));
        assert_eq!(usage.windows[0].remaining_percent, Some(74.0));
        assert!(client.child.try_wait().unwrap().is_some());
    }
    #[tokio::test]
    async fn bounded_frames_reject_malformed_oversized_and_truncated() {
        assert_eq!(
            read_frame(&mut &b"{\"id\":1}\n"[..])
                .await
                .unwrap()
                .unwrap()["id"],
            1
        );
        for bytes in [
            b"invalid\n".to_vec(),
            vec![b'a'; FRAME_LIMIT + 1],
            b"{\"id\":1}".to_vec(),
        ] {
            assert!(read_frame(&mut bytes.as_slice()).await.is_err());
        }
    }
    #[test]
    fn unsupported_auth_never_exposes_chatgpt_quotas() {
        assert_eq!(
            account_identity(&json!({"account":null})).unwrap_err().code,
            "auth-required"
        );
        assert_eq!(
            account_identity(&json!({"account":{"type":"apiKey"}}))
                .unwrap_err()
                .code,
            "unsupported-auth-mode"
        );
        assert!(discover(Some("codex.cmd app-server")).is_err());
    }
    #[tokio::test]
    #[ignore = "reads local signed-in Codex usage; run explicitly, no inference"]
    async fn live_read_only_protocol_gate() {
        let (_send, stop) = watch::channel(false);
        let proxy_url = std::env::var("CODEX_USAGE_WIDGET_PROBE_PROXY").ok();
        let result = read_cycle(None, proxy_url.as_deref(), "live-probe-session", stop, 0)
            .await
            .expect("read-only protocol gate failed (safe code)");
        let normalized = crate::model::normalize(
            &result.raw,
            crate::model::now_ms(),
            &crate::model::Usage::default(),
        );
        assert!(!result.dirty, "read invalidated by push; retry explicitly");
        assert!(
            normalized.selected_limit_id.is_some(),
            "no recognized family"
        );
        println!("handshake=pass account_consistency=pass usage_read=pass proxy_configured={} stable_identity_present={} window_count={} optional_allowed_present={} optional_reset_credit_count_present={}", proxy_url.is_some(), result.stable_identity, normalized.windows.len(), normalized.ordinary_usage_allowed.is_some(), normalized.reset_credits_available.is_some());
    }
}

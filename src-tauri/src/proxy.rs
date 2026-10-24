//! Per-child HTTP proxy configuration; never changes the parent environment or Codex files.
use tokio::process::Command;

const PROXY_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "WS_PROXY",
    "WSS_PROXY",
];

pub fn validate(address: &str) -> Result<(), &'static str> {
    let invalid = || "invalid-proxy-url";
    if address.len() > 2048
        || address.chars().any(|c| c.is_whitespace() || c.is_control())
        || address.contains('@')
    {
        return Err(invalid());
    }
    let url = tauri::Url::parse(address).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.port() == Some(0)
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn apply(command: &mut Command, address: Option<&str>) -> Result<(), &'static str> {
    let Some(address) = address else {
        return Ok(());
    };
    validate(address)?;
    // Explicit custom settings override inherited bypass lists. Loopback remains local.
    // Set both spellings for clients which look up lowercase names on other platforms;
    // std::process::Command treats these case-insensitively on Windows.
    for key in PROXY_KEYS
        .iter()
        .copied()
        .chain(std::iter::once("NO_PROXY"))
    {
        let value = if key == "NO_PROXY" {
            "localhost,127.0.0.1,::1"
        } else {
            address
        };
        command.env(key, value).env(key.to_ascii_lowercase(), value);
    }
    Ok(())
}

pub fn configure_codex(command: &mut Command, address: Option<&str>) -> Result<(), &'static str> {
    apply(command, address)?;
    if address.is_some() {
        // A selected proxy must not be superseded by Codex's system/PAC route or
        // bootstrap system-proxy fallback. These are process-only config overrides.
        command.args([
            "--config",
            "features.respect_system_proxy=false",
            "--config",
            "features.system_proxy_fallback=false",
        ]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_urls_accept_http_without_credentials_and_reject_ambiguous_inputs() {
        for address in [
            "http://127.0.0.1:7890",
            "https://proxy.example:8443/",
            "http://[::1]:3128",
            "http://localhost",
        ] {
            validate(address).unwrap();
        }
        for address in [
            "",
            "localhost:7890",
            "socks5://localhost:1080",
            "http://user:pass@host",
            "http://@host",
            "http://host/path",
            "http://host?x",
            "http://host#x",
            "http://host:0",
            "http://host\n",
            "http://host:99999",
        ] {
            assert!(validate(address).is_err(), "accepted invalid proxy");
        }
    }
    #[test]
    fn custom_proxy_is_scoped_to_child_and_blank_preserves_inheritance() {
        let mut child = Command::new("unused");
        apply(&mut child, None).unwrap();
        assert_eq!(child.as_std().get_envs().count(), 0);
        child
            .env("https_proxy", "http://old.example:3128")
            .env("NO_PROXY", "*");
        apply(&mut child, Some("http://127.0.0.1:7890")).unwrap();
        for (key, value) in child.as_std().get_envs() {
            let key = key.to_string_lossy().to_ascii_uppercase();
            let value = value.unwrap().to_string_lossy();
            if key == "NO_PROXY" {
                assert_eq!(value, "localhost,127.0.0.1,::1");
            } else {
                assert_eq!(value, "http://127.0.0.1:7890");
            }
        }
    }

    #[test]
    fn explicit_proxy_disables_system_route_overrides_without_touching_default_launches() {
        let mut inherited = Command::new("codex.exe");
        configure_codex(&mut inherited, None).unwrap();
        assert_eq!(inherited.as_std().get_args().count(), 0);
        let mut custom = Command::new("codex.exe");
        configure_codex(&mut custom, Some("http://127.0.0.1:7890")).unwrap();
        assert_eq!(
            custom.as_std().get_args().collect::<Vec<_>>(),
            [
                "--config",
                "features.respect_system_proxy=false",
                "--config",
                "features.system_proxy_fallback=false"
            ]
        );
    }
}

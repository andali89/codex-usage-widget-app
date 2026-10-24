//! Settings, reminder episodes and submission claims share one checksummed transaction.
//! Raw protocol responses, credentials and account identifiers are never stored.
use crate::model::Settings;
use crate::reminders::Durable;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
const MAX_DOCUMENT_BYTES: usize = 8 * 1_048_576;

pub(crate) fn new_profile_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{:x}",
        Sha256::digest(format!(
            "{:?}-{}-{}",
            std::time::SystemTime::now(),
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    )
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: u32,
    checksum: String,
    settings: serde_json::Value,
}

pub struct Store {
    pub(crate) path: PathBuf,
    pub(crate) profile_id: String,
    pub(crate) data_revision: std::cell::Cell<u64>,
    pub blocked: bool,
}

pub struct Loaded {
    pub settings: Settings,
    pub reminders: Durable,
    pub diagnostic: Option<String>,
    pub first_launch: bool,
    pub profile_id: String,
    pub data_revision: u64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    settings: Settings,
    reminders: Durable,
    profile_id: String,
    data_revision: u64,
}

fn decode(bytes: &[u8]) -> Result<Document, &'static str> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| "storage-corrupt")?;
    if ![1, 2, 3, 4].contains(&envelope.format) {
        return Err("storage-future-version");
    }
    let body = serde_json::to_vec(&envelope.settings).map_err(|_| "storage-corrupt")?;
    if format!("{:x}", Sha256::digest(body)) != envelope.checksum {
        return Err("storage-checksum");
    }
    let metadata = envelope.settings.clone();
    let (mut value, reminders) = if envelope.format == 1 {
        (envelope.settings, Durable::default())
    } else {
        let doc = envelope.settings.as_object().ok_or("storage-corrupt")?;
        if doc.len() != if envelope.format == 4 { 4 } else { 2 } {
            return Err("storage-corrupt");
        }
        (
            doc.get("settings").ok_or("storage-corrupt")?.clone(),
            serde_json::from_value(doc.get("reminders").ok_or("storage-corrupt")?.clone())
                .map_err(|_| "storage-corrupt")?,
        )
    };
    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if version > 5 {
        return Err("storage-future-version");
    }
    if version < 5 {
        let obj = value.as_object_mut().ok_or("storage-corrupt")?;
        obj.insert("schemaVersion".into(), 5.into());
        if !obj
            .get("themePreference")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| ["system", "light", "dark"].contains(&s))
        {
            obj.insert("themePreference".into(), "system".into());
        }
    }
    let settings: Settings = serde_json::from_value(value).map_err(|_| "storage-corrupt")?;
    settings.validate()?;
    reminders.validate()?;
    let profile_id = if envelope.format == 4 {
        metadata
            .get("profileId")
            .and_then(serde_json::Value::as_str)
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("storage-corrupt")?
            .to_owned()
    } else {
        String::new()
    };
    let data_revision = if envelope.format == 4 {
        metadata
            .get("dataRevision")
            .and_then(serde_json::Value::as_u64)
            .ok_or("storage-corrupt")?
    } else {
        0
    };
    Ok(Document {
        settings,
        reminders,
        profile_id,
        data_revision,
    })
}

fn read(path: &Path) -> Result<Document, &'static str> {
    let metadata = fs::metadata(path).map_err(|_| "storage-read")?;
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err("storage-corrupt");
    }
    decode(&fs::read(path).map_err(|_| "storage-read")?)
}

impl Store {
    pub fn open(directory: &Path) -> (Self, Loaded) {
        let path = directory.join("state.json");
        let first_launch = !path.exists() && !directory.join("state.backup.json").exists();
        let mut store = Self {
            path,
            profile_id: String::new(),
            data_revision: std::cell::Cell::new(0),
            blocked: false,
        };
        let (mut document, diagnostic) = if first_launch {
            (Document::default(), None)
        } else {
            match read(&store.path) {
                Ok(settings) => (settings, None),
                Err("storage-future-version") => {
                    store.blocked = true;
                    (Document::default(), Some("storage-future-version".into()))
                }
                Err(_) => match read(&directory.join("state.backup.json")) {
                    Ok(settings) => (settings, Some("storage-recovered-backup".into())),
                    Err("storage-future-version") => {
                        store.blocked = true;
                        (Document::default(), Some("storage-future-version".into()))
                    }
                    Err(_) => (Document::default(), Some("storage-unrecoverable".into())),
                },
            }
        };
        if diagnostic.is_some() {
            document.reminders.recover(crate::model::now_ms());
            document.settings.reminders.enabled = false;
        }
        if document.profile_id.is_empty() {
            document.profile_id = if first_launch {
                new_profile_id()
            } else {
                format!("{:x}", Sha256::digest(serde_json::to_vec(&serde_json::json!({"settings":document.settings,"reminders":document.reminders})).unwrap_or_default()))
            };
        }
        store.profile_id = document.profile_id.clone();
        store.data_revision.set(document.data_revision);
        (
            store,
            Loaded {
                settings: document.settings,
                reminders: document.reminders,
                diagnostic,
                first_launch,
                profile_id: document.profile_id,
                data_revision: document.data_revision,
            },
        )
    }

    pub fn save(&self, settings: &Settings, reminders: &Durable) -> Result<(), &'static str> {
        if self.blocked {
            return Err("storage-future-version");
        }
        settings.validate()?;
        reminders.validate()?;
        let parent = self.path.parent().ok_or("storage-path")?;
        fs::create_dir_all(parent).map_err(|_| "storage-unwritable")?;
        let revision = self
            .data_revision
            .get()
            .checked_add(1)
            .ok_or("storage-capacity")?;
        let value = serde_json::json!({"settings":settings,"reminders":reminders,"profileId":self.profile_id,"dataRevision":revision});
        let body = serde_json::to_vec(&value).map_err(|_| "storage-serialize")?;
        let envelope = Envelope {
            format: 4,
            checksum: format!("{:x}", Sha256::digest(body)),
            settings: value,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| "storage-serialize")?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err("storage-capacity");
        }
        let pending = parent.join(format!("state.{}.pending", std::process::id()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&pending)
                .map_err(|_| "storage-unwritable")?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "storage-write")?;
            drop(file);
            replace(
                &pending,
                &self.path,
                read(&self.path)
                    .is_ok()
                    .then(|| parent.join("state.backup.json")),
            )
        })();
        if result.is_err() {
            let _ = fs::remove_file(&pending);
        }
        if result.is_ok() {
            self.data_revision.set(revision);
        }
        result
    }
}

#[cfg(windows)]
pub(crate) fn replace(
    source: &Path,
    destination: &Path,
    backup: Option<PathBuf>,
) -> Result<(), &'static str> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let wide = |p: &Path| -> Result<Vec<u16>, &'static str> {
        let absolute = std::path::absolute(p).map_err(|_| "storage-path")?;
        let units = absolute.as_os_str().encode_wide().collect::<Vec<_>>();
        let extended = "\\\\?\\".encode_utf16().collect::<Vec<_>>();
        let mut result = if units.starts_with(&extended) {
            units
        } else if units.starts_with(&[92, 92]) {
            "\\\\?\\UNC\\"
                .encode_utf16()
                .chain(units.into_iter().skip(2))
                .collect()
        } else {
            extended.into_iter().chain(units).collect()
        };
        result.push(0);
        Ok(result)
    };
    let source = wide(source)?;
    let destination = wide(destination)?;
    // ReplaceFile retains a last-good backup only when the outgoing file validated.
    let ok = unsafe {
        if let Some(backup) = backup {
            let backup = wide(&backup)?;
            ReplaceFileW(
                destination.as_ptr(),
                source.as_ptr(),
                backup.as_ptr(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } else {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if ok == 0 {
        Err("storage-commit")
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path, backup: Option<PathBuf>) -> Result<(), &'static str> {
    if let Some(backup) = backup {
        fs::copy(destination, backup).map_err(|_| "storage-backup")?;
    }
    fs::rename(source, destination).map_err(|_| "storage-commit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ThemePreference;
    #[test]
    fn schema_four_migrates_new_preferences_without_losing_reminder_claims() {
        let mut engine = crate::reminders::Engine::new(Durable::default(), false);
        engine.test_claim(1000, 1).unwrap();
        let settings = serde_json::json!({"settings":{"schemaVersion":4,"themePreference":"dark"},"reminders":engine.data});
        let body = serde_json::to_vec(&settings).unwrap();
        let bytes = serde_json::to_vec(&Envelope {
            format: 3,
            checksum: format!("{:x}", Sha256::digest(body)),
            settings,
        })
        .unwrap();
        let migrated = decode(&bytes).unwrap();
        assert_eq!(migrated.settings.schema_version, 5);
        assert_eq!(migrated.settings.theme_preference, ThemePreference::Dark);
        assert!(migrated.settings.proxy_url.is_none());
        assert_eq!(
            migrated.settings.storage_location,
            crate::model::StorageLocation::Executable
        );
        assert!(migrated.settings.windows.visible_periods.is_empty());
        assert_eq!(migrated.reminders.history.len(), 1);
    }
    fn directory() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "codex-widget-storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    #[test]
    fn atomic_roundtrip_and_corruption_recovery() {
        let dir = directory();
        let (store, loaded) = Store::open(&dir);
        assert!(loaded.first_launch);
        let mut s = loaded.settings;
        s.theme_preference = ThemePreference::Dark;
        store.save(&s, &Durable::default()).unwrap();
        s.revision = 1;
        store.save(&s, &Durable::default()).unwrap();
        assert_eq!(Store::open(&dir).1.settings.revision, 1);
        fs::write(dir.join("state.json"), "corrupt").unwrap();
        let (_, recovered) = Store::open(&dir);
        assert_eq!(recovered.settings.theme_preference, ThemePreference::Dark);
        assert_eq!(recovered.settings.revision, 0);
        assert_eq!(
            recovered.diagnostic.as_deref(),
            Some("storage-recovered-backup")
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_commit_does_not_report_success() {
        let dir = directory();
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir(dir.join("state.json")).unwrap();
        let (store, _) = Store::open(&dir);
        assert!(store
            .save(&Settings::default(), &Durable::default())
            .is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn future_backup_cannot_be_overwritten_after_primary_corruption() {
        let dir = directory();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("state.json"), "corrupt").unwrap();
        let settings = serde_json::json!({"schemaVersion":99});
        let body = serde_json::to_vec(&settings).unwrap();
        let backup = serde_json::to_vec(&Envelope {
            format: 1,
            checksum: format!("{:x}", Sha256::digest(body)),
            settings,
        })
        .unwrap();
        fs::write(dir.join("state.backup.json"), &backup).unwrap();
        let (store, loaded) = Store::open(&dir);
        assert_eq!(loaded.diagnostic.as_deref(), Some("storage-future-version"));
        assert_eq!(
            store.save(&Settings::default(), &Durable::default()),
            Err("storage-future-version")
        );
        assert_eq!(fs::read(dir.join("state.backup.json")).unwrap(), backup);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn migration_preserves_theme_and_rejects_future_versions() {
        for (version, theme, expected) in [
            (0, "dark", Some(ThemePreference::Dark)),
            (0, "unknown", Some(ThemePreference::System)),
            (9, "light", None),
        ] {
            let settings = serde_json::json!({"schemaVersion":version,"themePreference":theme});
            let body = serde_json::to_vec(&settings).unwrap();
            let bytes = serde_json::to_vec(&Envelope {
                format: 1,
                checksum: format!("{:x}", Sha256::digest(body)),
                settings,
            })
            .unwrap();
            match expected {
                Some(theme) => assert_eq!(decode(&bytes).unwrap().settings.theme_preference, theme),
                None => assert!(decode(&bytes).is_err()),
            }
        }
    }
    #[test]
    fn settings_three_migrate_with_sign_in_disabled_and_four_preserves_pending_intent() {
        let encode = |settings| {
            let body = serde_json::to_vec(&settings).unwrap();
            serde_json::to_vec(&Envelope {
                format: 1,
                checksum: format!("{:x}", Sha256::digest(body)),
                settings,
            })
            .unwrap()
        };
        let migrated = decode(&encode(
            serde_json::json!({"schemaVersion":3,"themePreference":"dark"}),
        ))
        .unwrap();
        assert_eq!(migrated.settings.schema_version, 5);
        assert_eq!(migrated.settings.theme_preference, ThemePreference::Dark);
        assert!(!migrated.settings.autostart.requested);
        assert!(!migrated.settings.autostart.pending);
        let settings = serde_json::to_value(Settings {
            autostart: crate::lifecycle::Autostart::default().request(true),
            ..Default::default()
        })
        .unwrap();
        let pending = decode(&encode(settings)).unwrap();
        assert!(pending.settings.autostart.requested);
        assert!(pending.settings.autostart.pending);
        assert_eq!(pending.settings.autostart.effective, None);
    }
    #[test]
    fn format_two_migrates_and_format_three_keeps_test_claims_and_blocks_future_data() {
        let settings =
            serde_json::json!({"settings":Settings::default(),"reminders":Durable::default()});
        let body = serde_json::to_vec(&settings).unwrap();
        let bytes = serde_json::to_vec(&Envelope {
            format: 2,
            checksum: format!("{:x}", Sha256::digest(body)),
            settings,
        })
        .unwrap();
        let old = decode(&bytes).unwrap();
        let dir = directory();
        let (store, _) = Store::open(&dir);
        let mut engine = crate::reminders::Engine::new(old.reminders, false);
        engine.test_claim(1000, 1).unwrap();
        store.save(&old.settings, &engine.data).unwrap();
        let written = fs::read(dir.join("state.json")).unwrap();
        let mut envelope: Envelope = serde_json::from_slice(&written).unwrap();
        assert_eq!(envelope.format, 4);
        let loaded = decode(&written).unwrap();
        assert_eq!(
            loaded.reminders.history[0].events[0].category,
            crate::reminders::Category::Test
        );
        envelope.format = 5;
        assert_eq!(
            decode(&serde_json::to_vec(&envelope).unwrap()).err(),
            Some("storage-future-version")
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rolled_back_ledger_disables_delivery_until_explicit_enablement() {
        let dir = directory();
        let (store, mut loaded) = Store::open(&dir);
        loaded.settings.reminders.enabled = true;
        let mut ledger = Durable::default();
        let mut episode = crate::reminders::Episode::default();
        episode.handled.insert(20);
        ledger.episodes.insert("opaque|window".into(), episode);
        store.save(&loaded.settings, &ledger).unwrap();
        loaded.settings.revision += 1;
        ledger.sequence = 1;
        store.save(&loaded.settings, &ledger).unwrap();
        fs::write(dir.join("state.json"), "corrupt").unwrap();
        let (_, recovered) = Store::open(&dir);
        assert!(!recovered.settings.reminders.enabled);
        assert!(recovered.reminders.delivery_paused);
        assert!(recovered.reminders.episodes["opaque|window"]
            .handled
            .contains(&20));
        assert!(recovered.reminders.episodes["opaque|window"].rebase);
        assert!(recovered.reminders.schedule_scanned_at.is_some());
        fs::remove_dir_all(dir).unwrap();
    }
}

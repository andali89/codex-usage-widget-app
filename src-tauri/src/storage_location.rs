//! Owns the configuration writer, portable discovery, and journaled relocation.
use crate::{
    model::{Settings, StorageLocation},
    reminders::Durable,
    storage::{self, Loaded, Store},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

const LOCATOR_LIMIT: u64 = 256 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub preferred: StorageLocation,
    pub active_directory: String,
    pub fallback: bool,
    pub diagnostic: Option<String>,
    pub portable_locator: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resolution {
    CopyCurrent,
    UseDestination,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub token: String,
    pub conflict: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    id: String,
    directory: PathBuf,
    previous_directory: PathBuf,
    original_hash: String,
    snapshot_hash: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Locator {
    format: u32,
    revision: u64,
    preferred: StorageLocation,
    active_directory: PathBuf,
    profile_id: String,
    pending: Option<Pending>,
}

struct Lease {
    _file: File,
}
impl Lease {
    fn acquire(directory: &Path) -> Result<Self, &'static str> {
        fs::create_dir_all(directory).map_err(|_| "storage-unwritable")?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        Ok(Self {
            _file: options
                .open(directory.join("state.lock"))
                .map_err(|_| "storage-in-use")?,
        })
    }
}

struct Inner {
    executable: PathBuf,
    appdata: PathBuf,
    fallback_locator: PathBuf,
    store: Store,
    lease: Option<Lease>,
    locator: Locator,
    diagnostic: Option<String>,
    portable_locator: bool,
}

pub struct Manager(Mutex<Inner>);

fn key(path: &Path) -> String {
    // Resolve the existing parent too: creating a new directory must not change
    // its identity or invalidate the destination confirmation token.
    let mut parent = path;
    let mut suffix = Vec::new();
    let mut resolved = loop {
        if let Ok(canonical) = parent.canonicalize() {
            break canonical;
        }
        match (parent.parent(), parent.file_name()) {
            (Some(next), Some(name)) => {
                suffix.push(name.to_owned());
                parent = next;
            }
            _ => break path.to_owned(),
        }
    };
    for name in suffix.into_iter().rev() {
        resolved.push(name);
    }
    let text = resolved.to_string_lossy().to_string();
    #[cfg(windows)]
    let text = if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        text.strip_prefix("\\\\?\\").unwrap_or(&text).to_owned()
    }
    .replace('/', "\\")
    .to_lowercase();
    text
}

fn digest(path: &Path) -> Result<String, &'static str> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= 8 * 1_048_576 => Ok(format!(
            "{:x}",
            Sha256::digest(fs::read(path).map_err(|_| "storage-read")?)
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        _ => Err("storage-corrupt"),
    }
}

fn writable(directory: &Path) -> Result<(), &'static str> {
    fs::create_dir_all(directory).map_err(|_| "storage-unwritable")?;
    let probe = directory.join(format!(".widget-write-{}", storage::new_profile_id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .map_err(|_| "storage-unwritable")?;
        file.write_all(b"probe")
            .and_then(|_| file.sync_all())
            .map_err(|_| "storage-unwritable")
    })();
    let _ = fs::remove_file(probe);
    result
}

fn read_locator(path: &Path) -> Result<Option<Locator>, &'static str> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("storage-read"),
    };
    if !metadata.is_file() || metadata.len() > LOCATOR_LIMIT {
        return Err("storage-locator-invalid");
    }
    let bytes = fs::read(path).map_err(|_| "storage-read")?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "storage-locator-invalid")?;
    if value
        .get("format")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|f| f > 1)
    {
        return Err("storage-future-version");
    }
    let locator: Locator = serde_json::from_value(value).map_err(|_| "storage-locator-invalid")?;
    locator.preferred.validate()?;
    if locator.format != 1
        || locator.revision > u64::MAX - 4
        || !locator.active_directory.is_absolute()
        || locator.profile_id.len() != 64
        || !locator.profile_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("storage-locator-invalid");
    }
    if let Some(pending) = &locator.pending {
        if pending.id.len() != 64
            || !pending.id.bytes().all(|b| b.is_ascii_hexdigit())
            || pending.snapshot_hash.len() != 64
            || !pending.snapshot_hash.bytes().all(|b| b.is_ascii_hexdigit())
            || (!pending.original_hash.is_empty()
                && (pending.original_hash.len() != 64
                    || !pending.original_hash.bytes().all(|b| b.is_ascii_hexdigit())))
            || !pending.directory.is_absolute()
            || pending.directory != locator.active_directory
            || !pending.previous_directory.is_absolute()
        {
            return Err("storage-locator-invalid");
        }
    }
    Ok(Some(locator))
}

fn atomic_locator(path: &Path, locator: &Locator) -> Result<(), &'static str> {
    let parent = path.parent().ok_or("storage-path")?;
    fs::create_dir_all(parent).map_err(|_| "storage-unwritable")?;
    let bytes = serde_json::to_vec(locator).map_err(|_| "storage-serialize")?;
    if path.is_file() && read_locator(path).is_err() {
        if fs::metadata(path).map_err(|_| "storage-read")?.len() > LOCATOR_LIMIT {
            return Err("storage-locator-invalid");
        }
        let archive = parent.join(format!(
            "location.{}.{}.previous",
            crate::model::now_ms(),
            storage::new_profile_id()
        ));
        let contents = fs::read(path).map_err(|_| "storage-read")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(archive)
            .map_err(|_| "storage-backup")?;
        file.write_all(&contents)
            .and_then(|_| file.sync_all())
            .map_err(|_| "storage-backup")?;
    }
    let pending = parent.join(format!(".location-{}.pending", storage::new_profile_id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(|_| "storage-unwritable")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "storage-write")?;
        drop(file);
        storage::replace(
            &pending,
            path,
            read_locator(path)
                .ok()
                .flatten()
                .map(|_| path.with_extension("backup.json")),
        )
    })();
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
}

impl Inner {
    fn directory(&self, selection: &StorageLocation) -> PathBuf {
        match selection {
            StorageLocation::Executable => self.executable.clone(),
            StorageLocation::AppData => self.appdata.clone(),
            StorageLocation::Custom { directory } => PathBuf::from(directory),
        }
    }

    fn commit_locator(&mut self, locator: &Locator) -> Result<(), &'static str> {
        let portable = self.executable.join("storage-location.json");
        for (path, is_portable) in [(&portable, true), (&self.fallback_locator, false)] {
            if matches!(read_locator(path), Err("storage-future-version")) {
                return Err("storage-future-version");
            }
            if atomic_locator(path, locator).is_ok() {
                self.portable_locator = is_portable;
                return Ok(());
            }
        }
        Err("storage-locator-unwritable")
    }

    fn inspection(&self, selection: &StorageLocation) -> Result<Inspection, &'static str> {
        selection.validate()?;
        let directory = self.directory(selection);
        let hash = digest(&directory.join("state.json"))?;
        let token = format!(
            "{:x}",
            Sha256::digest(format!(
                "{}\0{}\0{}",
                key(&directory),
                hash,
                digest(&directory.join("state.backup.json"))?
            ))
        );
        let (destination, loaded) = Store::open(&directory);
        if destination.blocked {
            return Err("storage-future-version");
        }
        if loaded.diagnostic.is_some() {
            return Err("storage-destination-invalid");
        }
        let conflict = !loaded.first_launch
            && key(&directory) != key(self.store.path.parent().ok_or("storage-path")?);
        Ok(Inspection { token, conflict })
    }

    fn stage_and_switch(
        &mut self,
        directory: &Path,
        settings: &Settings,
        ledger: &Durable,
        source: Option<&Loaded>,
        expected_token: Option<&str>,
    ) -> Result<(), &'static str> {
        writable(directory)?;
        let lease = if key(directory) == key(&self.locator.active_directory) && self.lease.is_some()
        {
            None
        } else {
            Some(Lease::acquire(directory)?)
        };
        if expected_token.is_some_and(|token| {
            self.inspection(&settings.storage_location)
                .map_or(true, |inspection| inspection.token != token)
        }) {
            return Err("storage-destination-changed");
        }
        let (mut target, loaded) = Store::open(directory);
        if target.blocked {
            return Err("storage-future-version");
        }
        if loaded.diagnostic.is_some() {
            return Err("storage-destination-invalid");
        }
        let previous = self.locator.clone();
        let id = storage::new_profile_id();
        let staged = directory.join(format!("state.{id}.staged"));
        let original_hash = digest(&target.path)?;
        target.profile_id =
            source.map_or_else(|| self.store.profile_id.clone(), |s| s.profile_id.clone());
        let revision = source.map_or_else(|| self.store.data_revision.get(), |s| s.data_revision);
        target
            .data_revision
            .set(revision.max(if loaded.profile_id == target.profile_id {
                loaded.data_revision
            } else {
                0
            }));
        let final_path = target.path.clone();
        target.path = staged.clone();
        target.save(settings, ledger)?;
        target.path = final_path;
        let mut locator = Locator {
            format: 1,
            revision: previous.revision.checked_add(1).ok_or("storage-capacity")?,
            preferred: settings.storage_location.clone(),
            active_directory: directory.to_owned(),
            profile_id: target.profile_id.clone(),
            pending: Some(Pending {
                id,
                directory: directory.to_owned(),
                previous_directory: previous.active_directory.clone(),
                original_hash,
                snapshot_hash: digest(&staged)?,
            }),
        };
        if let Err(error) = self.commit_locator(&locator) {
            let _ = fs::remove_file(staged);
            return Err(error);
        }
        if let Err(error) = activate(
            locator
                .pending
                .as_ref()
                .ok_or("storage-relocation-interrupted")?,
        ) {
            let mut rollback = previous;
            rollback.revision = locator.revision + 1;
            if self.commit_locator(&rollback).is_err() {
                self.store.blocked = true;
                self.diagnostic = Some("storage-relocation-interrupted".into());
            }
            return Err(error);
        }
        locator.pending = None;
        locator.revision += 1;
        // The committed journal already points at the new valid snapshot. If clearing
        // it fails, startup will verify that snapshot and finish the same transaction.
        let _ = self.commit_locator(&locator);
        self.store = target;
        if let Some(lease) = lease {
            self.lease = Some(lease);
        }
        self.locator = locator;
        self.diagnostic = if key(directory) != key(&self.directory(&settings.storage_location)) {
            Some("storage-fallback".into())
        } else {
            None
        };
        Ok(())
    }

    fn save(&mut self, settings: &Settings, ledger: &Durable) -> Result<(), &'static str> {
        if self.lease.is_none() {
            return Err("storage-in-use");
        }
        if self.store.blocked {
            return Err("storage-future-version");
        }
        if settings.storage_location != self.locator.preferred {
            return Err("storage-location-confirm-required");
        }
        match self.store.save(settings, ledger) {
            Ok(()) => Ok(()),
            Err(error)
                if ["storage-unwritable", "storage-write", "storage-commit"].contains(&error) =>
            {
                for directory in [self.executable.clone(), self.appdata.clone()] {
                    if key(&directory) == key(&self.locator.active_directory) {
                        continue;
                    }
                    let (_, destination) = Store::open(&directory);
                    if !destination.first_launch && destination.profile_id != self.store.profile_id
                    {
                        continue;
                    }
                    if self
                        .stage_and_switch(&directory, settings, ledger, None, None)
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}

fn activate(pending: &Pending) -> Result<(), &'static str> {
    let destination = pending.directory.join("state.json");
    let staged = pending
        .directory
        .join(format!("state.{}.staged", pending.id));
    let actual_hash = digest(&destination)?;
    if actual_hash == pending.snapshot_hash {
        return Ok(());
    }
    if actual_hash != pending.original_hash || digest(&staged)? != pending.snapshot_hash {
        return Err("storage-destination-changed");
    }
    // Preserve both prior files under unique names before replacing a configuration.
    for name in ["state.json", "state.backup.json"] {
        let source = pending.directory.join(name);
        if source.is_file() {
            let archive = pending.directory.join(format!(
                "{name}.{}.{}.previous",
                crate::model::now_ms(),
                pending.id
            ));
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(archive)
                .map_err(|_| "storage-backup")?;
            let bytes = fs::read(source).map_err(|_| "storage-read")?;
            output
                .write_all(&bytes)
                .and_then(|_| output.sync_all())
                .map_err(|_| "storage-backup")?;
        }
    }
    storage::replace(
        &staged,
        &destination,
        (!actual_hash.is_empty()).then(|| pending.directory.join("state.backup.json")),
    )
}

impl Manager {
    pub fn open(executable: PathBuf, appdata: PathBuf) -> (Self, Loaded) {
        let hash = format!("{:x}", Sha256::digest(key(&executable)));
        let fallback_locator = appdata
            .join("storage-locations")
            .join(format!("{hash}.json"));
        let portable = executable.join("storage-location.json");
        let mut candidates = Vec::new();
        let mut locator_error = None;
        for (path, is_portable) in [(portable.clone(), true), (fallback_locator.clone(), false)] {
            for candidate in [path.clone(), path.with_extension("backup.json")] {
                match read_locator(&candidate) {
                    Ok(Some(locator)) => candidates.push((locator, is_portable)),
                    // A future record must win over later parse errors, including
                    // backups: an older build cannot safely rewrite its journal.
                    Err(error) if locator_error != Some("storage-future-version") => {
                        locator_error = Some(error);
                    }
                    _ => {}
                }
            }
        }
        candidates.sort_by_key(|(locator, portable)| (locator.revision, *portable));
        let selected = candidates.pop();
        let had_locator = selected.is_some();
        let (mut store, initial) = Store::open(&appdata);
        let (locator, portable_locator) = selected.unwrap_or_else(|| {
            (
                Locator {
                    format: 1,
                    revision: 0,
                    preferred: StorageLocation::Executable,
                    active_directory: appdata.clone(),
                    profile_id: initial.profile_id.clone(),
                    pending: None,
                },
                true,
            )
        });
        store.profile_id = locator.profile_id.clone();
        let mut inner = Inner {
            executable,
            appdata,
            fallback_locator,
            store,
            lease: None,
            locator,
            diagnostic: None,
            portable_locator,
        };
        if let Some(error) =
            locator_error.filter(|error| !had_locator || *error == "storage-future-version")
        {
            inner.store.blocked = true;
            inner.diagnostic = Some(error.into());
            let mut loaded = initial;
            loaded.reminders.recover(crate::model::now_ms());
            loaded.settings.reminders.enabled = false;
            loaded.settings.storage_location = inner.locator.preferred.clone();
            loaded.diagnostic = Some(error.into());
            return (Self(Mutex::new(inner)), loaded);
        }
        if !had_locator
            && initial
                .diagnostic
                .as_deref()
                .is_some_and(|code| code != "storage-recovered-backup")
            && Store::open(&inner.executable).1.first_launch
        {
            inner.store.blocked = true;
            inner.diagnostic = initial.diagnostic.clone();
            return (Self(Mutex::new(inner)), initial);
        }
        if let Some(pending) = inner.locator.pending.clone() {
            if let Ok(_lease) = Lease::acquire(&pending.directory) {
                if activate(&pending).is_ok() {
                    let mut completed = inner.locator.clone();
                    completed.pending = None;
                    completed.revision += 1;
                    let _ = inner.commit_locator(&completed);
                    inner.locator = completed;
                } else {
                    inner.diagnostic = Some("storage-relocation-interrupted".into());
                }
            }
        }
        let (_, previous_writer) = Store::open(&inner.locator.active_directory);
        let may_be_stale = had_locator
            && (previous_writer.first_launch
                || previous_writer.diagnostic.is_some()
                || previous_writer.profile_id != inner.locator.profile_id);
        // Find the latest accessible snapshot belonging to this configuration. A
        // fallback directory containing a different profile is never overwritten.
        let preferred = inner.directory(&inner.locator.preferred);
        let mut sources = vec![
            inner.locator.active_directory.clone(),
            preferred.clone(),
            inner.executable.clone(),
            inner.appdata.clone(),
        ];
        if let Some(pending) = &inner.locator.pending {
            sources.push(pending.previous_directory.clone());
        }
        let mut latest: Option<Loaded> = if had_locator {
            None
        } else {
            [
                inner.directory(&inner.locator.preferred),
                inner.appdata.clone(),
            ]
            .iter()
            .find_map(|directory| {
                let (_, loaded) = Store::open(directory);
                (!loaded.first_launch
                    && (loaded.diagnostic.is_none()
                        || loaded.diagnostic.as_deref() == Some("storage-recovered-backup")))
                .then_some(loaded)
            })
        };
        if let Some(source) = &latest {
            inner.locator.profile_id = source.profile_id.clone();
        }
        for directory in &sources {
            let (_, loaded) = Store::open(directory);
            if !loaded.first_launch
                && loaded.diagnostic.is_none()
                && loaded.profile_id == inner.locator.profile_id
                && latest
                    .as_ref()
                    .is_none_or(|s| loaded.data_revision > s.data_revision)
            {
                latest = Some(loaded);
            }
        }
        let mut output = None;
        let mut tried = Vec::new();
        for directory in [preferred, inner.executable.clone(), inner.appdata.clone()] {
            if tried.contains(&key(&directory)) {
                continue;
            }
            tried.push(key(&directory));
            if writable(&directory).is_err() {
                continue;
            }
            let lease = match Lease::acquire(&directory) {
                Ok(lease) => lease,
                Err(_) => {
                    let (mut target, mut loaded) = Store::open(&directory);
                    target.blocked = true;
                    loaded.reminders.recover(crate::model::now_ms());
                    loaded.settings.reminders.enabled = false;
                    loaded.diagnostic = Some("storage-in-use".into());
                    inner.store = target;
                    inner.locator.active_directory = directory;
                    output = Some(loaded);
                    break;
                }
            };
            let (mut target, mut loaded) = Store::open(&directory);
            if target.blocked || loaded.diagnostic.is_some() {
                if loaded.diagnostic.as_deref() == Some("storage-unrecoverable") {
                    target.blocked = true;
                }
                inner.store = target;
                inner.lease = Some(lease);
                inner.locator.active_directory = directory;
                output = Some(loaded);
                break;
            }
            if had_locator && !loaded.first_launch && loaded.profile_id != inner.locator.profile_id
            {
                continue;
            }
            let restoring = latest
                .as_ref()
                .is_some_and(|s| loaded.first_launch || s.data_revision > loaded.data_revision);
            if restoring {
                drop(lease);
                let mut source = latest.take().expect("snapshot checked");
                source.settings.storage_location = inner.locator.preferred.clone();
                if may_be_stale {
                    source.reminders.recover(crate::model::now_ms());
                    source.settings.reminders.enabled = false;
                    source.diagnostic = Some("storage-stale-fallback".into());
                }
                inner.store.profile_id = source.profile_id.clone();
                inner.store.data_revision.set(source.data_revision);
                if inner
                    .stage_and_switch(
                        &directory,
                        &source.settings,
                        &source.reminders,
                        Some(&source),
                        None,
                    )
                    .is_ok()
                {
                    source.first_launch = false;
                    output = Some(source);
                    break;
                }
                latest = Some(source);
                continue;
            }
            let is_fallback = key(&directory) != key(&inner.directory(&inner.locator.preferred));
            if had_locator
                && (may_be_stale
                    || loaded.profile_id != inner.locator.profile_id
                    || (latest.is_none()
                        && key(&directory) != key(&inner.locator.active_directory)))
            {
                loaded.reminders.recover(crate::model::now_ms());
                loaded.settings.reminders.enabled = false;
                loaded.diagnostic = Some("storage-stale-fallback".into());
            }
            loaded.settings.storage_location = inner.locator.preferred.clone();
            inner.locator.active_directory = directory;
            inner.locator.profile_id = target.profile_id.clone();
            inner.store = target;
            inner.lease = Some(lease);
            if is_fallback {
                inner.diagnostic = Some("storage-fallback".into());
            }
            let mut locator = inner.locator.clone();
            locator.revision += 1;
            if inner.commit_locator(&locator).is_err() {
                inner.diagnostic = Some("storage-locator-unwritable".into());
                inner.store.blocked = true;
            }
            inner.locator = locator;
            output = Some(loaded);
            break;
        }
        let mut loaded = output.unwrap_or_else(|| {
            inner.store.blocked = true;
            let mut fallback = initial;
            fallback.reminders.recover(crate::model::now_ms());
            fallback.settings.reminders.enabled = false;
            fallback.diagnostic = Some("storage-unwritable".into());
            fallback
        });
        if let Some(error) = locator_error {
            if error == "storage-future-version" || !had_locator {
                inner.store.blocked = true;
                loaded.diagnostic = Some(error.into());
            } else {
                loaded.reminders.recover(crate::model::now_ms());
                loaded.settings.reminders.enabled = false;
                loaded.diagnostic = Some("storage-location-recovered".into());
            }
        }
        loaded.settings.storage_location = inner.locator.preferred.clone();
        (Self(Mutex::new(inner)), loaded)
    }

    pub fn status(&self) -> Status {
        let inner = self.0.lock().expect("storage manager poisoned");
        Status {
            preferred: inner.locator.preferred.clone(),
            active_directory: inner
                .locator
                .active_directory
                .to_string_lossy()
                .into_owned(),
            fallback: key(&inner.locator.active_directory)
                != key(&inner.directory(&inner.locator.preferred)),
            diagnostic: inner.diagnostic.clone(),
            portable_locator: inner.portable_locator,
        }
    }

    pub fn inspect(&self, selection: &StorageLocation) -> Result<Inspection, &'static str> {
        self.0
            .lock()
            .map_err(|_| "storage-unavailable")?
            .inspection(selection)
    }

    pub fn save(&self, settings: &Settings, ledger: &Durable) -> Result<(), &'static str> {
        self.0
            .lock()
            .map_err(|_| "storage-unavailable")?
            .save(settings, ledger)
    }

    pub fn save_selected(
        &self,
        settings: &Settings,
        ledger: &Durable,
        resolution: Option<Resolution>,
        token: Option<&str>,
    ) -> Result<Option<Loaded>, &'static str> {
        let mut inner = self.0.lock().map_err(|_| "storage-unavailable")?;
        if inner.store.blocked {
            return Err("storage-future-version");
        }
        if inner.lease.is_none() {
            return Err("storage-in-use");
        }
        if settings.storage_location == inner.locator.preferred {
            inner.save(settings, ledger)?;
            return Ok(None);
        }
        let inspection = inner.inspection(&settings.storage_location)?;
        if Some(inspection.token.as_str()) != token {
            return Err("storage-destination-changed");
        }
        if inspection.conflict && resolution.is_none() {
            return Err("storage-destination-conflict");
        }
        let directory = inner.directory(&settings.storage_location);
        if matches!(resolution, Some(Resolution::UseDestination)) && inspection.conflict {
            let (_, mut loaded) = Store::open(&directory);
            loaded.settings.storage_location = settings.storage_location.clone();
            loaded.settings.revision = settings.revision;
            inner.stage_and_switch(
                &directory,
                &loaded.settings,
                &loaded.reminders,
                Some(&loaded),
                token,
            )?;
            return Ok(Some(loaded));
        }
        inner.stage_and_switch(&directory, settings, ledger, None, token)?;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        exe: PathBuf,
        appdata: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("widget-locations-{}", storage::new_profile_id()));
            let exe = root.join("exe");
            let appdata = root.join("appdata");
            fs::create_dir_all(&exe).unwrap();
            fs::create_dir_all(&appdata).unwrap();
            Self { root, exe, appdata }
        }
        fn open(&self) -> (Manager, Loaded) {
            Manager::open(self.exe.clone(), self.appdata.clone())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn fresh_default_is_portable_and_remembers_custom_selection() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        assert!(loaded.first_launch);
        assert_eq!(
            manager.status().active_directory,
            fixture.exe.to_string_lossy()
        );
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let custom = fixture.root.join("custom");
        let mut settings = loaded.settings;
        settings.storage_location = StorageLocation::Custom {
            directory: custom.to_string_lossy().into_owned(),
        };
        let inspection = manager.inspect(&settings.storage_location).unwrap();
        assert!(!inspection.conflict);
        manager
            .save_selected(&settings, &loaded.reminders, None, Some(&inspection.token))
            .unwrap();
        assert!(custom.join("state.json").is_file());
        assert!(fixture.exe.join("state.json").is_file());
        drop(manager);
        let (manager, loaded) = fixture.open();
        assert_eq!(manager.status().active_directory, custom.to_string_lossy());
        assert_eq!(loaded.settings.storage_location, settings.storage_location);
    }

    #[test]
    fn legacy_source_corruption_and_future_versions_are_not_silently_forked() {
        for content in [
            b"unrelated data".as_slice(),
            br#"{"format":99,"checksum":"opaque","settings":{}}"#.as_slice(),
        ] {
            let fixture = Fixture::new();
            fs::write(fixture.appdata.join("state.json"), content).unwrap();
            let (manager, loaded) = fixture.open();
            assert!(loaded.diagnostic.is_some());
            assert!(!loaded.first_launch);
            assert!(manager.save(&loaded.settings, &loaded.reminders).is_err());
            assert!(!fixture.exe.join("state.json").exists());
            assert_eq!(
                fs::read(fixture.appdata.join("state.json")).unwrap(),
                content
            );
        }
    }

    #[test]
    fn future_locator_backup_prevents_startup_and_relocation_writes() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        drop(manager);
        let primary = fixture.exe.join("storage-location.json");
        let original = fs::read(&primary).unwrap();
        let state = fs::read(fixture.exe.join("state.json")).unwrap();
        let future = br#"{"format":99}"#;
        fs::write(primary.with_extension("backup.json"), future).unwrap();
        // A later invalid fallback must not hide the future-version diagnostic.
        let hash = format!("{:x}", Sha256::digest(key(&fixture.exe)));
        let fallback = fixture.appdata.join("storage-locations");
        fs::create_dir_all(&fallback).unwrap();
        fs::write(fallback.join(format!("{hash}.json")), b"invalid").unwrap();
        let (manager, mut loaded) = fixture.open();
        assert_eq!(loaded.diagnostic.as_deref(), Some("storage-future-version"));
        assert_eq!(fs::read(&primary).unwrap(), original);
        assert_eq!(fs::read(fixture.exe.join("state.json")).unwrap(), state);
        loaded.settings.storage_location = StorageLocation::Custom {
            directory: fixture.root.join("other").to_string_lossy().into_owned(),
        };
        let inspection = manager.inspect(&loaded.settings.storage_location).unwrap();
        assert!(matches!(
            manager.save_selected(
                &loaded.settings,
                &loaded.reminders,
                None,
                Some(&inspection.token)
            ),
            Err("storage-future-version")
        ));
        assert!(!fixture.root.join("other").exists());
        assert_eq!(
            fs::read(primary.with_extension("backup.json")).unwrap(),
            future
        );
    }

    #[test]
    fn corrupt_locator_recovery_preserves_bad_bytes_and_pauses_reminders() {
        let fixture = Fixture::new();
        let (manager, mut loaded) = fixture.open();
        loaded.settings.reminders.enabled = true;
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        drop(manager);
        // A second startup creates a backup location record.
        let (manager, _) = fixture.open();
        drop(manager);
        fs::write(
            fixture.exe.join("storage-location.json"),
            b"corrupt location",
        )
        .unwrap();
        let (_manager, loaded) = fixture.open();
        assert!(!loaded.settings.reminders.enabled);
        assert!(loaded.reminders.delivery_paused);
        assert_eq!(
            loaded.diagnostic.as_deref(),
            Some("storage-location-recovered")
        );
        assert!(fs::read_dir(&fixture.exe).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("location.")));
    }

    #[test]
    fn long_unicode_paths_support_atomic_primary_and_backup_replacement() {
        let fixture = Fixture::new();
        let directory = fixture
            .root
            .join("nested".repeat(20))
            .join("配置".repeat(16));
        let (store, mut loaded) = Store::open(&directory);
        store.save(&loaded.settings, &loaded.reminders).unwrap();
        loaded.settings.revision += 1;
        store.save(&loaded.settings, &loaded.reminders).unwrap();
        assert_eq!(Store::open(&directory).1.settings.revision, 1);
        assert!(directory.join("state.backup.json").is_file());
    }

    #[test]
    fn orphan_staging_and_invalid_locators_cannot_replace_active_data() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let orphan = fixture
            .exe
            .join(format!("state.{}.staged", storage::new_profile_id()));
        fs::write(&orphan, b"uncommitted fixture").unwrap();
        drop(manager);
        let (manager, restored) = fixture.open();
        assert_eq!(
            restored.settings.theme_preference,
            loaded.settings.theme_preference
        );
        assert!(orphan.is_file());
        drop(manager);
        fs::remove_file(fixture.exe.join("storage-location.backup.json")).unwrap();
        let invalid = b"not a location record";
        fs::write(fixture.exe.join("storage-location.json"), invalid).unwrap();
        let (manager, restored) = fixture.open();
        assert!(restored.diagnostic.is_some());
        assert!(manager
            .save(&restored.settings, &restored.reminders)
            .is_err());
        assert_eq!(
            fs::read(fixture.exe.join("storage-location.json")).unwrap(),
            invalid
        );
    }

    #[test]
    fn missing_last_writer_pauses_reminders_on_an_older_recovery_copy() {
        let fixture = Fixture::new();
        let (manager, mut loaded) = fixture.open();
        loaded.settings.reminders.enabled = true;
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let custom = fixture.root.join("custom");
        let mut settings = loaded.settings;
        settings.storage_location = StorageLocation::Custom {
            directory: custom.to_string_lossy().into_owned(),
        };
        let token = manager.inspect(&settings.storage_location).unwrap().token;
        manager
            .save_selected(&settings, &loaded.reminders, None, Some(&token))
            .unwrap();
        drop(manager);
        // A disconnected volume is represented by a non-directory path. The EXE copy is older.
        fs::rename(&custom, fixture.root.join("disconnected")).unwrap();
        fs::write(&custom, b"unavailable volume").unwrap();
        let (manager, restored) = fixture.open();
        assert!(manager.status().fallback);
        assert!(!restored.settings.reminders.enabled);
        assert!(restored.reminders.delivery_paused);
        assert_eq!(
            restored.diagnostic.as_deref(),
            Some("storage-stale-fallback")
        );
    }

    #[test]
    fn migration_carries_settings_and_claims_without_deleting_appdata() {
        let fixture = Fixture::new();
        let (store, mut original) = Store::open(&fixture.appdata);
        original.settings.theme_preference = crate::model::ThemePreference::Dark;
        let mut engine = crate::reminders::Engine::new(original.reminders, false);
        engine.test_claim(1000, 1).unwrap();
        store.save(&original.settings, &engine.data).unwrap();
        let original_bytes = fs::read(fixture.appdata.join("state.json")).unwrap();
        let (manager, loaded) = fixture.open();
        assert_eq!(
            loaded.settings.theme_preference,
            crate::model::ThemePreference::Dark
        );
        assert_eq!(loaded.reminders.history.len(), engine.data.history.len());
        assert!(!loaded.first_launch);
        assert!(fixture.exe.join("state.json").is_file());
        assert_eq!(
            fs::read(fixture.appdata.join("state.json")).unwrap(),
            original_bytes
        );
        assert!(manager.status().portable_locator);
    }

    #[test]
    fn existing_portable_data_wins_over_unrelated_appdata() {
        let fixture = Fixture::new();
        let (exe, mut portable) = Store::open(&fixture.exe);
        portable.settings.theme_preference = crate::model::ThemePreference::Dark;
        exe.save(&portable.settings, &portable.reminders).unwrap();
        let (appdata, legacy) = Store::open(&fixture.appdata);
        for _ in 0..3 {
            appdata.save(&legacy.settings, &legacy.reminders).unwrap();
        }
        let (_manager, loaded) = fixture.open();
        assert_eq!(
            loaded.settings.theme_preference,
            crate::model::ThemePreference::Dark
        );
    }

    #[test]
    fn different_destinations_require_choice_and_a_current_token() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let custom = fixture.root.join("custom");
        let (target, mut other) = Store::open(&custom);
        other.settings.theme_preference = crate::model::ThemePreference::Dark;
        target.save(&other.settings, &other.reminders).unwrap();
        let before = fs::read(custom.join("state.json")).unwrap();
        let mut settings = loaded.settings;
        settings.storage_location = StorageLocation::Custom {
            directory: custom.to_string_lossy().into_owned(),
        };
        let inspection = manager.inspect(&settings.storage_location).unwrap();
        assert!(inspection.conflict);
        assert!(matches!(
            manager.save_selected(&settings, &loaded.reminders, None, Some(&inspection.token)),
            Err("storage-destination-conflict")
        ));
        target.save(&other.settings, &other.reminders).unwrap();
        assert!(matches!(
            manager.save_selected(
                &settings,
                &loaded.reminders,
                Some(Resolution::CopyCurrent),
                Some(&inspection.token)
            ),
            Err("storage-destination-changed")
        ));
        let inspection = manager.inspect(&settings.storage_location).unwrap();
        let selected = manager
            .save_selected(
                &settings,
                &loaded.reminders,
                Some(Resolution::UseDestination),
                Some(&inspection.token),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            selected.settings.theme_preference,
            crate::model::ThemePreference::Dark
        );
        assert!(fs::read_dir(&custom).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".previous")));
        assert!(!before.is_empty());
    }

    #[test]
    fn runtime_fallback_carries_latest_state_then_returns_to_preferred() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let custom = fixture.root.join("custom");
        let mut settings = loaded.settings;
        settings.storage_location = StorageLocation::Custom {
            directory: custom.to_string_lossy().into_owned(),
        };
        let token = manager.inspect(&settings.storage_location).unwrap().token;
        manager
            .save_selected(&settings, &loaded.reminders, None, Some(&token))
            .unwrap();
        // Make only the active commit destination unusable, without deleting any data.
        fs::rename(custom.join("state.json"), custom.join("saved.json")).unwrap();
        fs::create_dir(custom.join("state.json")).unwrap();
        settings.theme_preference = crate::model::ThemePreference::Dark;
        manager.save(&settings, &loaded.reminders).unwrap();
        assert!(manager.status().fallback);
        assert_eq!(
            manager.status().active_directory,
            fixture.exe.to_string_lossy()
        );
        fs::remove_dir(custom.join("state.json")).unwrap();
        fs::rename(custom.join("saved.json"), custom.join("state.json")).unwrap();
        drop(manager);
        let (manager, loaded) = fixture.open();
        assert_eq!(manager.status().active_directory, custom.to_string_lossy());
        assert_eq!(
            loaded.settings.theme_preference,
            crate::model::ThemePreference::Dark
        );
    }

    #[test]
    fn unavailable_exe_uses_appdata_locator_and_custom_then_fallback_order() {
        let fixture = Fixture::new();
        let blocked = fixture.root.join("not-a-folder");
        fs::write(&blocked, b"unchanged").unwrap();
        let (manager, loaded) = Manager::open(blocked.clone(), fixture.appdata.clone());
        assert!(manager.status().fallback);
        assert!(!manager.status().portable_locator);
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        assert_eq!(fs::read(blocked).unwrap(), b"unchanged");
        assert_eq!(
            manager.status().active_directory,
            fixture.appdata.to_string_lossy()
        );
    }

    #[test]
    fn a_second_writer_and_unrelated_files_are_not_overwritten() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let (second, blocked) = fixture.open();
        assert!(second.status().fallback || blocked.diagnostic.is_some());
        drop(second);
        let custom = fixture.root.join("unrelated");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("state.json"), b"not widget data").unwrap();
        let location = StorageLocation::Custom {
            directory: custom.to_string_lossy().into_owned(),
        };
        assert!(manager.inspect(&location).is_err());
        assert_eq!(
            fs::read(custom.join("state.json")).unwrap(),
            b"not widget data"
        );
    }

    #[test]
    fn committed_journal_rolls_forward_and_uncommitted_staging_is_ignored() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let custom = fixture.root.join("custom");
        fs::create_dir_all(&custom).unwrap();
        let (mut staged, _) = Store::open(&custom);
        let id = storage::new_profile_id();
        staged.path = custom.join(format!("state.{id}.staged"));
        staged.profile_id = loaded.profile_id.clone();
        let mut settings = loaded.settings.clone();
        settings.theme_preference = crate::model::ThemePreference::Dark;
        settings.storage_location = StorageLocation::Custom {
            directory: custom.to_string_lossy().into_owned(),
        };
        staged.save(&settings, &loaded.reminders).unwrap();
        let mut locator = manager.0.lock().unwrap().locator.clone();
        locator.revision += 1;
        locator.preferred = settings.storage_location.clone();
        locator.active_directory = custom.clone();
        locator.pending = Some(Pending {
            id,
            directory: custom.clone(),
            previous_directory: fixture.exe.clone(),
            original_hash: String::new(),
            snapshot_hash: digest(&staged.path).unwrap(),
        });
        atomic_locator(&fixture.exe.join("storage-location.json"), &locator).unwrap();
        drop(manager);
        let (manager, loaded) = fixture.open();
        assert_eq!(
            loaded.settings.theme_preference,
            crate::model::ThemePreference::Dark
        );
        assert_eq!(manager.status().active_directory, custom.to_string_lossy());
        assert!(manager.0.lock().unwrap().locator.pending.is_none());
    }

    #[test]
    fn newest_fallback_manifest_wins_and_future_data_stays_untouched() {
        let fixture = Fixture::new();
        let (manager, loaded) = fixture.open();
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        let mut locator = manager.0.lock().unwrap().locator.clone();
        locator.revision += 10;
        locator.preferred = StorageLocation::AppData;
        let fallback = manager.0.lock().unwrap().fallback_locator.clone();
        atomic_locator(&fallback, &locator).unwrap();
        drop(manager);
        let (manager, loaded) = fixture.open();
        assert_eq!(manager.status().preferred, StorageLocation::AppData);
        manager.save(&loaded.settings, &loaded.reminders).unwrap();
        drop(manager);
        let future = br#"{"format":99,"checksum":"opaque","settings":{}}"#;
        fs::write(fixture.appdata.join("state.json"), future).unwrap();
        let (manager, loaded) = fixture.open();
        assert_eq!(loaded.diagnostic.as_deref(), Some("storage-future-version"));
        assert!(manager.save(&loaded.settings, &loaded.reminders).is_err());
        assert_eq!(
            fs::read(fixture.appdata.join("state.json")).unwrap(),
            future
        );
    }
}

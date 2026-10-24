use crate::{AutoLaunch, Result, WindowsEnableMode};
use std::io;
use windows_registry::{Key, CURRENT_USER, LOCAL_MACHINE};
use windows_result::HRESULT;

const AL_REGKEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const TASK_MANAGER_OVERRIDE_REGKEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const TASK_MANAGER_OVERRIDE_ENABLED_VALUE: [u8; 12] = [
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const E_ACCESSDENIED: HRESULT = HRESULT::from_win32(0x80070005_u32);
const E_FILENOTFOUND: HRESULT = HRESULT::from_win32(0x80070002_u32);

/// Windows implement
impl AutoLaunch {
    /// Create a new AutoLaunch instance
    /// - `app_name`: application name
    /// - `app_path`: application path
    /// - `enable_mode`: behavior of the enable feature
    /// - `args`: startup args passed to the binary
    ///
    /// ## Notes
    ///
    /// The parameters of `AutoLaunch::new` are different on each platform.
    pub fn new(
        app_name: &str,
        app_path: &str,
        enable_mode: WindowsEnableMode,
        args: &[impl AsRef<str>],
    ) -> AutoLaunch {
        AutoLaunch {
            app_name: app_name.into(),
            app_path: app_path.into(),
            enable_mode,
            args: args.iter().map(|s| s.as_ref().to_string()).collect(),
        }
    }

    /// Enable the AutoLaunch setting
    ///
    /// ## Errors
    ///
    /// - failed to open the registry key
    /// - failed to set value
    pub fn enable(&self) -> Result<()> {
        match self.enable_mode {
            WindowsEnableMode::Dynamic => self
                .enable_as_admin()
                .or_else(|e| {
                    if e.code() == E_ACCESSDENIED {
                        self.enable_as_current_user()
                    } else {
                        Err(e)
                    }
                })
                .map_err(std::io::Error::from)?,
            WindowsEnableMode::CurrentUser => self
                .enable_as_current_user()
                .map_err(std::io::Error::from)?,
            WindowsEnableMode::System => self.enable_as_admin().map_err(std::io::Error::from)?,
        }
        Ok(())
    }

    fn enable_as_admin(&self) -> windows_registry::Result<()> {
        self.enable_with_root_key(LOCAL_MACHINE)
    }

    fn enable_as_current_user(&self) -> windows_registry::Result<()> {
        self.enable_with_root_key(CURRENT_USER)
    }

    fn enable_with_root_key(&self, root_key: &Key) -> windows_registry::Result<()> {
        root_key
            .create(AL_REGKEY)?
            .set_string(&self.app_name, self.command_line())?;

        match root_key
            .options()
            .write()
            .open(TASK_MANAGER_OVERRIDE_REGKEY)
        {
            Ok(key) => key.set_bytes(
                &self.app_name,
                windows_registry::Type::Bytes,
                &TASK_MANAGER_OVERRIDE_ENABLED_VALUE,
            )?,
            Err(error) if error.code() == E_FILENOTFOUND => {
                return Ok(());
            }
            Err(error) => {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Disable the AutoLaunch setting
    ///
    /// ## Errors
    ///
    /// - failed to open the registry key
    /// - failed to delete value
    pub fn disable(&self) -> Result<()> {
        if self.enable_mode == WindowsEnableMode::CurrentUser {
            return self
                .disable_as_current_user()
                .map_err(|e| std::io::Error::from(e).into());
        }
        // try to delete both admin and current user registry values
        if let Err(error) = self.disable_as_admin() {
            if error.code() == E_ACCESSDENIED {
                match self.enable_mode {
                    // Fail if our enable mode is system but we don't have the access
                    WindowsEnableMode::System => return Err(std::io::Error::from(error).into()),
                    // Otherwise ignore this error
                    _ => {}
                }
            } else {
                return Err(std::io::Error::from(error).into());
            }
        }
        self.disable_as_current_user()
            .map_err(std::io::Error::from)?;
        Ok(())
    }

    fn disable_as_admin(&self) -> windows_registry::Result<()> {
        self.disable_with_root_key(LOCAL_MACHINE)
    }

    fn disable_as_current_user(&self) -> windows_registry::Result<()> {
        self.disable_with_root_key(CURRENT_USER)
    }

    fn disable_with_root_key(&self, root_key: &Key) -> windows_registry::Result<()> {
        match root_key
            .options()
            .write()
            .open(AL_REGKEY)
            .and_then(|key| key.remove_value(&self.app_name))
        {
            Ok(_) => Ok(()),
            Err(error) if error.code() == E_FILENOTFOUND => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Check whether the AutoLaunch setting is enabled
    pub fn is_enabled(&self) -> Result<bool> {
        if self.enable_mode == WindowsEnableMode::CurrentUser {
            return Ok(
                self.is_registered(CURRENT_USER)? && self.is_task_manager_enabled(CURRENT_USER)?
            );
        }
        let is_registered =
            self.is_registered(LOCAL_MACHINE)? || self.is_registered(CURRENT_USER)?;
        if !is_registered {
            return Ok(false);
        }
        let is_task_manager_enabled = self.is_task_manager_enabled(LOCAL_MACHINE)?
            && self.is_task_manager_enabled(CURRENT_USER)?;
        Ok(is_task_manager_enabled)
    }

    fn is_registered(&self, root_key: &Key) -> io::Result<bool> {
        let registered = match root_key
            .open(AL_REGKEY)
            .and_then(|key| key.get_string(&self.app_name))
        {
            Ok(command) => command == self.command_line(),
            Err(error) if error.code() == E_FILENOTFOUND => false,
            Err(error) => {
                return Err(error.into());
            }
        };
        Ok(registered)
    }

    fn is_task_manager_enabled(&self, root_key: &Key) -> io::Result<bool> {
        let task_manager_enabled = match root_key
            .open(TASK_MANAGER_OVERRIDE_REGKEY)
            .and_then(|key| key.get_value(&self.app_name))
        {
            Ok(value) => startup_approved(&value)?,
            Err(error) if error.code() == E_FILENOTFOUND => true,
            Err(error) => {
                return Err(error.into());
            }
        };
        Ok(task_manager_enabled)
    }

    fn command_line(&self) -> String {
        std::iter::once(&self.app_path)
            .chain(&self.args)
            .map(|arg| quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

// Windows command-line quoting, including trailing backslashes and embedded quotes.
fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            '\\',
            if ch == '"' { slashes * 2 + 1 } else { slashes },
        ));
        slashes = 0;
        out.push(ch);
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

// Unknown/malformed StartupApproved data is unknown, never guessed enabled.
fn startup_approved(bytes: &[u8]) -> io::Result<bool> {
    if bytes.len() != 12 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unknown startup approval record",
        ));
    }
    match u32::from_le_bytes(bytes[..4].try_into().unwrap()) {
        2 | 6 => Ok(true),
        3 | 7 => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unknown startup approval state",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_command_quotes_paths_arguments_and_trailing_slashes() {
        assert_eq!(
            quote(r"C:\Program Files\Widget\widget.exe"),
            r#""C:\Program Files\Widget\widget.exe""#
        );
        assert_eq!(quote("--autostart"), "\"--autostart\"");
        assert_eq!(quote("a\\\"b\\"), "\"a\\\\\\\"b\\\\\"");
        assert_eq!(quote(""), "\"\"");
    }

    #[test]
    fn startup_disabled_with_zero_timestamp_is_still_disabled() {
        let mut data = [0; 12];
        for state in [2, 6] {
            data[0] = state;
            assert!(startup_approved(&data).unwrap());
        }
        for state in [3, 7] {
            data[0] = state;
            assert!(!startup_approved(&data).unwrap());
        }
        data[0] = 9;
        assert!(startup_approved(&data).is_err());
        assert!(startup_approved(&[2; 8]).is_err());
    }

    #[test]
    fn widget_default_is_per_user_and_registration_matches_exact_command() {
        assert_eq!(WindowsEnableMode::default(), WindowsEnableMode::CurrentUser);
        let app = AutoLaunch::new(
            "widget",
            r"C:\Program Files\Widget\widget.exe",
            WindowsEnableMode::default(),
            &["--autostart"],
        );
        assert_eq!(
            app.command_line(),
            r#""C:\Program Files\Widget\widget.exe" "--autostart""#
        );
    }
}

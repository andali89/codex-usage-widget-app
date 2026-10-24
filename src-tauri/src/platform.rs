use crate::model::{Appearance, ThemePreference};

pub fn account_salt() -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        let mut bytes = [0_u8; 32];
        if unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        } < 0
        {
            return None;
        }
        Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Uses Windows' date-specific timezone conversion, including historical DST rules.
pub fn local_minute(at_ms: i64) -> Option<crate::reminders::LocalMinute> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{FILETIME, SYSTEMTIME},
            System::Time::{
                FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTimeEx,
            },
        };
        let ticks = at_ms.checked_add(11_644_473_600_000)?.checked_mul(10_000)? as u64;
        let ft = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
        let mut local = utc;
        let mut local_ft = ft;
        if unsafe { FileTimeToSystemTime(&ft, &mut utc) } == 0
            || unsafe { SystemTimeToTzSpecificLocalTimeEx(std::ptr::null(), &utc, &mut local) } == 0
            || unsafe { SystemTimeToFileTime(&local, &mut local_ft) } == 0
        {
            return None;
        }
        let local_ticks =
            (u64::from(local_ft.dwHighDateTime) << 32) | u64::from(local_ft.dwLowDateTime);
        let local_ms = i64::try_from(local_ticks / 10_000)
            .ok()?
            .checked_sub(11_644_473_600_000)?;
        Some(crate::reminders::LocalMinute {
            day: local_ms.div_euclid(86_400_000),
            minute: local.wHour * 60 + local.wMinute,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = at_ms;
        None
    }
}

pub fn tomorrow_nine(now: i64) -> Option<i64> {
    let target = local_minute(now)?.day + 1;
    let start = (now.div_euclid(60_000) + 1) * 60_000;
    // First valid minute at/after 09:00 also handles a timezone's unusual gap/fold.
    (start..start + 49 * 3_600_000)
        .step_by(60_000)
        .find(|at| local_minute(*at).is_some_and(|t| t.day == target && t.minute >= 540))
}

/// Only compares OS timezone configuration. Kept in memory, never logged or persisted.
pub fn timezone_signature() -> Option<Vec<i64>> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Time::{
            GetDynamicTimeZoneInformation, DYNAMIC_TIME_ZONE_INFORMATION,
        };
        let mut zone: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        let state = unsafe { GetDynamicTimeZoneInformation(&mut zone) };
        if state == u32::MAX {
            return None;
        }
        let mut values = vec![
            i64::from(state),
            i64::from(zone.Bias),
            i64::from(zone.StandardBias),
            i64::from(zone.DaylightBias),
            i64::from(zone.DynamicDaylightTimeDisabled),
        ];
        values.extend(zone.TimeZoneKeyName.iter().map(|c| i64::from(*c)));
        for d in [zone.StandardDate, zone.DaylightDate] {
            values.extend(
                [
                    d.wYear,
                    d.wMonth,
                    d.wDay,
                    d.wDayOfWeek,
                    d.wHour,
                    d.wMinute,
                    d.wSecond,
                    d.wMilliseconds,
                ]
                .map(i64::from),
            );
        }
        Some(values)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(all(test, windows))]
mod calendar_tests {
    use super::*;
    #[test]
    fn windows_calendar_conversion_and_tomorrow_snooze_are_consistent() {
        let now = crate::model::now_ms();
        let local = local_minute(now).unwrap();
        let tomorrow = tomorrow_nine(now).unwrap();
        let target = local_minute(tomorrow).unwrap();
        assert_eq!(target.day, local.day + 1);
        assert!(target.minute >= 540);
        assert!(tomorrow > now && tomorrow - now <= 49 * 3_600_000);
        let a = account_salt().unwrap();
        let b = account_salt().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        let signature = timezone_signature().unwrap();
        assert!(signature.len() > 128);
        assert_eq!(timezone_signature().as_ref(), Some(&signature));
    }
}

#[cfg(windows)]
fn registry_dword(subkey: &str, name: &str) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let key = subkey.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut value: u32 = 0;
    let mut size = 4;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    (status == 0).then_some(value)
}

pub fn appearance(preference: ThemePreference) -> Appearance {
    let mut state = Appearance {
        theme_preference: preference,
        resolved_theme: "light".into(),
        transparency_allowed: false,
        text_scale: 100,
        ..Appearance::default()
    };
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{
            Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
            WindowsAndMessaging::{
                SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST,
            },
        };
        let mut contrast: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
        contrast.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        let mut animate: i32 = 0;
        unsafe {
            if SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                (&mut contrast as *mut HIGHCONTRASTW).cast(),
                0,
            ) != 0
            {
                state.high_contrast = contrast.dwFlags & HCF_HIGHCONTRASTON != 0;
            }
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut animate as *mut i32).cast(),
                0,
            );
        }
        state.reduced_motion = animate == 0;
        let personal = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
        let dark = registry_dword(personal, "AppsUseLightTheme") == Some(0);
        state.resolved_theme = if dark { "dark" } else { "light" }.into();
        state.transparency_allowed =
            !state.high_contrast && registry_dword(personal, "EnableTransparency") == Some(1);
        state.text_scale = registry_dword("Software\\Microsoft\\Accessibility", "TextScaleFactor")
            .unwrap_or(100)
            .clamp(100, 225);
    }
    match preference {
        ThemePreference::Light => state.resolved_theme = "light".into(),
        ThemePreference::Dark => state.resolved_theme = "dark".into(),
        ThemePreference::System => {}
    }
    state
}

#[cfg(windows)]
pub struct OwnedJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for OwnedJob {}

#[cfg(windows)]
impl OwnedJob {
    pub fn attach(child: &tokio::process::Child) -> Result<Self, &'static str> {
        use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err("process-ownership");
        }
        let Some(child_handle) = child.raw_handle() else {
            unsafe {
                CloseHandle(handle);
            }
            return Err("process-ownership");
        };
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let valid = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            ) != 0
                && AssignProcessToJobObject(handle, child_handle as _) != 0
        };
        if !valid {
            unsafe {
                CloseHandle(handle);
            }
            return Err("process-ownership");
        }
        Ok(Self(handle))
    }
}

#[cfg(windows)]
impl Drop for OwnedJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

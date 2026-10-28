//! Ephemeral display-only drafts. Never persisted or passed to the quota scheduler.
use crate::{
    model::{Settings, ThemePreference},
    widget_appearance::WidgetAppearance,
};
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    pub theme_preference: ThemePreference,
    pub widget_appearance: WidgetAppearance,
    pub snap_gap_logical_px: u8,
    pub freeze_popup: bool,
}
impl Patch {
    pub fn validate(&self) -> Result<(), String> {
        if self.snap_gap_logical_px > 48 {
            return Err("invalid-snap-gap".into());
        }
        let issues = self.widget_appearance.issues();
        if !issues.is_empty() {
            return Err(
                serde_json::json!({"code":"invalid-widget-settings","issues":issues}).to_string(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Session {
    pub owner: String,
    pub base_revision: u64,
    pub sequence: u64,
    pub expires: u64,
    pub patch: Option<Patch>,
}
impl Session {
    pub fn begin(owner: String, revision: u64, now: u64) -> Result<Self, String> {
        if owner.len() != 36 || !owner.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-') {
            return Err("invalid-preview-session".into());
        }
        Ok(Self {
            owner,
            base_revision: revision,
            sequence: 0,
            expires: now + 10000,
            patch: None,
        })
    }
    pub fn update(
        &mut self,
        owner: &str,
        sequence: u64,
        mut patch: Option<Patch>,
        revision: u64,
        now: u64,
    ) -> Result<(), String> {
        if self.owner != owner || self.base_revision != revision || now >= self.expires {
            return Err("display-preview-ended".into());
        }
        if sequence <= self.sequence {
            return Err("display-preview-outdated".into());
        }
        if let Some(p) = &mut patch {
            p.widget_appearance.normalize();
            p.validate()?;
        }
        self.sequence = sequence;
        self.expires = now + 10000;
        if patch.is_some() {
            self.patch = patch;
        }
        Ok(())
    }
    pub fn effective(&self, saved: &Settings) -> Settings {
        let mut settings = saved.clone();
        if let Some(p) = &self.patch {
            settings.theme_preference = p.theme_preference;
            settings.widget_appearance = p.widget_appearance.clone();
            settings.windows.snap_gap_logical_px = p.snap_gap_logical_px;
        }
        settings
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = "00000000-0000-0000-0000-000000000001";
    fn patch() -> Patch {
        Patch {
            theme_preference: ThemePreference::Dark,
            widget_appearance: WidgetAppearance {
                compact_countdown_bold: true,
                compact_countdown_high_contrast: true,
                edge_scale_percent: 80,
                hud_scale_percent: 137,
                show_border: false,
                background_transparency_percent: 80,
                ..Default::default()
            },
            snap_gap_logical_px: 24,
            freeze_popup: true,
        }
    }
    #[test]
    fn ownership_sequence_and_invalid_input_keep_last_valid_preview() {
        let mut s = Session::begin(OWNER.into(), 3, 0).unwrap();
        s.update(OWNER, 1, Some(patch()), 3, 1).unwrap();
        assert!(s.update("another", 2, Some(patch()), 3, 2).is_err());
        assert!(s.update(OWNER, 1, None, 3, 2).is_err());
        let mut invalid = patch();
        invalid.snap_gap_logical_px = 49;
        assert!(s.update(OWNER, 2, Some(invalid), 3, 2).is_err());
        assert_eq!(s.patch.unwrap().snap_gap_logical_px, 24);
    }
    #[test]
    fn expiry_conflicts_and_display_scope() {
        let saved = Settings::default();
        let mut s = Session::begin(OWNER.into(), saved.revision, 0).unwrap();
        s.update(OWNER, 1, Some(patch()), saved.revision, 1)
            .unwrap();
        let effective = s.effective(&saved);
        assert_eq!(effective.revision, saved.revision);
        assert_eq!(effective.mode, saved.mode);
        assert_eq!(effective.codex_path, saved.codex_path);
        assert_eq!(effective.windows.snap_gap_logical_px, 24);
        assert_eq!(saved.windows.snap_gap_logical_px, 8);
        assert!(!effective.widget_appearance.show_border);
        assert!(saved.widget_appearance.show_border);
        assert_eq!(effective.widget_appearance.edge_scale_percent, 80);
        assert_eq!(effective.widget_appearance.hud_scale_percent, 137);
        assert!(effective.widget_appearance.compact_countdown_bold);
        assert!(effective.widget_appearance.compact_countdown_high_contrast);
        assert_eq!(saved.widget_appearance.edge_scale_percent, 100);
        assert_eq!(saved.widget_appearance.hud_scale_percent, 100);
        assert!(!saved.widget_appearance.compact_countdown_bold);
        assert_eq!(
            effective
                .widget_appearance
                .resolve(true, false, true, true)
                .background_alpha,
            1.0
        );
        assert!(s.update(OWNER, 2, None, saved.revision + 1, 2).is_err());
        assert!(s.update(OWNER, 2, None, saved.revision, 10001).is_err());
    }
}

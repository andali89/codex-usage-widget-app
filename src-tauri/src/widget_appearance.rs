//! Validated paint roles and conservative contrast over arbitrary desktop content.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ROLES: [&str; 8] = [
    "background",
    "contentSurface",
    "text",
    "secondaryText",
    "fiveHour",
    "weekly",
    "track",
    "border",
];
pub type Palette = BTreeMap<String, String>;
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorMode {
    #[default]
    ThemeDefaults,
    Custom,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Palettes {
    pub light: Palette,
    pub dark: Palette,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PresetId {
    Original,
    Ocean,
    Forest,
    Plum,
    Slate,
    Amber,
}
impl PresetId {
    pub fn key(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Plum => "plum",
            Self::Slate => "slate",
            Self::Amber => "amber",
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct QuotaCustomization {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuotaThemes {
    pub light: QuotaCustomization,
    pub dark: QuotaCustomization,
}
pub fn preset_palette(id: PresetId, dark: bool) -> Palette {
    static PRESETS: std::sync::OnceLock<BTreeMap<String, Palettes>> = std::sync::OnceLock::new();
    let presets = PRESETS.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/appearance-presets.json"))
            .expect("bundled presets")
    });
    let mut palette = defaults(dark);
    let pair = &presets[id.key()];
    palette.extend(if dark {
        pair.dark.clone()
    } else {
        pair.light.clone()
    });
    palette
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct WidgetAppearance {
    pub preset_id: Option<PresetId>,
    pub quota_customizations: BTreeMap<PresetId, QuotaThemes>,
    #[serde(deserialize_with = "deserialize_transparency")]
    pub background_transparency_percent: u8,
    pub color_mode: ColorMode,
    pub palettes: Palettes,
}
fn deserialize_transparency<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    value.as_u64().filter(|value|*value<=80).map(|value|value as u8).ok_or_else(||serde::de::Error::custom("widgetAppearance.backgroundTransparencyPercent: use a whole percentage from 0 to 80"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_fixtures_validate_retained_maps_and_all_composite_backgrounds() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/widget-appearance.json"))
                .unwrap();
        for fixture in fixtures["palettes"].as_array().unwrap() {
            let mut appearance = WidgetAppearance::default();
            let overrides = serde_json::from_value(fixture["roles"].clone()).unwrap();
            if fixture["theme"] == "dark" {
                appearance.palettes.dark = overrides;
            } else {
                appearance.palettes.light = overrides;
            }
            assert_eq!(
                appearance.issues().is_empty(),
                fixture["valid"].as_bool().unwrap(),
                "{}",
                fixture["name"]
            );
        }
        for fixture in fixtures["composition"].as_array().unwrap() {
            assert!(
                (worst_contrast(
                    fixture["foreground"].as_str().unwrap(),
                    fixture["backing"].as_str().unwrap(),
                    fixture["alpha"].as_f64().unwrap()
                ) - fixture["worst"].as_f64().unwrap())
                .abs()
                    < 1e-6
            );
        }
    }
    #[test]
    fn normalization_and_override_precedence_preserve_saved_preferences() {
        let mut appearance = WidgetAppearance {
            background_transparency_percent: 80,
            color_mode: ColorMode::Custom,
            ..Default::default()
        };
        appearance
            .palettes
            .light
            .insert("fiveHour".into(), "#6d28d9".into());
        appearance.normalize();
        assert_eq!(appearance.palettes.light["fiveHour"], "#6D28D9");
        for (hc, allowed, composition, reason) in [
            (true, true, true, "high-contrast"),
            (false, false, true, "windows-transparency-off-or-unknown"),
            (false, true, false, "native-composition-unverified"),
        ] {
            let result = appearance.resolve(false, hc, allowed, composition);
            assert_eq!(result.background_alpha, 1.0);
            assert_eq!(result.reason, Some(reason));
        }
        let normal = appearance.resolve(false, false, true, true);
        assert!((normal.background_alpha - 0.2).abs() < 1e-6);
        assert_eq!(normal.backing_alpha, 1.0);
        assert_eq!(appearance.background_transparency_percent, 80);
    }
    #[test]
    fn presets_and_retained_customizations_validate_both_themes() {
        for id in [
            PresetId::Original,
            PresetId::Ocean,
            PresetId::Forest,
            PresetId::Plum,
            PresetId::Slate,
            PresetId::Amber,
        ] {
            let mut settings = WidgetAppearance {
                preset_id: Some(id),
                ..Default::default()
            };
            assert!(settings.issues().is_empty(), "{id:?}");
            let mut custom = QuotaThemes::default();
            custom.light.five_hour = Some("#FFFFFF".into());
            settings.quota_customizations.insert(id, custom);
            assert!(!settings.issues().is_empty());
        }
        for json in [
            serde_json::json!({"presetId":"unknown"}),
            serde_json::json!({"presetId":"original","quotaCustomizations":{"unknown":{}}}),
            serde_json::json!({"quotaCustomizations":{"original":{"light":{"other":true}}}}),
        ] {
            assert!(serde_json::from_value::<WidgetAppearance>(json).is_err());
        }
    }
    #[test]
    fn ipc_rejects_fractional_and_unknown_fields() {
        for value in [
            serde_json::json!({"backgroundTransparencyPercent":1.5}),
            serde_json::json!({"backgroundTransparencyPercent":null}),
            serde_json::json!({"backgroundTransparencyPercent":-1}),
            serde_json::json!({"other":true}),
            serde_json::json!({"palettes":{"system":{}}}),
        ] {
            assert!(serde_json::from_value::<WidgetAppearance>(value).is_err());
        }
        let value = WidgetAppearance {
            background_transparency_percent: 81,
            ..Default::default()
        };
        assert!(!value.issues().is_empty());
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub field: String,
    pub message: String,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveAppearance {
    pub original_structure: bool,
    pub palette: Palette,
    pub divider: String,
    pub background_alpha: f64,
    pub backing_alpha: f64,
    pub reason: Option<&'static str>,
    pub composition_available: bool,
    pub protect_content: bool,
}
pub fn defaults(dark: bool) -> Palette {
    static DEFAULTS: std::sync::OnceLock<Palettes> = std::sync::OnceLock::new();
    let p = DEFAULTS.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/appearance-defaults.json"))
            .expect("bundled palettes")
    });
    if dark {
        p.dark.clone()
    } else {
        p.light.clone()
    }
}
fn rgb(hex: &str) -> Option<[f64; 3]> {
    if hex.len() != 7
        || !hex.starts_with('#')
        || !hex.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return None;
    }
    let mut channels = [0.0; 3];
    for (i, channel) in channels.iter_mut().enumerate() {
        *channel = f64::from(u8::from_str_radix(&hex[1 + i * 2..3 + i * 2], 16).ok()?) / 255.0;
    }
    Some(channels)
}
fn luminance(rgb: [f64; 3]) -> f64 {
    rgb.into_iter()
        .zip([0.2126, 0.7152, 0.0722])
        .map(|(v, w)| {
            w * if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        })
        .sum()
}
fn ratio(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn contrast(a: &str, b: &str) -> f64 {
    ratio(luminance(rgb(a).unwrap()), luminance(rgb(b).unwrap()))
}
/// All RGB backgrounds produce a continuous luminance interval. A foreground
/// inside that interval has a worst-case ratio of 1, even if endpoints pass.
pub fn worst_contrast(foreground: &str, backing: &str, alpha: f64) -> f64 {
    let color = rgb(backing).unwrap();
    let low = luminance(color.map(|c| c * alpha));
    let high = luminance(color.map(|c| c * alpha + 1.0 - alpha));
    let fg = luminance(rgb(foreground).unwrap());
    if fg >= low && fg <= high {
        1.0
    } else {
        ratio(fg, low).min(ratio(fg, high))
    }
}
impl WidgetAppearance {
    pub fn palette(&self, dark: bool) -> Palette {
        let mut p = self
            .preset_id
            .map(|id| preset_palette(id, dark))
            .unwrap_or_else(|| defaults(dark));
        if let Some(id) = self.preset_id {
            if let Some(pair) = self.quota_customizations.get(&id) {
                let custom = if dark { &pair.dark } else { &pair.light };
                if custom.enabled {
                    if let Some(color) = &custom.five_hour {
                        p.insert("fiveHour".into(), color.clone());
                    }
                    if let Some(color) = &custom.weekly {
                        p.insert("weekly".into(), color.clone());
                    }
                }
            }
            return p;
        }
        if matches!(self.color_mode, ColorMode::Custom) {
            p.extend(
                if dark {
                    &self.palettes.dark
                } else {
                    &self.palettes.light
                }
                .clone(),
            );
        }
        p
    }
    fn legacy_issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        if self.background_transparency_percent > 80 {
            issues.push(Issue {
                field: "widgetAppearance.backgroundTransparencyPercent".into(),
                message: "Use a whole percentage from 0 to 80.".into(),
            });
        }
        for (theme, overrides) in [
            ("light", &self.palettes.light),
            ("dark", &self.palettes.dark),
        ] {
            let field = |role: &str| format!("widgetAppearance.palettes.{theme}.{role}");
            if overrides.len() > ROLES.len() {
                issues.push(Issue {
                    field: format!("widgetAppearance.palettes.{theme}"),
                    message: "Only the eight supported color roles are allowed.".into(),
                });
                continue;
            }
            let mut p = defaults(theme == "dark");
            for (role, value) in overrides {
                if !ROLES.contains(&role.as_str()) || rgb(value).is_none() {
                    issues.push(Issue {
                        field: field(if ROLES.contains(&role.as_str()) {
                            role
                        } else {
                            "unknownRole"
                        }),
                        message: "Choose a supported color role and a #RRGGBB color without alpha."
                            .into(),
                    });
                } else {
                    p.insert(role.clone(), value.clone());
                }
            }
            // Validate retained maps even while Theme defaults is selected.
            let base = defaults(theme == "dark");
            let mut pairs = Vec::new();
            for fg in ["text", "secondaryText", "fiveHour", "weekly"] {
                pairs.push((fg, "contentSurface", 4.5));
            }
            for fg in ["fiveHour", "weekly"] {
                pairs.push((fg, "track", 3.0));
            }
            // Essential outline and focus remain independently controlled.
            pairs.push(("border", "contentSurface", 3.0));
            for (fg, bg, minimum) in pairs {
                let r = contrast(&p[fg], &p[bg]);
                if r + 1e-8 < minimum {
                    issues.push(Issue { field: field(if overrides.contains_key(fg) { fg } else { bg }), message: format!("{fg} on {bg}: {r:.2}:1; needs {minimum}:1. Use theme default or adjust this pair.") });
                }
            }
            for role in ["fiveHour", "weekly"] {
                for background in if theme == "dark" {
                    ["#081426", "#10213A", "#193452", "#10233E"]
                } else {
                    ["#F3F6FB", "#FFFFFF", "#E8EFF8", "#F6FAFF"]
                } {
                    let r = contrast(&p[role], background);
                    if r + 1e-8 < 4.5 {
                        issues.push(Issue {
                            field: field(role),
                            message: format!(
                                "{role} on dashboard {background}: {r:.2}:1; needs 4.5:1."
                            ),
                        });
                    }
                }
                let r = contrast(&p[role], &base["track"]);
                if r + 1e-8 < 3.0 {
                    issues.push(Issue {
                        field: field(role),
                        message: format!("{role} on dashboard track: {r:.2}:1; needs 3:1."),
                    });
                }
            }
            // Semantic text and keyboard focus cannot be customized away.
            for foreground in if theme == "dark" {
                ["#FBBF24", "#FCA5A5", "#34D399", "#60A5FA"]
            } else {
                ["#92400E", "#B91C1C", "#047857", "#005FCC"]
            } {
                if contrast(foreground, &p["contentSurface"]) + 1e-8 < 4.5 {
                    issues.push(Issue { field: field("contentSurface"), message: "Content surface must retain readable status text and keyboard focus. Use theme default.".into() });
                }
            }
        }
        issues
    }
    pub fn issues(&self) -> Vec<Issue> {
        if self.preset_id.is_none() {
            return self.legacy_issues();
        }
        let mut issues = self.legacy_issues();
        for id in [
            PresetId::Original,
            PresetId::Ocean,
            PresetId::Forest,
            PresetId::Plum,
            PresetId::Slate,
            PresetId::Amber,
        ] {
            if self.preset_id != Some(id) && !self.quota_customizations.contains_key(&id) {
                continue;
            }
            for dark in [false, true] {
                let mut palette = preset_palette(id, dark);
                if let Some(pair) = self.quota_customizations.get(&id) {
                    let custom = if dark { &pair.dark } else { &pair.light };
                    if let Some(c) = &custom.five_hour {
                        palette.insert("fiveHour".into(), c.clone());
                    }
                    if let Some(c) = &custom.weekly {
                        palette.insert("weekly".into(), c.clone());
                    }
                }
                let theme = if dark { "dark" } else { "light" };
                let mut check = Self {
                    background_transparency_percent: self.background_transparency_percent,
                    ..Self::default()
                };
                if dark {
                    check.palettes.dark = palette;
                } else {
                    check.palettes.light = palette;
                }
                issues.extend(check.legacy_issues().into_iter().map(|mut issue| {
                    issue.field = issue.field.replace(
                        &format!("palettes.{theme}"),
                        &format!("quotaCustomizations.{}.{theme}", id.key()),
                    );
                    issue
                }));
            }
        }
        issues
    }
    pub fn normalize(&mut self) {
        for pair in self.quota_customizations.values_mut() {
            for custom in [&mut pair.light, &mut pair.dark] {
                if let Some(c) = &mut custom.five_hour {
                    c.make_ascii_uppercase();
                }
                if let Some(c) = &mut custom.weekly {
                    c.make_ascii_uppercase();
                }
            }
        }
        for value in self
            .palettes
            .light
            .values_mut()
            .chain(self.palettes.dark.values_mut())
        {
            value.make_ascii_uppercase();
        }
    }
    pub fn resolve(
        &self,
        dark: bool,
        high_contrast: bool,
        transparency_allowed: bool,
        composition_available: bool,
    ) -> EffectiveAppearance {
        let palette = if high_contrast {
            defaults(dark)
        } else {
            self.palette(dark)
        };
        let reason = if high_contrast {
            Some("high-contrast")
        } else if !transparency_allowed {
            Some("windows-transparency-off-or-unknown")
        } else if !composition_available {
            Some("native-composition-unverified")
        } else {
            None
        };
        let background_alpha = if reason.is_some() {
            1.0
        } else {
            1.0 - f64::from(self.background_transparency_percent) / 100.0
        };
        let low = luminance(rgb(&palette["background"]).unwrap())
            .min(luminance(rgb(&palette["contentSurface"]).unwrap()));
        let high = luminance(rgb(&palette["background"]).unwrap())
            .max(luminance(rgb(&palette["contentSurface"]).unwrap()));
        let mut foregrounds = ["text", "secondaryText", "fiveHour", "weekly"]
            .into_iter()
            .map(|role| palette[role].as_str())
            .collect::<Vec<_>>();
        foregrounds.extend(if dark {
            ["#FBBF24", "#FCA5A5", "#34D399", "#60A5FA"]
        } else {
            ["#92400E", "#B91C1C", "#047857", "#005FCC"]
        });
        let protect_content = background_alpha < 1.0
            || foregrounds.into_iter().any(|fg| {
                let value = luminance(rgb(fg).unwrap());
                worst_contrast(fg, &palette["background"], background_alpha) < 4.5
                    || value >= low && value <= high
            })
            || contrast(&palette["border"], &palette["background"]) < 3.0;
        // Opaque content backing is the conservative fallback for semantic labels,
        // focus and gradients; decorative areas retain the requested alpha.
        let backing_alpha = 1.0;
        let overrides = if dark {
            &self.palettes.dark
        } else {
            &self.palettes.light
        };
        let divider = if !high_contrast
            && matches!(self.color_mode, ColorMode::Custom)
            && overrides.contains_key("border")
        {
            palette["border"].clone()
        } else {
            if dark { "#334B6B" } else { "#CBD5E1" }.into()
        };
        EffectiveAppearance {
            original_structure: self.preset_id == Some(PresetId::Original)
                || (self.preset_id.is_none()
                    && matches!(self.color_mode, ColorMode::ThemeDefaults)),
            palette,
            divider,
            background_alpha,
            backing_alpha,
            reason,
            composition_available,
            protect_content,
        }
    }
}

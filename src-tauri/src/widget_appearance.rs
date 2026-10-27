//! Validated palettes and derived readable quota paint on known app surfaces.
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
    Glacier,
    Matcha,
    Orchid,
    Slate,
    Graphite,
    Rosewood,
    Sapphire,
    Sunrise,
    Mint,
    Terracotta,
}
impl PresetId {
    pub fn key(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Glacier => "glacier",
            Self::Matcha => "matcha",
            Self::Orchid => "orchid",
            Self::Slate => "slate",
            Self::Graphite => "graphite",
            Self::Rosewood => "rosewood",
            Self::Sapphire => "sapphire",
            Self::Sunrise => "sunrise",
            Self::Mint => "mint",
            Self::Terracotta => "terracotta",
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct WidgetAppearance {
    pub show_border: bool,
    pub preset_id: Option<PresetId>,
    pub quota_customizations: BTreeMap<PresetId, QuotaThemes>,
    #[serde(deserialize_with = "deserialize_transparency")]
    pub background_transparency_percent: u8,
    pub color_mode: ColorMode,
    pub palettes: Palettes,
}
impl Default for WidgetAppearance {
    fn default() -> Self {
        Self {
            show_border: true,
            preset_id: None,
            quota_customizations: BTreeMap::new(),
            background_transparency_percent: 0,
            color_mode: ColorMode::default(),
            palettes: Palettes::default(),
        }
    }
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
        assert_eq!(normal.background_alpha, 1.0);
        assert_eq!(normal.backing_alpha, 1.0);
        assert_eq!(normal.reason, Some("transparency-disabled"));
        assert_eq!(appearance.background_transparency_percent, 80);
    }
    #[test]
    fn border_defaults_on_and_accessibility_overrides_off() {
        let default: WidgetAppearance = serde_json::from_str("{}").unwrap();
        assert!(default.show_border);
        assert!(WidgetAppearance::default().show_border);
        for dark in [false, true] {
            let mut appearance = default.clone();
            let on = appearance.resolve(dark, false, true, true);
            appearance.show_border = false;
            let off = appearance.resolve(dark, false, true, true);
            assert_eq!(off.frame_border, "transparent");
            assert_eq!(off.divider, on.divider);
            assert_eq!(off.palette, on.palette);
            assert_eq!(off.quota_text, on.quota_text);
            assert_ne!(
                appearance.resolve(dark, true, true, true).frame_border,
                "transparent"
            );
            for percent in [0, 20, 50, 80] {
                appearance.background_transparency_percent = percent;
                let result = appearance.resolve(dark, false, true, true);
                assert_eq!(result.background_alpha, 1.0);
                assert_eq!(result.backing_alpha, 1.0);
                assert_eq!(appearance.background_transparency_percent, percent);
            }
        }
    }
    #[test]
    fn presets_and_retained_customizations_validate_both_themes() {
        for id in [
            PresetId::Original,
            PresetId::Glacier,
            PresetId::Matcha,
            PresetId::Orchid,
            PresetId::Slate,
            PresetId::Terracotta,
            PresetId::Graphite,
            PresetId::Rosewood,
            PresetId::Sapphire,
            PresetId::Sunrise,
            PresetId::Mint,
        ] {
            let mut settings = WidgetAppearance {
                preset_id: Some(id),
                ..Default::default()
            };
            assert!(settings.issues().is_empty(), "{id:?}");
            let mut custom = QuotaThemes::default();
            custom.light.five_hour = Some("#FFF".into());
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
    fn shared_derived_paint_contract() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/widget-derived-paint.json"
        ))
        .unwrap();
        for f in fixtures.as_array().unwrap() {
            let id: PresetId = serde_json::from_value(f["id"].clone()).unwrap();
            let dark = f["theme"] == "dark";
            let mut appearance = WidgetAppearance {
                preset_id: Some(id),
                show_border: f["showBorder"].as_bool().unwrap_or(true),
                ..Default::default()
            };
            if let Some(colors) = f["colors"].as_object() {
                let pair = appearance.quota_customizations.entry(id).or_default();
                let c = if dark {
                    &mut pair.dark
                } else {
                    &mut pair.light
                };
                c.enabled = true;
                c.five_hour = Some(colors["fiveHour"].as_str().unwrap().into());
                c.weekly = Some(colors["weekly"].as_str().unwrap().into());
            }
            assert!(appearance.issues().is_empty());
            let resolved =
                serde_json::to_value(appearance.resolve(dark, false, true, true)).unwrap();
            for key in ["frameBorder", "divider", "quotaText"] {
                assert_eq!(resolved[key], f[key], "{f} {key}");
            }
        }
    }
    #[test]
    fn ipc_rejects_fractional_and_unknown_fields() {
        for value in [
            serde_json::json!({"backgroundTransparencyPercent":1.5}),
            serde_json::json!({"backgroundTransparencyPercent":null}),
            serde_json::json!({"backgroundTransparencyPercent":-1}),
            serde_json::json!({"showBorder":null}),
            serde_json::json!({"showBorder":"false"}),
            serde_json::json!({"showBorder":0}),
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
    pub frame_border: String,
    pub quota_text: Palette,
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
fn mix_color(a: &str, b: &str, amount: f64) -> String {
    let aa = rgb(a).unwrap();
    let bb = rgb(b).unwrap();
    let c = std::array::from_fn::<_, 3, _>(|i| {
        (255.0 * (aa[i] * (1.0 - amount) + bb[i] * amount)).round() as u8
    });
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
fn quota_paint(palette: &Palette, dark: bool) -> Palette {
    let mut surfaces = vec![palette["contentSurface"].as_str()];
    surfaces.extend(if dark {
        ["#081426", "#10213A", "#193452", "#10233E"]
    } else {
        ["#F3F6FB", "#FFFFFF", "#E8EFF8", "#F6FAFF"]
    });
    let readable = |color: &str, backgrounds: &[&str], minimum: f64| {
        let target = if dark { "#FFFFFF" } else { "#000000" };
        (0..=100)
            .map(|step| mix_color(color, target, f64::from(step) / 100.0))
            .find(|shade| {
                backgrounds
                    .iter()
                    .all(|bg| contrast(shade, bg) + 1e-8 >= minimum)
            })
            .unwrap_or(target.into())
    };
    let mut text = Palette::new();
    for role in ["fiveHour", "weekly"] {
        text.insert(role.into(), readable(&palette[role], &surfaces, 4.5));
    }
    text
}
/// All RGB backgrounds produce a continuous luminance interval. A foreground
/// inside that interval has a worst-case ratio of 1, even if endpoints pass.
#[cfg(test)]
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
            PresetId::Glacier,
            PresetId::Matcha,
            PresetId::Orchid,
            PresetId::Slate,
            PresetId::Terracotta,
            PresetId::Graphite,
            PresetId::Rosewood,
            PresetId::Sapphire,
            PresetId::Sunrise,
            PresetId::Mint,
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
                for role in ["fiveHour", "weekly"] {
                    if rgb(&palette[role]).is_none() {
                        issues.push(Issue {
                            field: format!(
                                "widgetAppearance.quotaCustomizations.{}.{theme}.{role}",
                                id.key()
                            ),
                            message: "Choose a #RRGGBB color without alpha.".into(),
                        });
                    }
                }
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
            Some("transparency-disabled")
        };
        // Preserve the requested percentage, but keep every painted surface opaque.
        let background_alpha = 1.0;
        let overrides = if dark {
            &self.palettes.dark
        } else {
            &self.palettes.light
        };
        let original_structure = self.preset_id == Some(PresetId::Original)
            || (self.preset_id.is_none() && matches!(self.color_mode, ColorMode::ThemeDefaults));
        let divider = if !original_structure
            || (!high_contrast
                && matches!(self.color_mode, ColorMode::Custom)
                && overrides.contains_key("border"))
        {
            palette["border"].clone()
        } else {
            if dark { "#334B6B" } else { "#CBD5E1" }.into()
        };
        let frame_base = if original_structure {
            &divider
        } else {
            &palette["border"]
        };
        let frame_border = if !self.show_border && !high_contrast {
            "transparent".into()
        } else if dark && (self.preset_id.is_none() || self.preset_id == Some(PresetId::Original)) {
            frame_base.clone()
        } else {
            mix_color(frame_base, &palette["background"], 0.35)
        };
        let quota_text = quota_paint(&palette, dark);
        EffectiveAppearance {
            frame_border,
            quota_text,
            original_structure: self.preset_id == Some(PresetId::Original)
                || (self.preset_id.is_none()
                    && matches!(self.color_mode, ColorMode::ThemeDefaults)),
            palette,
            divider,
            background_alpha,
            backing_alpha: background_alpha,
            reason,
            composition_available,
            protect_content: false,
        }
    }
}

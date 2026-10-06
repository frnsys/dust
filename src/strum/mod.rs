//! Strumming: turns block chords into guitar-like strummed note events.
mod params;
mod pattern;
mod render;
mod strummer;

pub use params::StrumParams;
pub use pattern::{StrumPattern, StrokeEvent, Stroke};
pub use render::{render_strummed, render_block};
pub use strummer::{Strummer, NoteEvent, EventKind, sort_events};

use anyhow::{Result, Context, bail};
use serde::Deserialize;
use serde_yaml::Value;

/// The strum pattern library, compiled into the binary.
pub const DEFAULT_LIBRARY: &str = include_str!("../../strums.yaml");

/// A named strum pattern with its fully resolved parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct StrumPreset {
    pub name: String,
    pub pattern: StrumPattern,
    pub params: StrumParams,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrumLibrary {
    pub defaults: StrumParams,
    pub presets: Vec<StrumPreset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryFile {
    #[serde(default)]
    defaults: Value,
    #[serde(default)]
    patterns: Vec<Value>,
}

/// Apply the entries of `overrides` (a YAML mapping) on top of `base`.
fn merge(base: &Value, overrides: &Value) -> Value {
    let mut out = base.clone();
    if let (Value::Mapping(out), Value::Mapping(overrides)) = (&mut out, overrides) {
        for (k, v) in overrides {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn params_from(value: &Value) -> Result<StrumParams> {
    let value = if value.is_null() { Value::Mapping(Default::default()) } else { value.clone() };
    Ok(serde_yaml::from_value(value)?)
}

impl StrumLibrary {
    pub fn from_yaml(yaml: &str) -> Result<StrumLibrary> {
        let file: LibraryFile = serde_yaml::from_str(yaml).context("invalid strum library")?;
        let defaults_value = if file.defaults.is_null() {
            Value::Mapping(Default::default())
        } else {
            file.defaults
        };
        let defaults = params_from(&defaults_value).context("invalid strum defaults")?;

        let mut presets = vec![];
        for entry in &file.patterns {
            let Value::Mapping(map) = entry else {
                bail!("strum pattern entries must be mappings with `name` and `pattern`");
            };
            let name = match map.get("name") {
                Some(Value::String(s)) => s.clone(),
                _ => bail!("strum pattern entry is missing a `name`"),
            };
            let pattern = match map.get("pattern") {
                Some(Value::String(s)) => s.parse::<StrumPattern>()
                    .with_context(|| format!("strum pattern `{}`", name))?,
                _ => bail!("strum pattern `{}` is missing a `pattern`", name),
            };
            let mut overrides = map.clone();
            overrides.remove("name");
            overrides.remove("pattern");
            let params = params_from(&merge(&defaults_value, &Value::Mapping(overrides)))
                .with_context(|| format!("strum pattern `{}`", name))?;
            presets.push(StrumPreset { name, pattern, params });
        }
        Ok(StrumLibrary { defaults, presets })
    }

    pub fn default_library() -> StrumLibrary {
        StrumLibrary::from_yaml(DEFAULT_LIBRARY).expect("built-in strum library is invalid")
    }

    pub fn get(&self, name: &str) -> Option<&StrumPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    pub fn names(&self) -> Vec<String> {
        self.presets.iter().map(|p| p.name.clone()).collect()
    }
}

/// Apply `key=value` overrides (values parsed as YAML scalars) to params.
pub fn override_params(params: &StrumParams, overrides: &[String]) -> Result<StrumParams> {
    let mut value = serde_yaml::to_value(params)?;
    for kv in overrides {
        let (k, v) = kv.split_once('=')
            .with_context(|| format!("override `{}` should look like key=value", kv))?;
        let v: Value = serde_yaml::from_str(v)?;
        if let Value::Mapping(map) = &mut value {
            map.insert(Value::String(k.to_string()), v);
        }
    }
    params_from(&value)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_builtin_library_loads() {
        let lib = StrumLibrary::default_library();
        assert!(lib.presets.len() > 3);
        assert!(lib.get("folk").is_some());
    }

    #[test]
    fn test_library_overrides() {
        let lib = StrumLibrary::from_yaml("
defaults:
  spread_ms: 50
patterns:
  - name: a
    pattern: d u
  - name: b
    pattern: d. du
    spread_ms: 20
    swing: 0.5
").unwrap();
        assert_eq!(lib.defaults.spread_ms, 50.0);
        assert_eq!(lib.get("a").unwrap().params.spread_ms, 50.0);
        assert_eq!(lib.get("a").unwrap().params.swing, 0.0);
        assert_eq!(lib.get("b").unwrap().params.spread_ms, 20.0);
        assert_eq!(lib.get("b").unwrap().params.swing, 0.5);
        assert_eq!(lib.get("b").unwrap().pattern.strokes.len(), 3);
    }

    #[test]
    fn test_library_rejects_bad_input() {
        assert!(StrumLibrary::from_yaml("patterns:\n  - name: a\n    pattern: d q").is_err());
        assert!(StrumLibrary::from_yaml("patterns:\n  - name: a\n    pattern: d\n    sprd: 1").is_err());
        assert!(StrumLibrary::from_yaml("patterns:\n  - pattern: d").is_err());
    }

    #[test]
    fn test_override_params() {
        let p = override_params(&StrumParams::default(), &["spread_ms=80".into(), "mute_hit=false".into()]).unwrap();
        assert_eq!(p.spread_ms, 80.0);
        assert!(!p.mute_hit);
        assert!(override_params(&StrumParams::default(), &["nope=1".into()]).is_err());
    }
}

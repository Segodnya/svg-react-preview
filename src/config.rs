//! All env-var parsing for the binary lives here.
//!
//! Every `SVG_REACT_PREVIEW_*` knob is read once in [`Config::from_env`] and
//! exposed as a typed value to the rest of `main`. Adding a new env var means
//! adding a field here — there is nowhere else to look.

use std::env;
use std::fs;
use std::io::{self, Read};

use anyhow::{Result, anyhow};
use svg_react_preview::source::Source;

const ENV_FILE: &str = "SVG_REACT_PREVIEW_FILE";
const ENV_ROW: &str = "SVG_REACT_PREVIEW_ROW";
const ENV_COLUMN: &str = "SVG_REACT_PREVIEW_COLUMN";
const ENV_INPUT: &str = "SVG_REACT_PREVIEW_INPUT";
const ENV_OPENER: &str = "SVG_REACT_PREVIEW_OPENER";
const ENV_HOTKEY: &str = "SVG_REACT_PREVIEW_HOTKEY";

pub struct Config {
    pub source: Source,
    pub opener: Opener,
    pub hotkey: HotkeySpec,
}

/// How the freshly-rendered SVG file should be opened — and, by extension,
/// whether the auto-preview keystroke should be synthesised.
///
/// The `Custom` arm is the seam tests use (via `SVG_REACT_PREVIEW_OPENER=true`)
/// to skip both the real editor and the macOS keystroke. Any user override of
/// the opener implies "I'm routing this somewhere other than Zed" — synthesising
/// `Cmd+Shift+V` against an unrelated frontmost app would be user-hostile.
pub enum Opener {
    Default,
    Custom(String),
}

impl Opener {
    pub fn command(&self) -> &str {
        match self {
            Self::Default => "zed",
            Self::Custom(c) => c,
        }
    }

    #[cfg(target_os = "macos")]
    pub const fn synthesise_keystroke(&self) -> bool {
        matches!(self, Self::Default)
    }
}

pub enum HotkeySpec {
    Default,
    Disabled,
    #[cfg(target_os = "macos")]
    Custom(String),
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            source: source_from_env()?,
            opener: opener_from_env(),
            hotkey: hotkey_from_env(),
        })
    }
}

/// Resolves the input mode from env + stdin per the precedence documented
/// in the README: `FILE`+`ROW`+`COLUMN` → `INPUT` → stdin.
fn source_from_env() -> Result<Source> {
    let file = non_empty_env(ENV_FILE);
    let row = non_empty_env(ENV_ROW).and_then(|s| s.parse::<usize>().ok());
    let col = non_empty_env(ENV_COLUMN).and_then(|s| s.parse::<usize>().ok());

    if let (Some(path), Some(row), Some(col)) = (file, row, col) {
        let source = fs::read_to_string(&path).map_err(|e| anyhow!("cannot read {path}: {e}"))?;
        return Ok(Source::Cursor {
            source,
            path,
            row,
            col,
        });
    }

    if let Some(v) = non_empty_env(ENV_INPUT) {
        return Ok(Source::Fragment(v));
    }

    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(Source::Fragment(buf))
}

fn opener_from_env() -> Opener {
    non_empty_env(ENV_OPENER).map_or(Opener::Default, Opener::Custom)
}

fn hotkey_from_env() -> HotkeySpec {
    match non_empty_env(ENV_HOTKEY) {
        None => HotkeySpec::Default,
        Some(s) if s.eq_ignore_ascii_case("none") => HotkeySpec::Disabled,
        #[cfg(target_os = "macos")]
        Some(s) => HotkeySpec::Custom(s),
        #[cfg(not(target_os = "macos"))]
        Some(_) => HotkeySpec::Default,
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn opener_default_synthesises_keystroke() {
        let o = Opener::Default;
        assert_eq!(o.command(), "zed");
        assert!(o.synthesise_keystroke());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opener_custom_skips_keystroke() {
        let o = Opener::Custom("true".into());
        assert_eq!(o.command(), "true");
        assert!(!o.synthesise_keystroke());
    }

    #[test]
    fn opener_default_command_is_zed() {
        assert_eq!(Opener::Default.command(), "zed");
    }

    #[test]
    fn opener_custom_command_passes_through() {
        assert_eq!(Opener::Custom("true".into()).command(), "true");
    }
}

//! Delivers a rendered SVG to the user.
//!
//! Two side effects, one decision: open the file in an editor, and on macOS
//! synthesise the preview shortcut so Zed flips the freshly-opened tab to its
//! built-in SVG preview. Whether the keystroke fires is decided here, against
//! the [`Opener`] variant — the "custom opener implies skip keystroke" invariant
//! lives in one match instead of being split across two modules.
//!
//! Hotkey parsing and the `AppleScript` orchestration are macOS-only; the rest
//! is platform-agnostic.

use std::path::Path;
use std::process::Command;

use crate::config::{HotkeySpec, Opener};

const DEFAULT_HOTKEY: &str = "cmd+shift+v";

pub fn deliver(path: &Path, opener: &Opener, hotkey: &HotkeySpec) {
    open_in_editor(path, opener);
    if opener.synthesise_keystroke() {
        trigger_preview(hotkey);
    }
}

fn open_in_editor(path: &Path, opener: &Opener) {
    let bin = opener.command();
    if Command::new(bin).arg(path).spawn().is_err() {
        eprintln!(
            "svg-react-preview: {} not found in PATH; preview saved at: {}",
            bin,
            path.display()
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn trigger_preview(_: &HotkeySpec) {
    // Linux/Windows: user presses the preview shortcut manually (see README).
}

#[cfg(target_os = "macos")]
fn trigger_preview(hotkey: &HotkeySpec) {
    macos::trigger(hotkey);
}

struct Hotkey {
    modifiers: Vec<&'static str>,
    key_code: u8,
}

impl Hotkey {
    fn applescript_clause(&self) -> String {
        if self.modifiers.is_empty() {
            String::new()
        } else {
            format!("using {{{}}}", self.modifiers.join(", "))
        }
    }
}

fn parse(spec: &str) -> Option<Hotkey> {
    let mut modifiers = Vec::new();
    let mut key_code: Option<u8> = None;

    for part in spec.split('+') {
        let token = part.trim().to_ascii_lowercase();
        if token.is_empty() {
            return None;
        }
        if let Some(modifier) = parse_modifier(&token) {
            if modifiers.contains(&modifier) {
                return None;
            }
            modifiers.push(modifier);
        } else if let Some(code) = key_code_for(&token) {
            if key_code.replace(code).is_some() {
                return None;
            }
        } else {
            return None;
        }
    }

    Some(Hotkey {
        modifiers,
        key_code: key_code?,
    })
}

fn parse_modifier(token: &str) -> Option<&'static str> {
    match token {
        "cmd" | "command" => Some("command down"),
        "shift" => Some("shift down"),
        "alt" | "opt" | "option" => Some("option down"),
        "ctrl" | "control" => Some("control down"),
        _ => None,
    }
}

fn key_code_for(name: &str) -> Option<u8> {
    Some(match name {
        "a" => 0,
        "s" => 1,
        "d" => 2,
        "f" => 3,
        "h" => 4,
        "g" => 5,
        "z" => 6,
        "x" => 7,
        "c" => 8,
        "v" => 9,
        "b" => 11,
        "q" => 12,
        "w" => 13,
        "e" => 14,
        "r" => 15,
        "y" => 16,
        "t" => 17,
        "o" => 31,
        "u" => 32,
        "i" => 34,
        "p" => 35,
        "l" => 37,
        "j" => 38,
        "k" => 40,
        "n" => 45,
        "m" => 46,
        "0" => 29,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "5" => 23,
        "6" => 22,
        "7" => 26,
        "8" => 28,
        "9" => 25,
        "space" => 49,
        "return" | "enter" => 36,
        "tab" => 48,
        "escape" | "esc" => 53,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{DEFAULT_HOTKEY, Hotkey, HotkeySpec, parse};

    const OSASCRIPT_TIMEOUT_MS: &str = "3000";

    const SCRIPT_TEMPLATE: &str = r#"
on run argv
    set deadline to (current date) + ((item 1 of argv) as integer) / 1000
    repeat while (current date) is less than deadline
        try
            tell application "System Events" to tell process "Zed"
                if exists window 1 then
                    set frontmost to true
                    delay 0.2
                    try
                        key code __KEY_CODE__ __MODIFIERS__
                        return "ok"
                    on error number errNum
                        return "denied:" & errNum
                    end try
                end if
            end tell
        end try
        delay 0.05
    end repeat
    return "timeout"
end run
"#;

    pub fn trigger(hotkey: &HotkeySpec) {
        let Some(parsed) = resolve(hotkey) else {
            return;
        };

        let script = SCRIPT_TEMPLATE
            .replace("__KEY_CODE__", &parsed.key_code.to_string())
            .replace("__MODIFIERS__", &parsed.applescript_clause());

        run_osascript(&script);
    }

    fn resolve(hotkey: &HotkeySpec) -> Option<Hotkey> {
        let spec = match hotkey {
            HotkeySpec::Disabled => return None,
            HotkeySpec::Default => DEFAULT_HOTKEY,
            HotkeySpec::Custom(s) => s.as_str(),
        };

        let parsed = parse(spec);
        if parsed.is_none() {
            eprintln!(
                "svg-react-preview: cannot parse SVG_REACT_PREVIEW_HOTKEY={spec:?} — \
                 skipping auto-preview. Expected something like 'cmd+shift+v', or 'none' \
                 to disable."
            );
        }
        parsed
    }

    #[cfg(test)]
    pub(super) fn resolve_for_test(hotkey: &HotkeySpec) -> Option<super::Hotkey> {
        resolve(hotkey)
    }

    fn run_osascript(script: &str) {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let Ok(mut child) = Command::new("osascript")
            .args(["-", OSASCRIPT_TIMEOUT_MS])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(script.as_bytes());
        }

        let Ok(output) = child.wait_with_output() else {
            return;
        };

        match String::from_utf8_lossy(&output.stdout).trim() {
            "ok" => {}
            s if s.starts_with("denied:") => {
                let num = s.strip_prefix("denied:").unwrap_or("");
                eprintln!(
                    "svg-react-preview: keystroke blocked by macOS (osascript error {num}) — \
                     grant Accessibility to the app that ran this binary \
                     (System Settings → Privacy & Security → Accessibility). \
                     Press the preview shortcut manually for now."
                );
            }
            _ => eprintln!(
                "svg-react-preview: Zed did not come to the foreground in time. \
                 Press the preview shortcut manually."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default() {
        let h = parse("cmd+shift+v").unwrap();
        assert_eq!(h.key_code, 9);
        assert_eq!(h.modifiers, ["command down", "shift down"]);
    }

    #[test]
    fn case_and_whitespace_insensitive() {
        let h = parse("  Cmd + SHIFT + V  ").unwrap();
        assert_eq!(h.key_code, 9);
        assert_eq!(h.modifiers.len(), 2);
    }

    #[test]
    fn modifier_synonyms() {
        assert_eq!(parse("opt+p").unwrap().modifiers, ["option down"]);
        assert_eq!(parse("alt+p").unwrap().modifiers, ["option down"]);
        assert_eq!(parse("option+p").unwrap().modifiers, ["option down"]);
        assert_eq!(parse("ctrl+p").unwrap().modifiers, ["control down"]);
        assert_eq!(parse("control+p").unwrap().modifiers, ["control down"]);
        assert_eq!(parse("command+p").unwrap().modifiers, ["command down"]);
    }

    #[test]
    fn function_and_special_keys() {
        assert_eq!(parse("f5").unwrap().key_code, 96);
        assert_eq!(parse("cmd+space").unwrap().key_code, 49);
        assert_eq!(parse("ctrl+enter").unwrap().key_code, 36);
        assert_eq!(parse("escape").unwrap().key_code, 53);
    }

    #[test]
    fn rejects_two_non_modifiers() {
        assert!(parse("cmd+v+p").is_none());
    }

    #[test]
    fn rejects_duplicate_modifier() {
        assert!(parse("cmd+cmd+v").is_none());
        assert!(parse("shift+cmd+shift+v").is_none());
    }

    #[test]
    fn rejects_unknown_token() {
        assert!(parse("cmd+shift+foobar").is_none());
    }

    #[test]
    fn rejects_no_key() {
        assert!(parse("cmd+shift").is_none());
    }

    #[test]
    fn rejects_empty_segment() {
        assert!(parse("").is_none());
        assert!(parse("cmd++v").is_none());
        assert!(parse("+v").is_none());
    }

    #[test]
    fn applescript_clause_with_modifiers() {
        let h = parse("cmd+shift+v").unwrap();
        assert_eq!(h.applescript_clause(), "using {command down, shift down}");
    }

    #[test]
    fn applescript_clause_without_modifiers() {
        let h = parse("f5").unwrap();
        assert_eq!(h.applescript_clause(), "");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_default_returns_default_hotkey() {
        let h = macos::resolve_for_test(&HotkeySpec::Default).unwrap();
        assert_eq!(h.key_code, 9);
        assert_eq!(h.modifiers, ["command down", "shift down"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_disabled_returns_none() {
        assert!(macos::resolve_for_test(&HotkeySpec::Disabled).is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_custom_valid() {
        let h = macos::resolve_for_test(&HotkeySpec::Custom("f5".into())).unwrap();
        assert_eq!(h.key_code, 96);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_custom_invalid_returns_none() {
        assert!(macos::resolve_for_test(&HotkeySpec::Custom("garbage".into())).is_none());
    }
}

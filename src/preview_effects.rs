//! Delivers a rendered SVG to the user.
//!
//! Two side effects, one decision: open the file in an editor, and on macOS
//! synthesise the preview shortcut so Zed shows its built-in SVG preview.
//! Whether keystrokes fire is decided here, against the [`Opener`] variant —
//! the "custom opener implies skip keystroke" invariant lives in one match
//! instead of being split across two modules.
//!
//! On macOS with the default opener and a parseable hotkey, the `AppleScript`
//! path both opens the file and dispatches the keystrokes in a single
//! transaction. The script polls `name of window 1` for the just-opened
//! file's basename before sending any keystroke, eliminating the focus
//! race where `zed <path>` returned before Zed had surfaced the new tab
//! (which made `Cmd+Shift+V` land on whatever tab was previously active —
//! typically the previously-opened preview).
//!
//! After the preview keystroke, two more keystrokes fire to clean up the
//! redundant text-mode `.svg` tab: `Cmd+Shift+[` (Zed default
//! `pane::ActivatePrevItem`) followed by `Cmd+W` (`pane::CloseActiveItem`).
//! This relies on Zed opening the preview as a NEW tab to the right of the
//! source `.svg` text tab in the SAME pane, so the immediately-previous tab
//! in tab order IS the text-mode `.svg`. Corner cases where this assumption
//! breaks:
//!   - User remapped `pane::ActivatePrevItem` or `pane::CloseActiveItem` away
//!     from `Cmd+Shift+[` / `Cmd+W` — the wrong tab may be focused and closed.
//!   - Preview opens in a separate split pane (not Zed's default for SVG):
//!     "previous item" navigates within the preview pane (which has only one
//!     item) and `Cmd+W` would close the preview itself.
//!   - User opened the same SVG previously, then activated some other tab
//!     manually before re-running the task: the existing text-mode tab is
//!     re-focused by `tell app "Zed" to open …`, but the "previous item"
//!     after `Cmd+Shift+V` is still the prior tab in strip order — usually
//!     still correct, but tab-strip order in edge cases is not guaranteed.
//!
//! In all of these the user can disable the auto-preview keystrokes with
//! `SVG_REACT_PREVIEW_HOTKEY=none` to fall back to a plain `zed <path>` open.
//!
//! Hotkey parsing and the `AppleScript` orchestration are macOS-only; the rest
//! is platform-agnostic.

use std::path::Path;
use std::process::Command;

use crate::config::{HotkeySpec, Opener};

const DEFAULT_HOTKEY: &str = "cmd+shift+v";

pub fn deliver(path: &Path, opener: &Opener, hotkey: &HotkeySpec) {
    #[cfg(target_os = "macos")]
    if opener.synthesise_keystroke()
        && !matches!(hotkey, HotkeySpec::Disabled)
        && macos::open_and_preview(path, hotkey)
    {
        return;
    }
    // Unparseable hotkey or non-macOS or custom opener: just spawn the configured opener.
    open_in_editor(path, opener);
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
    use std::path::Path;

    use super::{DEFAULT_HOTKEY, Hotkey, HotkeySpec, parse};

    const OSASCRIPT_TIMEOUT_MS: &str = "3000";

    // pane::ActivatePrevItem default chord = Cmd+Shift+[  (key code 33)
    // pane::CloseActiveItem default chord  = Cmd+W        (key code 13)
    const PREV_ITEM_KEY: u8 = 33;
    const CLOSE_ITEM_KEY: u8 = 13;

    // The script polls `name of window 1` for the just-opened file's basename
    // before dispatching keystrokes. This replaces the old fixed `delay 0.2`
    // and removes the residual focus-race on a cold-start Zed.
    const SCRIPT_TEMPLATE: &str = r#"
on run argv
    set deadline to (current date) + ((item 1 of argv) as integer) / 1000
    set svgPath to item 2 of argv
    set svgBasename to item 3 of argv
    tell application "Zed" to open POSIX file svgPath
    repeat while (current date) is less than deadline
        try
            tell application "System Events" to tell process "Zed"
                if exists window 1 then
                    if (name of window 1) contains svgBasename then
                        set frontmost to true
                        delay 0.05
                        try
                            key code __PREVIEW_KEY_CODE__ __PREVIEW_MODIFIERS__
                            delay 0.15
                            key code __PREV_ITEM__ using {command down, shift down}
                            delay 0.05
                            key code __CLOSE_ITEM__ using {command down}
                            return "ok"
                        on error number errNum
                            return "denied:" & errNum
                        end try
                    end if
                end if
            end tell
        end try
        delay 0.05
    end repeat
    return "timeout"
end run
"#;

    pub fn open_and_preview(path: &Path, hotkey: &HotkeySpec) -> bool {
        let Some(parsed) = resolve(hotkey) else {
            return false;
        };
        let script = build_script(&parsed);
        run_osascript(&script, path);
        true
    }

    fn build_script(preview: &Hotkey) -> String {
        SCRIPT_TEMPLATE
            .replace("__PREVIEW_KEY_CODE__", &preview.key_code.to_string())
            .replace("__PREVIEW_MODIFIERS__", &preview.applescript_clause())
            .replace("__PREV_ITEM__", &PREV_ITEM_KEY.to_string())
            .replace("__CLOSE_ITEM__", &CLOSE_ITEM_KEY.to_string())
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

    #[cfg(test)]
    pub(super) fn build_script_for_test(preview: &Hotkey) -> String {
        build_script(preview)
    }

    fn run_osascript(script: &str, path: &Path) {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let Some(path_str) = path.to_str() else {
            eprintln!(
                "svg-react-preview: preview path contains non-UTF-8 bytes; \
                 cannot pass to osascript: {}",
                path.display()
            );
            return;
        };
        let basename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path_str);

        let Ok(mut child) = Command::new("osascript")
            .args(["-", OSASCRIPT_TIMEOUT_MS, path_str, basename])
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

    #[cfg(target_os = "macos")]
    #[test]
    fn rendered_script_opens_file_and_dispatches_preview_then_closes_text_tab() {
        let preview = parse("cmd+shift+v").unwrap();
        let script = macos::build_script_for_test(&preview);

        // Opens the file via AppleScript so focus is guaranteed before the keystroke.
        assert!(script.contains(r#"tell application "Zed" to open POSIX file svgPath"#));
        // Active wait by window title — replaces the old fixed `delay 0.2`.
        assert!(script.contains("(name of window 1) contains svgBasename"));
        // Preview keystroke: Cmd+Shift+V → key code 9.
        assert!(script.contains("key code 9 using {command down, shift down}"));
        // Cmd+Shift+[ — activate previous item (the text-mode .svg).
        assert!(script.contains("key code 33 using {command down, shift down}"));
        // Cmd+W — close the now-active text-mode .svg tab.
        assert!(script.contains("key code 13 using {command down}"));
        // No leftover placeholders.
        assert!(!script.contains("__PREVIEW_KEY_CODE__"));
        assert!(!script.contains("__PREVIEW_MODIFIERS__"));
        assert!(!script.contains("__PREV_ITEM__"));
        assert!(!script.contains("__CLOSE_ITEM__"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rendered_script_handles_modifierless_preview_hotkey() {
        let preview = parse("f5").unwrap();
        let script = macos::build_script_for_test(&preview);

        // Preview line: "key code 96 \n" (trailing space + newline, no `using` clause).
        assert!(script.contains("key code 96 \n"));
        // Close-tab chord still fires.
        assert!(script.contains("key code 33 using {command down, shift down}"));
        assert!(script.contains("key code 13 using {command down}"));
    }
}

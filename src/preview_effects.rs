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

#[cfg(target_os = "macos")]
const DEFAULT_HOTKEY: &str = "cmd+shift+v";

/// Pure decision: given an opener and a hotkey spec (and the host OS, implied
/// via cfg), what should `deliver` actually do?
///
/// Splitting the decision out from the execution lets the full
/// `opener × hotkey × OS` matrix be tested without spawning subprocesses or
/// `osascript`.
#[derive(Debug)]
enum Plan {
    /// Spawn `bin path/to/file.svg` and let the editor handle the rest.
    SpawnOpener { bin: String },
    /// macOS-only: open the file via `AppleScript` and synthesise the preview
    /// keystroke chord. `script` is the fully rendered `AppleScript` source.
    #[cfg(target_os = "macos")]
    AppleScript { script: String },
}

fn decide(opener: &Opener, hotkey: &HotkeySpec) -> Plan {
    #[cfg(not(target_os = "macos"))]
    let _ = hotkey;
    #[cfg(target_os = "macos")]
    if opener.synthesise_keystroke()
        && !matches!(hotkey, HotkeySpec::Disabled)
        && let Some(parsed) = macos::resolve(hotkey)
    {
        return Plan::AppleScript {
            script: macos::build_script(&parsed),
        };
    }
    Plan::SpawnOpener {
        bin: opener.command().to_owned(),
    }
}

fn execute(plan: Plan, path: &Path) {
    match plan {
        Plan::SpawnOpener { bin } => {
            if Command::new(&bin).arg(path).spawn().is_err() {
                eprintln!(
                    "svg-react-preview: {bin} not found in PATH; preview saved at: {}",
                    path.display()
                );
            }
        }
        #[cfg(target_os = "macos")]
        Plan::AppleScript { script } => {
            macos::run_with(&script, path, macos::spawn_real);
        }
    }
}

pub fn deliver(path: &Path, opener: &Opener, hotkey: &HotkeySpec) {
    execute(decide(opener, hotkey), path);
}

#[cfg(target_os = "macos")]
struct Hotkey {
    modifiers: Vec<&'static str>,
    key_code: u8,
}

#[cfg(target_os = "macos")]
impl Hotkey {
    fn applescript_clause(&self) -> String {
        if self.modifiers.is_empty() {
            String::new()
        } else {
            format!("using {{{}}}", self.modifiers.join(", "))
        }
    }
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn parse_modifier(token: &str) -> Option<&'static str> {
    match token {
        "cmd" | "command" => Some("command down"),
        "shift" => Some("shift down"),
        "alt" | "opt" | "option" => Some("option down"),
        "ctrl" | "control" => Some("control down"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
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

    pub(super) fn build_script(preview: &Hotkey) -> String {
        SCRIPT_TEMPLATE
            .replace("__PREVIEW_KEY_CODE__", &preview.key_code.to_string())
            .replace("__PREVIEW_MODIFIERS__", &preview.applescript_clause())
            .replace("__PREV_ITEM__", &PREV_ITEM_KEY.to_string())
            .replace("__CLOSE_ITEM__", &CLOSE_ITEM_KEY.to_string())
    }

    pub(super) fn resolve(hotkey: &HotkeySpec) -> Option<Hotkey> {
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

    #[derive(Clone, Copy)]
    pub(super) enum Outcome<'a> {
        Ok,
        Denied(&'a str),
        Other,
    }

    pub(super) fn classify(stdout: &str) -> Outcome<'_> {
        match stdout.trim() {
            "ok" => Outcome::Ok,
            s if s.starts_with("denied:") => {
                Outcome::Denied(s.strip_prefix("denied:").unwrap_or(""))
            }
            _ => Outcome::Other,
        }
    }

    /// Pure rendering of the user-facing message for an outcome.
    ///
    /// Split out from `report` so each match arm is killable by a string-equality
    /// assertion — `report` itself only forwards the result to `eprintln!`.
    pub(super) fn message_for(outcome: Outcome<'_>) -> Option<String> {
        match outcome {
            Outcome::Ok => None,
            Outcome::Denied(num) => Some(format!(
                "svg-react-preview: keystroke blocked by macOS (osascript error {num}) — \
                 grant Accessibility to the app that ran this binary \
                 (System Settings → Privacy & Security → Accessibility). \
                 Press the preview shortcut manually for now."
            )),
            Outcome::Other => Some(
                "svg-react-preview: Zed did not come to the foreground in time. \
                 Press the preview shortcut manually."
                    .into(),
            ),
        }
    }

    fn report(outcome: Outcome<'_>) {
        if let Some(msg) = message_for(outcome) {
            eprintln!("{msg}");
        }
    }

    pub(super) fn run_with(
        script: &str,
        path: &Path,
        spawn: impl FnOnce(&str, &str, &str) -> Option<String>,
    ) {
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
        if let Some(stdout) = spawn(script, path_str, basename) {
            report(classify(&stdout));
        }
    }

    pub(super) fn spawn_real(script: &str, path_str: &str, basename: &str) -> Option<String> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = Command::new("osascript")
            .args(["-", OSASCRIPT_TIMEOUT_MS, path_str, basename])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(script.as_bytes());
        }

        let output = child.wait_with_output().ok()?;
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(all(test, target_os = "macos"))]
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

    /// Pins every named key to its exact macOS Virtual Keycode. Each row kills
    /// the corresponding "delete match arm" mutation in `key_code_for` —
    /// removing an arm makes that lookup fall through to the catch-all `None`,
    /// which trips the matching assertion.
    #[test]
    fn key_code_for_pins_every_named_key() {
        let cases: &[(&str, u8)] = &[
            ("a", 0),
            ("s", 1),
            ("d", 2),
            ("f", 3),
            ("h", 4),
            ("g", 5),
            ("z", 6),
            ("x", 7),
            ("c", 8),
            ("v", 9),
            ("b", 11),
            ("q", 12),
            ("w", 13),
            ("e", 14),
            ("r", 15),
            ("y", 16),
            ("t", 17),
            ("o", 31),
            ("u", 32),
            ("i", 34),
            ("p", 35),
            ("l", 37),
            ("j", 38),
            ("k", 40),
            ("n", 45),
            ("m", 46),
            ("0", 29),
            ("1", 18),
            ("2", 19),
            ("3", 20),
            ("4", 21),
            ("5", 23),
            ("6", 22),
            ("7", 26),
            ("8", 28),
            ("9", 25),
            ("space", 49),
            ("return", 36),
            ("enter", 36),
            ("tab", 48),
            ("escape", 53),
            ("esc", 53),
            ("f1", 122),
            ("f2", 120),
            ("f3", 99),
            ("f4", 118),
            ("f5", 96),
            ("f6", 97),
            ("f7", 98),
            ("f8", 100),
            ("f9", 101),
            ("f10", 109),
            ("f11", 103),
            ("f12", 111),
        ];
        for (name, expected) in cases {
            assert_eq!(
                key_code_for(name),
                Some(*expected),
                "key_code_for({name:?}) should be Some({expected})"
            );
        }
    }

    #[test]
    fn key_code_for_unknown_returns_none() {
        assert!(key_code_for("foobar").is_none());
        assert!(key_code_for("").is_none());
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

    #[test]
    fn resolve_default_returns_default_hotkey() {
        let h = macos::resolve(&HotkeySpec::Default).unwrap();
        assert_eq!(h.key_code, 9);
        assert_eq!(h.modifiers, ["command down", "shift down"]);
    }

    #[test]
    fn resolve_disabled_returns_none() {
        assert!(macos::resolve(&HotkeySpec::Disabled).is_none());
    }

    #[test]
    fn resolve_custom_valid() {
        let h = macos::resolve(&HotkeySpec::Custom("f5".into())).unwrap();
        assert_eq!(h.key_code, 96);
    }

    #[test]
    fn resolve_custom_invalid_returns_none() {
        assert!(macos::resolve(&HotkeySpec::Custom("garbage".into())).is_none());
    }

    #[test]
    fn rendered_script_opens_file_and_dispatches_preview_then_closes_text_tab() {
        let preview = parse("cmd+shift+v").unwrap();
        let script = macos::build_script(&preview);

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

    #[test]
    fn rendered_script_handles_modifierless_preview_hotkey() {
        let preview = parse("f5").unwrap();
        let script = macos::build_script(&preview);

        // Preview line: "key code 96 \n" (trailing space + newline, no `using` clause).
        assert!(script.contains("key code 96 \n"));
        // Close-tab chord still fires.
        assert!(script.contains("key code 33 using {command down, shift down}"));
        assert!(script.contains("key code 13 using {command down}"));
    }

    #[test]
    fn classify_ok() {
        assert!(matches!(macos::classify("ok"), macos::Outcome::Ok));
        assert!(matches!(macos::classify("  ok\n"), macos::Outcome::Ok));
    }

    #[test]
    fn classify_denied_with_error_number() {
        let outcome = macos::classify("denied:-1719");
        assert!(matches!(outcome, macos::Outcome::Denied("-1719")));
    }

    #[test]
    fn classify_denied_empty_payload() {
        // `denied:` without a number still hits the Denied arm (graceful display).
        let outcome = macos::classify("denied:");
        assert!(matches!(outcome, macos::Outcome::Denied("")));
    }

    #[test]
    fn classify_timeout_falls_through_to_other() {
        assert!(matches!(macos::classify("timeout"), macos::Outcome::Other));
    }

    #[test]
    fn classify_unknown_falls_through_to_other() {
        // Anything that is not exactly `ok` or `denied:*` is `Other`.
        assert!(matches!(macos::classify("garbage"), macos::Outcome::Other));
        // Substring `denied:` not at the start should not trip the guard.
        assert!(matches!(
            macos::classify("xdenied:1"),
            macos::Outcome::Other
        ));
        assert!(matches!(macos::classify(""), macos::Outcome::Other));
    }

    #[test]
    fn run_with_invokes_spawner_with_path_and_basename() {
        let path = std::path::PathBuf::from("/tmp/svg-react-preview/abc.svg");
        let mut got: Option<(String, String, String)> = None;
        macos::run_with("SCRIPT", &path, |script, path_str, basename| {
            got = Some((script.into(), path_str.into(), basename.into()));
            Some("ok".into())
        });
        let (script, path_str, basename) = got.expect("spawner must be invoked");
        assert_eq!(script, "SCRIPT");
        assert_eq!(path_str, "/tmp/svg-react-preview/abc.svg");
        assert_eq!(basename, "abc.svg");
    }

    #[test]
    fn run_with_skips_spawner_for_non_utf8_path() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let bytes = [0x66u8, 0xFF, 0x66]; // `f<invalid>f`
        let os = OsStr::from_bytes(&bytes);
        let path = std::path::Path::new(os);
        macos::run_with("SCRIPT", path, |_, _, _| {
            panic!("spawner must not be invoked for non-UTF-8 paths");
        });
    }

    #[test]
    fn run_with_handles_spawn_failure_gracefully() {
        // Spawner returns None (e.g. osascript missing); run_with must not panic
        // and must not call into the reporter (since we have no stdout to classify).
        let path = std::path::PathBuf::from("/tmp/x.svg");
        macos::run_with("SCRIPT", &path, |_, _, _| None);
    }

    #[test]
    fn run_with_falls_back_to_path_str_when_no_basename() {
        // `Path::new("/")` has no `file_name()`; basename must fall back to
        // path_str rather than the empty string (kills `unwrap_or` mutations).
        let path = std::path::PathBuf::from("/");
        let mut got: Option<(String, String)> = None;
        macos::run_with("SCRIPT", &path, |_, path_str, basename| {
            got = Some((path_str.into(), basename.into()));
            Some("ok".into())
        });
        let (path_str, basename) = got.expect("spawner must be invoked");
        assert_eq!(path_str, "/");
        assert_eq!(basename, "/");
    }

    #[test]
    fn message_for_ok_is_silent() {
        assert!(macos::message_for(macos::Outcome::Ok).is_none());
    }

    #[test]
    fn message_for_denied_includes_error_number_and_accessibility_hint() {
        let msg = macos::message_for(macos::Outcome::Denied("-1719"))
            .expect("denied must produce a message");
        assert!(msg.contains("-1719"), "got: {msg}");
        assert!(msg.contains("Accessibility"), "got: {msg}");
        assert!(msg.contains("blocked by macOS"), "got: {msg}");
    }

    #[test]
    fn message_for_other_indicates_focus_timeout() {
        let msg = macos::message_for(macos::Outcome::Other).expect("other must produce a message");
        assert!(msg.contains("foreground"), "got: {msg}");
        assert!(!msg.contains("Accessibility"), "got: {msg}");
    }

    // -- Decision matrix for `decide` -----------------------------------------
    //
    // The full opener × hotkey × OS matrix has 6 reachable cells (custom opener
    // always falls back to SpawnOpener regardless of hotkey, so it collapses to
    // 1; default opener × {Default, Disabled, Custom-valid, Custom-invalid} ×
    // {macOS, non-macOS} fans out to the rest). All cells are covered below
    // without spawning subprocesses.

    #[test]
    fn decide_default_opener_default_hotkey_renders_applescript() {
        let plan = decide(&Opener::Default, &HotkeySpec::Default);
        match plan {
            Plan::AppleScript { script } => {
                // cmd+shift+v → key code 9 with command+shift modifiers.
                assert!(
                    script.contains("key code 9 using {command down, shift down}"),
                    "got: {script}"
                );
            }
            Plan::SpawnOpener { bin } => panic!("expected AppleScript, got SpawnOpener({bin})"),
        }
    }

    #[test]
    fn decide_default_opener_disabled_hotkey_falls_back_to_spawn() {
        let plan = decide(&Opener::Default, &HotkeySpec::Disabled);
        assert!(matches!(plan, Plan::SpawnOpener { ref bin } if bin == "zed"));
    }

    #[test]
    fn decide_default_opener_custom_valid_hotkey_renders_applescript_with_custom_keycode() {
        let plan = decide(&Opener::Default, &HotkeySpec::Custom("f5".into()));
        match plan {
            Plan::AppleScript { script } => {
                // f5 → key code 96, no modifiers → trailing space + newline.
                assert!(script.contains("key code 96 \n"), "got: {script}");
            }
            Plan::SpawnOpener { bin } => panic!("expected AppleScript, got SpawnOpener({bin})"),
        }
    }

    #[test]
    fn decide_default_opener_custom_invalid_hotkey_falls_back_to_spawn() {
        let plan = decide(&Opener::Default, &HotkeySpec::Custom("garbage".into()));
        assert!(matches!(plan, Plan::SpawnOpener { ref bin } if bin == "zed"));
    }

    #[test]
    fn decide_custom_opener_default_hotkey_skips_keystroke() {
        // Custom opener implies "I'm routing this elsewhere" — never synthesise.
        let plan = decide(&Opener::Custom("true".into()), &HotkeySpec::Default);
        assert!(matches!(plan, Plan::SpawnOpener { ref bin } if bin == "true"));
    }

    #[test]
    fn decide_custom_opener_disabled_hotkey_skips_keystroke() {
        let plan = decide(&Opener::Custom("vim".into()), &HotkeySpec::Disabled);
        assert!(matches!(plan, Plan::SpawnOpener { ref bin } if bin == "vim"));
    }
}

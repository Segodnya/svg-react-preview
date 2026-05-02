use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cli() -> Command {
    let mut c = Command::cargo_bin("svg-react-preview").unwrap();
    // Strip any caller-set inputs so each test starts from a known baseline.
    c.env_remove("SVG_REACT_PREVIEW_FILE")
        .env_remove("SVG_REACT_PREVIEW_ROW")
        .env_remove("SVG_REACT_PREVIEW_COLUMN")
        .env_remove("SVG_REACT_PREVIEW_INPUT")
        // Replace the real `zed` launcher with a no-op binary so tests don't pop a window.
        .env("SVG_REACT_PREVIEW_OPENER", "true");
    c
}

#[test]
fn stdin_input_succeeds() {
    cli()
        .write_stdin(r#"<svg><path d="M0 0"/></svg>"#)
        .assert()
        .success();
}

#[test]
fn env_input_overrides_stdin() {
    cli()
        .env("SVG_REACT_PREVIEW_INPUT", r#"<svg><path d="M0"/></svg>"#)
        .write_stdin("garbage that should not be parsed")
        .assert()
        .success();
}

#[test]
fn empty_stdin_fails_with_message() {
    cli()
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty input"));
}

#[test]
fn invalid_jsx_fails() {
    cli()
        .write_stdin("<svg><path>")
        .assert()
        .failure()
        .stderr(predicate::str::contains("svg-react-preview:"));
}

#[test]
fn warnings_printed_to_stderr() {
    cli()
        .write_stdin("<Icon/>")
        .assert()
        .success()
        .stderr(predicate::str::contains("warn:"));
}

#[test]
fn missing_opener_prints_path_fallback() {
    cli()
        .env(
            "SVG_REACT_PREVIEW_OPENER",
            "definitely-not-a-real-binary-xyz",
        )
        .write_stdin(r#"<svg><path/></svg>"#)
        .assert()
        .success()
        .stderr(predicate::str::contains("not found in PATH"));
}

#[test]
fn file_row_col_input_finds_svg() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "const x = <svg><path d=\"M0\"/></svg>;").unwrap();
    cli()
        .env("SVG_REACT_PREVIEW_FILE", tmp.path())
        .env("SVG_REACT_PREVIEW_ROW", "1")
        .env("SVG_REACT_PREVIEW_COLUMN", "13")
        .assert()
        .success();
}

#[test]
fn missing_file_errors() {
    cli()
        .env("SVG_REACT_PREVIEW_FILE", "/nonexistent/path/file.tsx")
        .env("SVG_REACT_PREVIEW_ROW", "1")
        .env("SVG_REACT_PREVIEW_COLUMN", "1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot read"));
}

#[test]
fn cursor_outside_svg_errors() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "const x = 42;").unwrap();
    cli()
        .env("SVG_REACT_PREVIEW_FILE", tmp.path())
        .env("SVG_REACT_PREVIEW_ROW", "1")
        .env("SVG_REACT_PREVIEW_COLUMN", "1")
        .assert()
        .failure();
}

#[test]
fn empty_env_var_falls_through_to_stdin() {
    // Empty SVG_REACT_PREVIEW_INPUT should be treated as unset.
    cli()
        .env("SVG_REACT_PREVIEW_INPUT", "")
        .write_stdin(r#"<svg><path/></svg>"#)
        .assert()
        .success();
}

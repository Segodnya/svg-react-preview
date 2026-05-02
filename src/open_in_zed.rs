use std::path::Path;
use std::process::Command;

pub fn open(path: &Path) {
    // Allow tests / advanced users to override the launched binary (must be on PATH).
    let bin = std::env::var("SVG_REACT_PREVIEW_OPENER")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "zed".into());
    if Command::new(&bin).arg(path).spawn().is_err() {
        eprintln!(
            "svg-react-preview: {} not found in PATH; preview saved at: {}",
            bin,
            path.display()
        );
    }
}

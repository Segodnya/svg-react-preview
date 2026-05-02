use std::path::Path;
use std::process::Command;

pub fn open(path: &Path) {
    if Command::new("zed").arg(path).spawn().is_err() {
        eprintln!(
            "svg-react-preview: zed not found in PATH; preview saved at: {}",
            path.display()
        );
    }
}

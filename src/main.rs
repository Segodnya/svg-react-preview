use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use svg_react_preview::{serialize, transform};

mod config;
mod preview_effects;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("svg-react-preview: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cfg = config::Config::from_env()?;
    let expr = cfg.source.into_expr()?;
    let result = transform::to_svg(&expr)?;

    for w in &result.warnings {
        eprintln!("svg-react-preview: warn: {w}");
    }

    let xml = serialize::to_xml(&result.root);
    let path = write_temp(&xml)?;
    preview_effects::deliver(&path, &cfg.opener, &cfg.hotkey);
    Ok(())
}

fn write_temp(xml: &str) -> Result<PathBuf> {
    let dir = env::temp_dir().join("svg-react-preview");
    fs::create_dir_all(&dir)?;
    let mut hasher = DefaultHasher::new();
    hasher.write(xml.as_bytes());
    let path = dir.join(format!("{:016x}.svg", hasher.finish()));
    fs::write(&path, xml)?;
    Ok(path)
}

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Result};
use svg_react_preview::{parse, serialize, transform};
use twox_hash::XxHash64;

mod open_in_zed;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("svg-react-preview: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let input = read_input()?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "empty input (set $SVG_REACT_PREVIEW_INPUT or pipe via stdin)"
        ));
    }

    let expr = parse::parse_jsx(trimmed)?;
    let result = transform::to_svg(&expr)?;

    for w in &result.warnings {
        eprintln!("svg-react-preview: warn: {}", w);
    }

    let xml = serialize::to_xml(&result.root);
    let path = write_temp(trimmed, &xml)?;
    open_in_zed::open(&path);
    Ok(())
}

fn read_input() -> Result<String> {
    if let Ok(v) = env::var("SVG_REACT_PREVIEW_INPUT") {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn write_temp(source: &str, xml: &str) -> Result<PathBuf> {
    let dir = env::temp_dir().join("svg-react-preview");
    fs::create_dir_all(&dir)?;
    let hash = XxHash64::oneshot(0, source.as_bytes());
    let path = dir.join(format!("{:016x}.svg", hash));
    fs::write(&path, xml)?;
    Ok(path)
}

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Result};
use svg_react_preview::{expand_selection, parse, serialize, transform};
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
            "empty input (place cursor inside <svg> with $SVG_REACT_PREVIEW_FILE/ROW/COLUMN, set $SVG_REACT_PREVIEW_INPUT, or pipe via stdin)"
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
    let file = non_empty_env("SVG_REACT_PREVIEW_FILE");
    let row = non_empty_env("SVG_REACT_PREVIEW_ROW").and_then(|s| s.parse::<usize>().ok());
    let col = non_empty_env("SVG_REACT_PREVIEW_COLUMN").and_then(|s| s.parse::<usize>().ok());

    if let (Some(file), Some(row), Some(col)) = (file, row, col) {
        let source =
            fs::read_to_string(&file).map_err(|e| anyhow!("cannot read {}: {}", file, e))?;
        return expand_selection::find_svg_at(&source, row, col)
            .map_err(|e| anyhow!("{} [{}:{}:{}]", e, file, row, col));
    }

    if let Some(v) = non_empty_env("SVG_REACT_PREVIEW_INPUT") {
        return Ok(v);
    }

    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

fn write_temp(source: &str, xml: &str) -> Result<PathBuf> {
    let dir = env::temp_dir().join("svg-react-preview");
    fs::create_dir_all(&dir)?;
    let hash = XxHash64::oneshot(0, source.as_bytes());
    let path = dir.join(format!("{:016x}.svg", hash));
    fs::write(&path, xml)?;
    Ok(path)
}

use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, anyhow};
use svg_react_preview::source::Source;
use svg_react_preview::{serialize, transform};

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
    let expr = select_source()?.into_expr()?;
    let result = transform::to_svg(&expr)?;

    for w in &result.warnings {
        eprintln!("svg-react-preview: warn: {}", w);
    }

    let xml = serialize::to_xml(&result.root);
    let path = write_temp(&xml)?;
    open_in_zed::open(&path);
    Ok(())
}

/// Resolves the input mode from environment + stdin per the precedence documented
/// in the README: `FILE`+`ROW`+`COLUMN` → `INPUT` → stdin.
fn select_source() -> Result<Source> {
    let file = non_empty_env("SVG_REACT_PREVIEW_FILE");
    let row = non_empty_env("SVG_REACT_PREVIEW_ROW").and_then(|s| s.parse::<usize>().ok());
    let col = non_empty_env("SVG_REACT_PREVIEW_COLUMN").and_then(|s| s.parse::<usize>().ok());

    if let (Some(path), Some(row), Some(col)) = (file, row, col) {
        let source =
            fs::read_to_string(&path).map_err(|e| anyhow!("cannot read {}: {}", path, e))?;
        return Ok(Source::Cursor {
            source,
            path,
            row,
            col,
        });
    }

    if let Some(v) = non_empty_env("SVG_REACT_PREVIEW_INPUT") {
        return Ok(Source::Fragment(v));
    }

    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    Ok(Source::Fragment(buf))
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
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

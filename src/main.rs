use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Result};
use svg_react_preview::{expand_selection, parse, serialize, transform};
use swc_ecma_ast::Expr;

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
    let expr = read_input()?;
    let result = transform::to_svg(&expr)?;

    for w in &result.warnings {
        eprintln!("svg-react-preview: warn: {}", w);
    }

    let xml = serialize::to_xml(&result.root);
    let path = write_temp(&xml)?;
    open_in_zed::open(&path);
    Ok(())
}

fn read_input() -> Result<Box<Expr>> {
    let file = non_empty_env("SVG_REACT_PREVIEW_FILE");
    let row = non_empty_env("SVG_REACT_PREVIEW_ROW").and_then(|s| s.parse::<usize>().ok());
    let col = non_empty_env("SVG_REACT_PREVIEW_COLUMN").and_then(|s| s.parse::<usize>().ok());

    if let (Some(file), Some(row), Some(col)) = (file, row, col) {
        let source =
            fs::read_to_string(&file).map_err(|e| anyhow!("cannot read {}: {}", file, e))?;
        let element = expand_selection::find_svg_at(&source, row, col)
            .map_err(|e| anyhow!("{} [{}:{}:{}]", e, file, row, col))?;
        return Ok(Box::new(Expr::JSXElement(element)));
    }

    if let Some(v) = non_empty_env("SVG_REACT_PREVIEW_INPUT") {
        return parse_fragment(&v);
    }

    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    parse_fragment(&buf)
}

fn parse_fragment(input: &str) -> Result<Box<Expr>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "empty input (place cursor inside <svg> with $SVG_REACT_PREVIEW_FILE/ROW/COLUMN, set $SVG_REACT_PREVIEW_INPUT, or pipe via stdin)"
        ));
    }
    parse::parse_jsx(trimmed)
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

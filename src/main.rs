use std::process::ExitCode;

use anyhow::Result;
use svg_react_preview::pipeline;

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
    let rendered = pipeline::render(cfg.source)?;

    for w in &rendered.warnings {
        eprintln!("svg-react-preview: warn: {w}");
    }

    let path = pipeline::write_to_temp_dir(&rendered.xml)?;
    preview_effects::deliver(&path, &cfg.opener, &cfg.hotkey);
    Ok(())
}

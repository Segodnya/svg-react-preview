//! End-to-end JSX-fragment → SVG-on-disk pipeline.
//!
//! `render` collapses the four-step chain (`Source::into_expr` →
//! `transform::to_svg` → collect warnings → `serialize::to_xml`) behind one call,
//! so callers do not have to know the order or the intermediate representations.
//! `write_to_temp_dir` handles the side-effecting half: the rendered XML lands in
//! `$TMPDIR/svg-react-preview/<hash>.svg`, and the path is returned for downstream
//! delivery (editor open + macOS keystroke synthesis).

use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::PathBuf;

use anyhow::Result;

use crate::serialize::to_xml;
use crate::source::Source;
use crate::transform::to_svg;

/// Result of [`render`]. `xml` is a valid stand-alone SVG document; `warnings`
/// are non-fatal advisories the caller may surface to the user (we route them
/// to stderr in the binary).
#[derive(Debug)]
pub struct RenderedSvg {
    pub xml: String,
    pub warnings: Vec<String>,
}

/// Render a [`Source`] into stand-alone SVG XML.
///
/// # Errors
/// - `Source::Fragment` errors if the input is empty or fails to parse as TSX.
/// - `Source::Cursor` errors if the cursor is out of range, the file is not
///   parseable, or no `<svg>` element encloses the cursor.
/// - The transform stage errors if the parsed expression contains no JSX
///   elements at all (e.g. a bare numeric literal).
pub fn render(source: Source) -> Result<RenderedSvg> {
    let expr = source.into_expr()?;
    let result = to_svg(&expr)?;
    let xml = to_xml(&result.root);
    Ok(RenderedSvg {
        xml,
        warnings: result.warnings,
    })
}

/// Writes `xml` into `$TMPDIR/svg-react-preview/<xxhash>.svg` and returns the path.
///
/// Re-rendering the same fragment yields the same hash and overwrites the
/// existing file, which keeps the directory bounded for repeated previews.
///
/// # Errors
/// I/O errors creating the directory or writing the file.
pub fn write_to_temp_dir(xml: &str) -> Result<PathBuf> {
    write_xml_to_dir(xml, &env::temp_dir().join("svg-react-preview"))
}

/// Writes `xml` into `dir/<xxhash>.svg` (creating `dir` if needed) and returns
/// the path. Pure seam factored out of [`write_to_temp_dir`] so unit tests can
/// inject a sandbox directory without mutating process-global env vars.
///
/// # Errors
/// I/O errors creating the directory or writing the file.
fn write_xml_to_dir(xml: &str, dir: &std::path::Path) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let mut hasher = DefaultHasher::new();
    hasher.write(xml.as_bytes());
    let path = dir.join(format!("{:016x}.svg", hasher.finish()));
    fs::write(&path, xml)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_fragment(tsx: &str) -> RenderedSvg {
        render(Source::Fragment(tsx.into())).expect("render must succeed")
    }

    #[test]
    fn renders_basic_svg_to_xml() {
        let r = render_fragment(r#"<svg><path d="M0 0"/></svg>"#);
        assert!(r.xml.starts_with("<svg"), "got: {}", r.xml);
        assert!(r.xml.contains("<path"), "got: {}", r.xml);
        assert!(r.warnings.is_empty(), "got warnings: {:?}", r.warnings);
    }

    #[test]
    fn collects_pascal_case_warning() {
        let r = render_fragment("<Icon/>");
        assert!(
            r.warnings.iter().any(|w| w.contains("placeholder")),
            "warnings: {:?}",
            r.warnings
        );
    }

    #[test]
    fn empty_fragment_errors() {
        let err = render(Source::Fragment(String::new()))
            .unwrap_err()
            .to_string();
        assert!(err.contains("empty input"), "got: {err}");
    }

    #[test]
    fn cursor_outside_svg_errors() {
        let err = render(Source::Cursor {
            source: "const x = 42;".into(),
            path: "input.tsx".into(),
            row: 1,
            col: 1,
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("not inside an <svg>"), "got: {err}");
    }

    #[test]
    fn cursor_inside_svg_renders_innermost() {
        let r = render(Source::Cursor {
            source: "const x = <svg><path d=\"M1\"/></svg>;".into(),
            path: "input.tsx".into(),
            row: 1,
            col: 17,
        })
        .expect("render must succeed");
        assert!(r.xml.contains("<path"), "got: {}", r.xml);
    }

    #[test]
    fn write_xml_to_dir_creates_file_with_xml() {
        let tmp = tempfile::tempdir().unwrap();
        let xml = "<svg><path/></svg>";
        let path = write_xml_to_dir(xml, tmp.path()).expect("write must succeed");
        assert!(path.starts_with(tmp.path()), "got: {path:?}");
        assert_eq!(path.extension().and_then(|s| s.to_str()), Some("svg"));
        assert_eq!(fs::read_to_string(&path).unwrap(), xml);
    }

    #[test]
    fn write_xml_to_dir_is_stable_for_identical_xml() {
        let tmp = tempfile::tempdir().unwrap();
        let xml = "<svg><circle r=\"5\"/></svg>";
        let a = write_xml_to_dir(xml, tmp.path()).unwrap();
        let b = write_xml_to_dir(xml, tmp.path()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn write_xml_to_dir_creates_missing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested").join("svg-react-preview");
        let path = write_xml_to_dir("<svg/>", &nested).expect("write must succeed");
        assert!(path.starts_with(&nested), "got: {path:?}");
    }
}

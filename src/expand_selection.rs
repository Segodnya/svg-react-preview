use anyhow::{anyhow, Result};
use swc_common::{sync::Lrc, FileName, SourceMap, Spanned};
use swc_ecma_ast::{EsVersion, JSXElement, JSXElementName};
use swc_ecma_parser::{parse_file_as_module, Syntax, TsSyntax};
use swc_ecma_visit::{Visit, VisitWith};

/// Returns the source code of the innermost `<svg>` element enclosing (row, col).
/// `row` and `col` are 1-based, matching Zed's `${ZED_ROW}` / `${ZED_COLUMN}`.
pub fn find_svg_at(source: &str, row: usize, col: usize) -> Result<String> {
    let offset = row_col_to_offset(source, row, col)
        .ok_or_else(|| anyhow!("cursor position {}:{} is past end of file", row, col))?;

    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("input.tsx".into()).into(),
        source.to_string(),
    );
    let syntax = Syntax::Typescript(TsSyntax {
        tsx: true,
        decorators: false,
        dts: false,
        no_early_errors: true,
        disallow_ambiguous_jsx_like: false,
    });

    let mut recovered = Vec::new();
    let module = parse_file_as_module(&fm, syntax, EsVersion::EsNext, None, &mut recovered)
        .map_err(|e| {
            anyhow!(
                "file is not parseable as TSX: {:?} (try $SVG_REACT_PREVIEW_INPUT or stdin instead)",
                e.kind()
            )
        })?;

    let start_pos = fm.start_pos.0 as usize;
    let target = start_pos + offset;

    let mut finder = SvgFinder { target, hit: None };
    module.visit_with(&mut finder);

    let (lo, hi) = finder
        .hit
        .ok_or_else(|| anyhow!("cursor at {}:{} is not inside an <svg> element", row, col))?;
    let from = lo - start_pos;
    let to = hi - start_pos;
    Ok(source[from..to].to_string())
}

/// Converts 1-based (row, col) into a byte offset, counting columns in Unicode characters.
/// Handles both `\n` and `\r\n` line endings (CR is not counted as a separate column).
fn row_col_to_offset(source: &str, row: usize, col: usize) -> Option<usize> {
    if row == 0 || col == 0 {
        return None;
    }

    let mut line_start = 0usize;
    let mut current = 1;
    while current < row {
        let rest = &source[line_start..];
        match rest.find('\n') {
            Some(nl) => {
                line_start += nl + 1;
                current += 1;
            }
            None => return None,
        }
    }

    // Walk col-1 characters along the line. Hitting \n or EOF early means the column is past the line.
    let line_slice = &source[line_start..];
    let mut iter = line_slice.char_indices();
    let mut byte_in_line = 0usize;
    for _ in 0..(col - 1) {
        match iter.next() {
            Some((_, '\n')) => return None,
            Some((i, c)) => byte_in_line = i + c.len_utf8(),
            None => return None,
        }
    }
    Some(line_start + byte_in_line)
}

struct SvgFinder {
    target: usize,
    /// (lo, hi): absolute swc BytePos of the narrowest enclosing <svg>.
    hit: Option<(usize, usize)>,
}

impl Visit for SvgFinder {
    fn visit_jsx_element(&mut self, el: &JSXElement) {
        let span = el.span();
        let lo = span.lo.0 as usize;
        let hi = span.hi.0 as usize;

        let is_svg = matches!(
            &el.opening.name,
            JSXElementName::Ident(id) if id.sym.as_str() == "svg"
        );

        if is_svg && lo <= self.target && self.target < hi {
            let narrower = match self.hit {
                None => true,
                Some((clo, chi)) => (hi - lo) < (chi - clo),
            };
            if narrower {
                self.hit = Some((lo, hi));
            }
        }

        el.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locates the `|` marker in the source, strips it, and returns the cleaned source + (row, col).
    fn split_marker(src: &str) -> (String, usize, usize) {
        let idx = src.find('|').expect("test source must contain `|` marker");
        let before = &src[..idx];
        let after = &src[idx + 1..];
        let row = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let last_nl = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = before[last_nl..].chars().count() + 1;
        (format!("{}{}", before, after), row, col)
    }

    fn run(src_with_marker: &str) -> Result<String> {
        let (src, row, col) = split_marker(src_with_marker);
        find_svg_at(&src, row, col)
    }

    #[test]
    fn cursor_in_middle_of_svg() {
        let out = run("export const Icon = () => (<svg><pa|th d=\"M0 0\"/></svg>);").unwrap();
        assert_eq!(out, r#"<svg><path d="M0 0"/></svg>"#);
    }

    #[test]
    fn cursor_on_opening_bracket() {
        let out = run("const x = |<svg><path/></svg>;").unwrap();
        assert_eq!(out, "<svg><path/></svg>");
    }

    #[test]
    fn cursor_inside_attributes() {
        let out = run("const x = <svg width|={24} height={24}><path/></svg>;").unwrap();
        assert_eq!(out, "<svg width={24} height={24}><path/></svg>");
    }

    #[test]
    fn self_closing_svg() {
        let out = run("const x = <svg width={24|} />;").unwrap();
        assert_eq!(out, "<svg width={24} />");
    }

    #[test]
    fn nested_svg_picks_innermost() {
        let src = "const x = <svg><svg><pa|th/></svg></svg>;";
        let out = run(src).unwrap();
        assert_eq!(out, "<svg><path/></svg>");
    }

    #[test]
    fn picks_correct_one_among_siblings() {
        let src = "const arr = [<svg><a/></svg>, <svg><b|/></svg>];";
        let out = run(src).unwrap();
        assert_eq!(out, "<svg><b/></svg>");
    }

    #[test]
    fn cursor_outside_any_svg_errors() {
        let src = "const x = <div>he|llo</div>;";
        let err = run(src).unwrap_err().to_string();
        assert!(err.contains("not inside an <svg>"), "got: {err}");
    }

    #[test]
    fn cursor_in_imports_errors() {
        let src = "import Re|act from 'react';\nconst x = <svg/>;";
        let err = run(src).unwrap_err().to_string();
        assert!(err.contains("not inside an <svg>"), "got: {err}");
    }

    #[test]
    fn row_col_past_end_errors() {
        let err = find_svg_at("const x = <svg/>;", 99, 1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("past end of file"), "got: {err}");
    }

    #[test]
    fn handles_utf8_above_cursor() {
        // Cyrillic in a comment before the SVG: byte offset != char offset on that line.
        let src = "// комментарий\nconst x = <svg><pa|th/></svg>;";
        let out = run(src).unwrap();
        assert_eq!(out, "<svg><path/></svg>");
    }

    #[test]
    fn handles_crlf() {
        let src = "// hello\r\nconst x = <svg><pa|th/></svg>;";
        let out = run(src).unwrap();
        assert_eq!(out, "<svg><path/></svg>");
    }

    #[test]
    fn multiline_svg() {
        let src = "const x = (\n  <svg viewBox=\"0 0 24 24\">\n    <path d=\"M0|\"/>\n  </svg>\n);";
        let out = run(src).unwrap();
        assert_eq!(
            out,
            "<svg viewBox=\"0 0 24 24\">\n    <path d=\"M0\"/>\n  </svg>"
        );
    }

    #[test]
    fn unparseable_file_errors() {
        // Plain XML without TSX scaffolding — swc cannot parse it as a module.
        let err = find_svg_at("<svg><path/>", 1, 7).unwrap_err().to_string();
        assert!(err.contains("not parseable as TSX"), "got: {err}");
    }
}

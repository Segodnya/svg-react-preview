use anyhow::{Result, anyhow};
use swc_common::{FileName, SourceMap, Spanned, sync::Lrc};
use swc_ecma_ast::{EsVersion, JSXElement, JSXElementName};
use swc_ecma_parser::parse_file_as_module;
use swc_ecma_visit::{Visit, VisitWith};

use crate::parse::tsx_syntax;

/// Returns the innermost `<svg>` element enclosing (row, col) as a parsed AST node.
/// `row` and `col` are 1-based, matching Zed's `${ZED_ROW}` / `${ZED_COLUMN}`.
pub fn find_svg_at(source: &str, row: usize, col: usize) -> Result<Box<JSXElement>> {
    let offset = row_col_to_offset(source, row, col)
        .ok_or_else(|| anyhow!("cursor position {}:{} is past end of file", row, col))?;

    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("input.tsx".into()).into(),
        source.to_string(),
    );

    let mut recovered = Vec::new();
    let module = parse_file_as_module(&fm, tsx_syntax(), EsVersion::EsNext, None, &mut recovered)
        .map_err(|e| {
        anyhow!(
            "file is not parseable as TSX: {:?} (try $SVG_REACT_PREVIEW_INPUT or stdin instead)",
            e.kind()
        )
    })?;

    let target = fm.start_pos.0 as usize + offset;
    let mut finder = SvgFinder { target, hit: None };
    module.visit_with(&mut finder);

    finder
        .hit
        .map(|(_, el)| el)
        .ok_or_else(|| anyhow!("cursor at {}:{} is not inside an <svg> element", row, col))
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
    /// (span size, cloned element): smallest enclosing <svg>.
    hit: Option<(usize, Box<JSXElement>)>,
}

impl Visit for SvgFinder {
    fn visit_jsx_element(&mut self, el: &JSXElement) {
        let span = el.span();
        let lo = span.lo.0 as usize;
        let hi = span.hi.0 as usize;
        let size = hi - lo;

        let is_svg = matches!(
            &el.opening.name,
            JSXElementName::Ident(id) if id.sym.as_str() == "svg"
        );

        if is_svg && lo <= self.target && self.target < hi {
            let narrower = self.hit.as_ref().is_none_or(|(s, _)| size < *s);
            if narrower {
                self.hit = Some((size, Box::new(el.clone())));
            }
        }

        el.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{serialize, transform};
    use swc_ecma_ast::Expr;

    /// Locates the `|` marker, strips it, runs the full pipeline on the found SVG.
    fn render(src_with_marker: &str) -> Result<String> {
        let idx = src_with_marker
            .find('|')
            .expect("test source must contain `|` marker");
        let before = &src_with_marker[..idx];
        let after = &src_with_marker[idx + 1..];
        let row = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let last_nl = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = before[last_nl..].chars().count() + 1;
        let src = format!("{}{}", before, after);

        let element = find_svg_at(&src, row, col)?;
        let expr = Expr::JSXElement(element);
        let res = transform::to_svg(&expr)?;
        Ok(serialize::to_xml(&res.root))
    }

    const XMLNS: &str = "http://www.w3.org/2000/svg";

    #[test]
    fn cursor_in_middle_of_svg() {
        let out = render("export const Icon = () => (<svg><pa|th d=\"M0 0\"/></svg>);").unwrap();
        assert_eq!(
            out,
            format!(r#"<svg xmlns="{XMLNS}"><path d="M0 0"/></svg>"#)
        );
    }

    #[test]
    fn cursor_on_opening_bracket() {
        let out = render("const x = |<svg><path/></svg>;").unwrap();
        assert_eq!(out, format!(r#"<svg xmlns="{XMLNS}"><path/></svg>"#));
    }

    #[test]
    fn cursor_inside_attributes() {
        let out = render("const x = <svg width|={24} height={24}><path/></svg>;").unwrap();
        assert_eq!(
            out,
            format!(r#"<svg xmlns="{XMLNS}" width="24" height="24"><path/></svg>"#)
        );
    }

    #[test]
    fn self_closing_svg() {
        let out = render("const x = <svg width={24|} />;").unwrap();
        assert_eq!(out, format!(r#"<svg xmlns="{XMLNS}" width="24"></svg>"#));
    }

    #[test]
    fn nested_svg_picks_innermost() {
        let out = render("const x = <svg><svg><pa|th/></svg></svg>;").unwrap();
        assert_eq!(out, format!(r#"<svg xmlns="{XMLNS}"><path/></svg>"#));
    }

    #[test]
    fn picks_correct_one_among_siblings() {
        let out = render("const arr = [<svg><a/></svg>, <svg><b|/></svg>];").unwrap();
        assert_eq!(out, format!(r#"<svg xmlns="{XMLNS}"><b></b></svg>"#));
    }

    #[test]
    fn cursor_outside_any_svg_errors() {
        let err = render("const x = <div>he|llo</div>;")
            .unwrap_err()
            .to_string();
        assert!(err.contains("not inside an <svg>"), "got: {err}");
    }

    #[test]
    fn cursor_in_imports_errors() {
        let err = render("import Re|act from 'react';\nconst x = <svg/>;")
            .unwrap_err()
            .to_string();
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
        let out = render("// комментарий\nconst x = <svg><pa|th/></svg>;").unwrap();
        assert_eq!(out, format!(r#"<svg xmlns="{XMLNS}"><path/></svg>"#));
    }

    #[test]
    fn handles_crlf() {
        let out = render("// hello\r\nconst x = <svg><pa|th/></svg>;").unwrap();
        assert_eq!(out, format!(r#"<svg xmlns="{XMLNS}"><path/></svg>"#));
    }

    #[test]
    fn multiline_svg() {
        let out = render(
            "const x = (\n  <svg viewBox=\"0 0 24 24\">\n    <path d=\"M0|\"/>\n  </svg>\n);",
        )
        .unwrap();
        assert_eq!(
            out,
            format!(r#"<svg xmlns="{XMLNS}" viewBox="0 0 24 24"><path d="M0"/></svg>"#)
        );
    }

    #[test]
    fn unparseable_file_errors() {
        let err = find_svg_at("<svg><path/>", 1, 7).unwrap_err().to_string();
        assert!(err.contains("not parseable as TSX"), "got: {err}");
    }

    #[test]
    fn row_zero_is_invalid() {
        let err = find_svg_at("const x = <svg/>;", 0, 1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("past end of file"), "got: {err}");
    }

    #[test]
    fn col_zero_is_invalid() {
        let err = find_svg_at("const x = <svg/>;", 1, 0)
            .unwrap_err()
            .to_string();
        assert!(err.contains("past end of file"), "got: {err}");
    }

    #[test]
    fn col_past_line_via_newline_is_invalid() {
        // Line 1 has 2 characters; column 5 walks past the newline.
        let err = find_svg_at("ab\ncd", 1, 5).unwrap_err().to_string();
        assert!(err.contains("past end of file"), "got: {err}");
    }

    #[test]
    fn col_past_eof_without_newline_is_invalid() {
        let err = find_svg_at("x", 1, 100).unwrap_err().to_string();
        assert!(err.contains("past end of file"), "got: {err}");
    }
}

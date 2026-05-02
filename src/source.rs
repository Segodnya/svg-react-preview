use anyhow::{Result, anyhow};
use swc_ecma_ast::Expr;

use crate::expand_selection::find_svg_at;
use crate::parse::parse_jsx;

/// One of the input modes documented in the README. Hides the asymmetry between
/// "parse a free-standing TSX expression" and "find the innermost `<svg>` enclosing
/// a cursor position in a full TSX file" behind a single `into_expr` operation.
pub enum Source {
    /// A free-standing TSX expression (e.g. piped via stdin or `$SVG_REACT_PREVIEW_INPUT`).
    Fragment(String),
    /// A full TSX file plus a 1-based cursor position. `path` is carried for error
    /// messages only — the source is read by the caller.
    Cursor {
        source: String,
        path: String,
        row: usize,
        col: usize,
    },
}

impl Source {
    pub fn into_expr(self) -> Result<Box<Expr>> {
        match self {
            Source::Fragment(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Err(anyhow!(
                        "empty input (place cursor inside <svg> with $SVG_REACT_PREVIEW_FILE/ROW/COLUMN, set $SVG_REACT_PREVIEW_INPUT, or pipe via stdin)"
                    ));
                }
                parse_jsx(trimmed)
            }
            Source::Cursor {
                source,
                path,
                row,
                col,
            } => {
                let element = find_svg_at(&source, row, col)
                    .map_err(|e| anyhow!("{} [{}:{}:{}]", e, path, row, col))?;
                Ok(Box::new(Expr::JSXElement(element)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_empty_errors() {
        let err = Source::Fragment(String::new())
            .into_expr()
            .unwrap_err()
            .to_string();
        assert!(err.contains("empty input"), "got: {err}");
    }

    #[test]
    fn fragment_whitespace_only_errors() {
        let err = Source::Fragment("   \n\t".into())
            .into_expr()
            .unwrap_err()
            .to_string();
        assert!(err.contains("empty input"), "got: {err}");
    }

    #[test]
    fn fragment_valid_jsx_parses() {
        let expr = Source::Fragment(r#"<svg><path d="M0"/></svg>"#.into())
            .into_expr()
            .unwrap();
        assert!(matches!(*expr, Expr::JSXElement(_)));
    }

    #[test]
    fn cursor_decorates_error_with_path() {
        let err = Source::Cursor {
            source: "const x = <div>hello</div>;".into(),
            path: "/tmp/example.tsx".into(),
            row: 1,
            col: 16,
        }
        .into_expr()
        .unwrap_err()
        .to_string();
        assert!(err.contains("/tmp/example.tsx:1:16"), "got: {err}");
        assert!(err.contains("not inside an <svg>"), "got: {err}");
    }

    #[test]
    fn cursor_inside_svg_returns_jsx_element_expr() {
        let expr = Source::Cursor {
            source: "const x = <svg><path/></svg>;".into(),
            path: "input.tsx".into(),
            row: 1,
            col: 17,
        }
        .into_expr()
        .unwrap();
        assert!(matches!(*expr, Expr::JSXElement(_)));
    }
}

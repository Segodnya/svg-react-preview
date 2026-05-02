use anyhow::{Result, anyhow};
use swc_common::{FileName, SourceFile, SourceMap, sync::Lrc};
use swc_ecma_ast::{EsVersion, Expr};
use swc_ecma_parser::{Syntax, TsSyntax, parse_file_as_expr};

/// Shared TSX parser configuration used by `parse_jsx` and `expand_selection`.
#[must_use]
pub const fn tsx_syntax() -> Syntax {
    Syntax::Typescript(TsSyntax {
        tsx: true,
        decorators: false,
        dts: false,
        no_early_errors: true,
        disallow_ambiguous_jsx_like: false,
    })
}

/// Builds the swc `SourceMap` + `SourceFile` pair every parser in this crate needs.
/// The map is returned alongside the file because callers that compute byte offsets
/// (see `expand_selection`) need `fm.start_pos`.
pub(crate) fn make_source_file(input: &str) -> (Lrc<SourceMap>, Lrc<SourceFile>) {
    let cm: Lrc<SourceMap> = Lrc::default();
    let fm = cm.new_source_file(
        FileName::Custom("input.tsx".into()).into(),
        input.to_string(),
    );
    (cm, fm)
}

/// Parses an arbitrary TSX fragment as a single expression.
/// Accepts `<svg>…</svg>`, `<>…</>`, parenthesised `(…)` — anything valid as an Expression.
///
/// # Errors
/// Returns the swc parse error if the input is not a valid TSX expression.
pub fn parse_jsx(input: &str) -> Result<Box<Expr>> {
    let (_cm, fm) = make_source_file(input);
    let mut recovered = Vec::new();
    parse_file_as_expr(&fm, tsx_syntax(), EsVersion::EsNext, None, &mut recovered)
        .map_err(|e| anyhow!("JSX parse error: {:?}", e.kind()))
}

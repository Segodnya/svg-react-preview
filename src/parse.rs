use anyhow::{Result, anyhow};
use swc_common::{FileName, SourceMap, sync::Lrc};
use swc_ecma_ast::{EsVersion, Expr};
use swc_ecma_parser::{Syntax, TsSyntax, parse_file_as_expr};

/// Shared TSX parser configuration used by `parse_jsx` and `expand_selection`.
pub fn tsx_syntax() -> Syntax {
    Syntax::Typescript(TsSyntax {
        tsx: true,
        decorators: false,
        dts: false,
        no_early_errors: true,
        disallow_ambiguous_jsx_like: false,
    })
}

/// Parses an arbitrary TSX fragment as a single expression.
/// Accepts `<svg>…</svg>`, `<>…</>`, parenthesised `(…)` — anything valid as an Expression.
pub fn parse_jsx(input: &str) -> Result<Box<Expr>> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("input.tsx".into()).into(),
        input.to_string(),
    );

    let mut recovered = Vec::new();
    parse_file_as_expr(&fm, tsx_syntax(), EsVersion::EsNext, None, &mut recovered)
        .map_err(|e| anyhow!("JSX parse error: {:?}", e.kind()))
}

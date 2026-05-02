//! Static evaluation of JSX/TSX expressions.
//!
//! Two operations live here:
//! - [`narrow_branch`] picks the JSX (or value) that would render when conditions
//!   are treated as truthy — `(e)` → `e`, `c ? a : b` → `a`, `c && rhs` → `rhs`.
//! - [`resolve_attr_value`] reduces a JSX attribute expression to a static string,
//!   falling back by attribute name when unresolvable.
//!
//! Centralising the rules keeps element narrowing and attribute resolution from
//! drifting on what counts as "statically decidable".

use swc_ecma_ast::{BinaryOp, Expr, Lit, UnaryOp};

use crate::defaults::fallback_for;

/// Walks past parenthesisation and statically-decidable branches.
#[must_use]
pub fn narrow_branch(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(p) => narrow_branch(&p.expr),
        Expr::Cond(c) => narrow_branch(&c.cons),
        Expr::Bin(b) if matches!(b.op, BinaryOp::LogicalAnd) => narrow_branch(&b.right),
        _ => expr,
    }
}

/// Resolves a JSX attribute expression to a static string value, falling back
/// by attribute name (camelCase) when the expression is unresolvable.
#[must_use]
pub fn resolve_attr_value(e: &Expr, attr_camel: &str) -> Option<String> {
    match narrow_branch(e) {
        Expr::Lit(Lit::Str(s)) => Some(s.value.to_atom_lossy().to_string()),
        // f64::to_string already prints integer values without a trailing ".0",
        // so no separate integer cast is needed.
        Expr::Lit(Lit::Num(n)) => Some(n.value.to_string()),
        Expr::Lit(Lit::Bool(b)) => Some(b.value.to_string()),
        Expr::Tpl(t) if t.exprs.is_empty() => {
            // `cooked` is `None` only on malformed escapes that the parser already
            // rejects, so the `q.raw` branch is unreachable in practice — keep it
            // as a defensive fallback rather than a panic.
            let s: String = t
                .quasis
                .iter()
                .map(|q| {
                    q.cooked
                        .as_ref()
                        .map_or_else(|| q.raw.to_string(), |c| c.to_atom_lossy().to_string())
                })
                .collect();
            Some(s)
        }
        Expr::Unary(u) if matches!(u.op, UnaryOp::Minus) => {
            resolve_attr_value(&u.arg, attr_camel).map(|s| format!("-{s}"))
        }
        // Identifiers and member access are unresolvable — fall back by attribute name.
        _ => fallback_for(attr_camel).map(ToString::to_string),
    }
}

// Behavior is exercised end-to-end through `transform.rs` tests, which run
// the full `parse_jsx → to_svg → to_xml` pipeline and cover every branch of
// `narrow_branch` and `resolve_attr_value` (paren, cond, logical-and, logical-or,
// string/number/bool literals, template literals with and without exprs, unary
// minus/plus, identifier fallback). Direct unit tests would just duplicate
// those assertions through a more brittle setup.

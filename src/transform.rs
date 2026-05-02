use anyhow::{Result, anyhow};
use swc_ecma_ast::{
    Expr, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement, JSXElementChild,
    JSXElementName, JSXExpr,
};

use crate::attr_map::{AttrAction, camel_to_kebab, map_attr};
use crate::defaults;
use crate::static_eval::{narrow_branch, resolve_attr_value};

#[derive(Debug, Clone)]
pub enum SvgNode {
    Element {
        name: String,
        attrs: Vec<(String, String)>,
        children: Vec<Self>,
    },
    Text(String),
    Comment(String),
}

pub struct TransformResult {
    pub root: SvgNode,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct Ctx {
    warnings: Vec<String>,
    has_xlink: bool,
}

/// Lowers a JSX expression into the internal `SvgNode` tree.
///
/// # Errors
/// Returns an error when the expression contains no JSX elements (e.g. a bare
/// numeric or string literal).
pub fn to_svg(expr: &Expr) -> Result<TransformResult> {
    let mut ctx = Ctx::default();
    let nodes = walk_expr(expr, &mut ctx);
    if nodes.is_empty() {
        return Err(anyhow!("no JSX elements in input"));
    }
    let root = wrap_root(nodes, &ctx);
    Ok(TransformResult {
        root,
        warnings: ctx.warnings,
    })
}

fn walk_expr(expr: &Expr, ctx: &mut Ctx) -> Vec<SvgNode> {
    match narrow_branch(expr) {
        Expr::JSXElement(el) => vec![transform_element(el, ctx)],
        Expr::JSXFragment(frag) => transform_children(&frag.children, ctx),
        // Anything else (including the defensive `Expr::JSXEmpty`, which swc's
        // `parse_file_as_expr` does not surface at the top level).
        _ => Vec::new(),
    }
}

fn transform_element(el: &JSXElement, ctx: &mut Ctx) -> SvgNode {
    let name = match &el.opening.name {
        JSXElementName::Ident(id) => id.sym.to_string(),
        JSXElementName::JSXNamespacedName(ns) => {
            format!("{}:{}", ns.ns.sym, ns.name.sym)
        }
        JSXElementName::JSXMemberExpr(_) => {
            ctx.warnings
                .push("JSX member expression rendered as placeholder".into());
            return placeholder();
        }
    };

    if !is_lowercase_tag(&name) {
        ctx.warnings
            .push(format!("unresolved <{name}/> rendered as placeholder"));
        return placeholder();
    }

    let mut attrs: Vec<(String, String)> = Vec::new();
    for ao in &el.opening.attrs {
        if let JSXAttrOrSpread::JSXAttr(a) = ao
            && let Some((k, v)) = process_attr(a, ctx)
        {
            if k.starts_with("xlink:") {
                ctx.has_xlink = true;
            }
            attrs.push((k, v));
        }
        // SpreadElement is dropped; defaults are applied on the root <svg>.
    }

    let children = transform_children(&el.children, ctx);
    SvgNode::Element {
        name,
        attrs,
        children,
    }
}

fn transform_children(children: &[JSXElementChild], ctx: &mut Ctx) -> Vec<SvgNode> {
    let mut out = Vec::new();
    for c in children {
        match c {
            JSXElementChild::JSXText(t) => {
                let trimmed = t.value.trim();
                if !trimmed.is_empty() {
                    out.push(SvgNode::Text(trimmed.to_string()));
                }
            }
            JSXElementChild::JSXElement(el) => out.push(transform_element(el, ctx)),
            JSXElementChild::JSXFragment(f) => {
                out.extend(transform_children(&f.children, ctx));
            }
            JSXElementChild::JSXExprContainer(ec) => match &ec.expr {
                JSXExpr::JSXEmptyExpr(_) => {}
                JSXExpr::Expr(e) => out.extend(walk_expr(e, ctx)),
            },
            JSXElementChild::JSXSpreadChild(_) => {}
        }
    }
    out
}

fn process_attr(attr: &JSXAttr, ctx: &mut Ctx) -> Option<(String, String)> {
    let raw = match &attr.name {
        JSXAttrName::Ident(id) => id.sym.to_string(),
        JSXAttrName::JSXNamespacedName(ns) => {
            format!("{}:{}", ns.ns.sym, ns.name.sym)
        }
    };

    let target = match map_attr(&raw) {
        AttrAction::Use(n) => n.to_string(),
        AttrAction::Passthrough => raw.clone(),
        AttrAction::Kebab => camel_to_kebab(&raw),
        AttrAction::Drop => return None,
        AttrAction::DropWarn(msg) => {
            ctx.warnings.push(msg.into());
            return None;
        }
    };

    let value = match &attr.value {
        Some(JSXAttrValue::Str(s)) => s.value.to_atom_lossy().to_string(),
        Some(JSXAttrValue::JSXExprContainer(c)) => match &c.expr {
            // Defensive — swc rejects `attr={}` / `attr={/* */}` at parse time (TS1109).
            JSXExpr::JSXEmptyExpr(_) => return None,
            JSXExpr::Expr(e) => resolve_attr_value(e, &raw)?,
        },
        // `None` covers boolean attrs (`attr` without value).
        // `JSXElement`/`JSXFragment` cover `attr=<x/>` — an experimental JSX dialect
        // swc does not parse; values always arrive wrapped in `{}`.
        None | Some(JSXAttrValue::JSXElement(_) | JSXAttrValue::JSXFragment(_)) => return None,
    };

    Some((target, value))
}

fn is_lowercase_tag(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

fn placeholder() -> SvgNode {
    SvgNode::Element {
        name: "rect".into(),
        attrs: vec![
            ("width".into(), "24".into()),
            ("height".into(), "24".into()),
            ("fill".into(), "none".into()),
            ("stroke".into(), "currentColor".into()),
            ("stroke-dasharray".into(), "2 2".into()),
        ],
        children: vec![],
    }
}

fn wrap_root(mut nodes: Vec<SvgNode>, ctx: &Ctx) -> SvgNode {
    if nodes.len() == 1
        && let SvgNode::Element { name, .. } = &nodes[0]
        && name == "svg"
    {
        let SvgNode::Element {
            name,
            mut attrs,
            children,
        } = nodes.pop().unwrap()
        // Unreachable — the outer `if let` already proved this is `Element`.
        else {
            unreachable!()
        };
        ensure_attr(&mut attrs, "xmlns", defaults::XMLNS);
        if ctx.has_xlink {
            ensure_attr(&mut attrs, "xmlns:xlink", defaults::XMLNS_XLINK);
        }
        return SvgNode::Element {
            name,
            attrs,
            children,
        };
    }
    // The closing brace above shows as uncovered in llvm-cov when this fallthrough
    // path runs (early-returns shadow it); a coverage-tool artifact, not dead code.
    wrap_in_svg(nodes, ctx)
}

fn wrap_in_svg(children: Vec<SvgNode>, ctx: &Ctx) -> SvgNode {
    let mut attrs: Vec<(String, String)> = vec![("xmlns".into(), defaults::XMLNS.into())];
    if ctx.has_xlink {
        attrs.push(("xmlns:xlink".into(), defaults::XMLNS_XLINK.into()));
    }
    attrs.extend([
        ("viewBox".into(), defaults::VIEW_BOX.into()),
        ("width".into(), defaults::SIZE.into()),
        ("height".into(), defaults::SIZE.into()),
        ("fill".into(), defaults::FILL.into()),
    ]);
    SvgNode::Element {
        name: "svg".into(),
        attrs,
        children,
    }
}

fn ensure_attr(attrs: &mut Vec<(String, String)>, key: &str, val: &str) {
    if !attrs.iter().any(|(k, _)| k == key) {
        attrs.insert(0, (key.into(), val.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_jsx;
    use crate::serialize::to_xml;

    fn render(tsx: &str) -> (String, Vec<String>) {
        let expr = parse_jsx(tsx).expect("parse_jsx should succeed");
        let res = to_svg(&expr).expect("to_svg should succeed");
        (to_xml(&res.root), res.warnings)
    }

    #[test]
    fn non_jsx_input_errors() {
        let expr = parse_jsx("42").unwrap();
        let err = to_svg(&expr).err().expect("expected error").to_string();
        assert!(err.contains("no JSX elements"), "got: {err}");
    }

    #[test]
    fn parenthesised_jsx_unwrapped() {
        let (out, _) = render("(<svg><path d=\"M0\"/></svg>)");
        assert!(out.starts_with("<svg"), "got: {out}");
        assert!(out.contains("<path"));
    }

    #[test]
    fn jsx_member_expr_tag_renders_placeholder_with_warning() {
        let (out, warnings) = render("<Ns.Icon/>");
        assert!(out.contains("<rect"), "expected placeholder, got: {out}");
        assert!(
            warnings.iter().any(|w| w.contains("member expression")),
            "warnings: {warnings:?}"
        );
    }

    #[test]
    fn namespaced_jsx_tag_kept_with_colon() {
        let (out, _) = render("<svg><xlink:foo/></svg>");
        assert!(out.contains("<xlink:foo"), "got: {out}");
    }

    #[test]
    fn text_child_rendered() {
        let (out, _) = render("<svg><text>Hello</text></svg>");
        assert!(out.contains(">Hello</text>"), "got: {out}");
    }

    #[test]
    fn fragment_child_is_flattened() {
        let (out, _) = render("<svg><><path d=\"M0\"/></></svg>");
        assert!(out.contains("<path"), "got: {out}");
    }

    #[test]
    fn expr_container_child_with_jsx_rendered() {
        let (out, _) = render("<svg>{<path d=\"M0\"/>}</svg>");
        assert!(out.contains("<path"), "got: {out}");
    }

    #[test]
    fn expr_container_child_with_empty_expr_skipped() {
        let (out, _) = render("<svg>{/* hi */}</svg>");
        assert!(out.starts_with("<svg"), "got: {out}");
    }

    #[test]
    fn spread_child_is_skipped() {
        let (out, _) = render("<svg>{...children}<path/></svg>");
        assert!(out.contains("<path"), "got: {out}");
    }

    #[test]
    fn boolean_attr_without_value_is_dropped() {
        let (out, _) = render("<svg disabled/>");
        assert!(!out.contains("disabled"), "got: {out}");
    }

    #[test]
    fn attr_with_jsx_element_value_is_dropped() {
        let (out, _) = render("<svg foo={<b/>}/>");
        assert!(!out.contains("foo="), "got: {out}");
    }

    #[test]
    fn attr_str_literal_in_expr_container() {
        let (out, _) = render(r#"<svg width={"50"}/>"#);
        assert!(out.contains(r#"width="50""#), "got: {out}");
    }

    #[test]
    fn attr_bool_literal_resolved_as_string() {
        let (out, _) = render("<svg foo={true}/>");
        assert!(out.contains(r#"foo="true""#), "got: {out}");
    }

    #[test]
    fn attr_template_literal_no_exprs_resolved() {
        let (out, _) = render("<svg width={`50`}/>");
        assert!(out.contains(r#"width="50""#), "got: {out}");
    }

    #[test]
    fn attr_paren_resolved() {
        let (out, _) = render("<svg width={(24)}/>");
        assert!(out.contains(r#"width="24""#), "got: {out}");
    }

    #[test]
    fn attr_cond_resolved_to_consequent() {
        let (out, _) = render(r#"<svg fill={x ? "red" : "blue"}/>"#);
        assert!(out.contains(r#"fill="red""#), "got: {out}");
    }

    #[test]
    fn attr_logical_and_resolved_to_rhs() {
        let (out, _) = render(r#"<svg fill={x && "red"}/>"#);
        assert!(out.contains(r#"fill="red""#), "got: {out}");
    }

    #[test]
    fn attr_logical_or_falls_back_not_narrowed() {
        // Only `&&` narrows to RHS; `||` leaves the expression unresolved → fallback.
        let (out, _) = render(r#"<svg fill={x || "red"}/>"#);
        assert!(out.contains(r#"fill="currentColor""#), "got: {out}");
        assert!(!out.contains(r#"fill="red""#), "got: {out}");
    }

    #[test]
    fn attr_template_with_exprs_falls_back() {
        // Template literals with interpolation are NOT statically resolvable → fallback.
        let (out, _) = render(r"<svg fill={`a${x}b`}/>");
        assert!(out.contains(r#"fill="currentColor""#), "got: {out}");
        // Must not have produced a literal join "ab".
        assert!(!out.contains(r#"fill="ab""#), "got: {out}");
    }

    #[test]
    fn attr_unary_plus_falls_back_not_negated() {
        // Only `UnaryOp::Minus` triggers numeric negation. Unary `+5` falls through
        // to the fallback (which is None for `x`), so the attribute is dropped.
        let (out, _) = render("<svg x={+5}/>");
        assert!(!out.contains("x="), "got: {out}");
    }

    #[test]
    fn attr_unary_minus_number() {
        let (out, _) = render("<svg x={-5}/>");
        assert!(out.contains(r#"x="-5""#), "got: {out}");
    }

    #[test]
    fn attr_non_integer_number_kept_as_decimal() {
        let (out, _) = render("<svg x={1.5}/>");
        assert!(out.contains(r#"x="1.5""#), "got: {out}");
    }

    #[test]
    fn attr_unresolvable_uses_fallback() {
        // Identifier value is unresolvable; falls back by attribute name.
        let (out, _) = render("<svg fill={someVar}/>");
        assert!(out.contains(r#"fill="currentColor""#), "got: {out}");
    }

    #[test]
    fn namespaced_attr_with_xlink_marks_root() {
        // Raw `xlink:href` (JSXNamespacedName attr) → passthrough; root must get xmlns:xlink.
        let (out, _) = render(r##"<svg><use xlink:href="#a"/></svg>"##);
        assert!(out.contains("xmlns:xlink="), "got: {out}");
    }

    #[test]
    fn xlink_camel_attr_on_non_svg_root_wraps_with_xlink() {
        // Single non-svg root → wrap_in_svg path; has_xlink set via xlinkHref.
        let (out, _) = render(r##"<use xlinkHref="#a"/>"##);
        assert!(out.contains("xmlns:xlink="), "got: {out}");
    }

    #[test]
    fn single_non_svg_root_wraps_in_svg() {
        let (out, _) = render(r#"<path d="M0"/>"#);
        assert!(out.starts_with("<svg"), "got: {out}");
        assert!(out.contains("<path"), "got: {out}");
    }
}

use anyhow::{anyhow, Result};
use swc_ecma_ast::{
    BinaryOp, Expr, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue, JSXElement,
    JSXElementChild, JSXElementName, JSXExpr, Lit, UnaryOp,
};

use crate::attr_map::{camel_to_kebab, map_attr, AttrAction};
use crate::defaults::{self, fallback_for};

#[derive(Debug, Clone)]
pub enum SvgNode {
    Element {
        name: String,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
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
    match expr {
        Expr::JSXElement(el) => vec![transform_element(el, ctx)],
        Expr::JSXFragment(frag) => transform_children(&frag.children, ctx),
        Expr::JSXEmpty(_) => Vec::new(),
        Expr::Paren(p) => walk_expr(&p.expr, ctx),
        // For ternary expressions, render the consequent (first branch).
        Expr::Cond(c) => walk_expr(&c.cons, ctx),
        // For `cond && <jsx/>`, render the right-hand side.
        Expr::Bin(b) if matches!(b.op, BinaryOp::LogicalAnd) => walk_expr(&b.right, ctx),
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
            .push(format!("unresolved <{}/> rendered as placeholder", name));
        return placeholder();
    }

    let mut attrs: Vec<(String, String)> = Vec::new();
    for ao in &el.opening.attrs {
        if let JSXAttrOrSpread::JSXAttr(a) = ao {
            if let Some((k, v)) = process_attr(a, ctx) {
                if k.starts_with("xlink:") {
                    ctx.has_xlink = true;
                }
                attrs.push((k, v));
            }
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
        None => return None,
        Some(JSXAttrValue::Str(s)) => s.value.to_atom_lossy().to_string(),
        Some(JSXAttrValue::JSXExprContainer(c)) => match &c.expr {
            JSXExpr::JSXEmptyExpr(_) => return None,
            JSXExpr::Expr(e) => resolve_expr_value(e, &raw)?,
        },
        Some(JSXAttrValue::JSXElement(_)) | Some(JSXAttrValue::JSXFragment(_)) => return None,
    };

    Some((target, value))
}

fn resolve_expr_value(e: &Expr, attr_camel: &str) -> Option<String> {
    match e {
        Expr::Lit(Lit::Str(s)) => Some(s.value.to_atom_lossy().to_string()),
        Expr::Lit(Lit::Num(n)) => Some(format_number(n.value)),
        Expr::Lit(Lit::Bool(b)) => Some(b.value.to_string()),
        Expr::Tpl(t) if t.exprs.is_empty() => {
            let s: String = t
                .quasis
                .iter()
                .map(|q| match &q.cooked {
                    Some(c) => c.to_atom_lossy().to_string(),
                    None => q.raw.to_string(),
                })
                .collect();
            Some(s)
        }
        Expr::Paren(p) => resolve_expr_value(&p.expr, attr_camel),
        Expr::Cond(c) => resolve_expr_value(&c.cons, attr_camel),
        Expr::Bin(b) if matches!(b.op, BinaryOp::LogicalAnd) => {
            resolve_expr_value(&b.right, attr_camel)
        }
        Expr::Unary(u) if matches!(u.op, UnaryOp::Minus) => {
            resolve_expr_value(&u.arg, attr_camel).map(|s| format!("-{}", s))
        }
        // Identifiers and member access are unresolvable — fall back by attribute name.
        _ => fallback_for(attr_camel).map(|s| s.to_string()),
    }
}

fn format_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e16 {
        format!("{}", n as i64)
    } else {
        n.to_string()
    }
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
    if nodes.len() == 1 {
        if let SvgNode::Element { name, .. } = &nodes[0] {
            if name == "svg" {
                let SvgNode::Element {
                    name,
                    mut attrs,
                    children,
                } = nodes.pop().unwrap()
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
        }
    }
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

use crate::transform::SvgNode;

pub fn to_xml(node: &SvgNode) -> String {
    let mut out = String::with_capacity(256);
    write_node(node, &mut out);
    out
}

fn write_node(node: &SvgNode, out: &mut String) {
    match node {
        SvgNode::Element {
            name,
            attrs,
            children,
        } => {
            out.push('<');
            out.push_str(name);
            for (k, v) in attrs {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                push_escaped(v, out, true);
                out.push('"');
            }
            if children.is_empty() && is_void(name) {
                out.push_str("/>");
            } else {
                out.push('>');
                for c in children {
                    write_node(c, out);
                }
                out.push_str("</");
                out.push_str(name);
                out.push('>');
            }
        }
        SvgNode::Text(t) => push_escaped(t, out, false),
        SvgNode::Comment(c) => {
            out.push_str("<!-- ");
            out.push_str(&c.replace("--", "- - "));
            out.push_str(" -->");
        }
    }
}

fn push_escaped(s: &str, out: &mut String, in_attr: bool) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' if in_attr => out.push_str("&quot;"),
            '>' if !in_attr => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn is_void(name: &str) -> bool {
    matches!(
        name,
        "path"
            | "circle"
            | "rect"
            | "line"
            | "polyline"
            | "polygon"
            | "ellipse"
            | "use"
            | "image"
            | "stop"
    )
}

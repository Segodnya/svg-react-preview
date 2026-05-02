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

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(name: &str, attrs: &[(&str, &str)], children: Vec<SvgNode>) -> SvgNode {
        SvgNode::Element {
            name: name.into(),
            attrs: attrs
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            children,
        }
    }

    #[test]
    fn renders_text_child() {
        let n = elem("text", &[], vec![SvgNode::Text("hi".into())]);
        assert_eq!(to_xml(&n), "<text>hi</text>");
    }

    #[test]
    fn renders_comment_child() {
        let n = elem("g", &[], vec![SvgNode::Comment("hello".into())]);
        assert_eq!(to_xml(&n), "<g><!-- hello --></g>");
    }

    #[test]
    fn comment_internal_dashes_are_neutralised() {
        let n = SvgNode::Comment("a--b".into());
        assert_eq!(to_xml(&n), "<!-- a- - b -->");
    }

    #[test]
    fn escapes_attr_values() {
        let n = elem("g", &[("title", r#"a&b<c"d"#)], vec![]);
        assert_eq!(to_xml(&n), r#"<g title="a&amp;b&lt;c&quot;d"></g>"#);
    }

    #[test]
    fn escapes_text_content() {
        let n = elem("text", &[], vec![SvgNode::Text("a&b<c>d".into())]);
        assert_eq!(to_xml(&n), "<text>a&amp;b&lt;c&gt;d</text>");
    }

    #[test]
    fn void_element_self_closes_when_empty() {
        let p = elem("path", &[("d", "M0")], vec![]);
        assert_eq!(to_xml(&p), r#"<path d="M0"/>"#);
    }

    #[test]
    fn void_element_with_children_uses_open_close() {
        let p = elem("path", &[], vec![SvgNode::Text("x".into())]);
        assert_eq!(to_xml(&p), "<path>x</path>");
    }

    #[test]
    fn non_void_element_uses_open_close() {
        let g = elem("g", &[], vec![]);
        assert_eq!(to_xml(&g), "<g></g>");
    }
}

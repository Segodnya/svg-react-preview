// Default values used when JSX expressions cannot be resolved statically.

pub const SIZE: &str = "24";
pub const VIEW_BOX: &str = "0 0 24 24";
pub const FILL: &str = "currentColor";
pub const STROKE: &str = "none";
pub const STROKE_WIDTH: &str = "1.5";
pub const XMLNS: &str = "http://www.w3.org/2000/svg";
pub const XMLNS_XLINK: &str = "http://www.w3.org/1999/xlink";

/// Returns a fallback value for a JSX attribute name (camelCase) or its SVG
/// equivalent (kebab-case), or `None` if no fallback is defined.
pub fn fallback_for(attr: &str) -> Option<&'static str> {
    match attr {
        "size" | "width" | "height" => Some(SIZE),
        "color" | "fill" => Some(FILL),
        "stroke" => Some(STROKE),
        "strokeWidth" | "stroke-width" => Some(STROKE_WIDTH),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_family() {
        assert_eq!(fallback_for("size"), Some(SIZE));
        assert_eq!(fallback_for("width"), Some(SIZE));
        assert_eq!(fallback_for("height"), Some(SIZE));
    }

    #[test]
    fn color_family() {
        assert_eq!(fallback_for("color"), Some(FILL));
        assert_eq!(fallback_for("fill"), Some(FILL));
    }

    #[test]
    fn stroke_family() {
        assert_eq!(fallback_for("stroke"), Some(STROKE));
        assert_eq!(fallback_for("strokeWidth"), Some(STROKE_WIDTH));
        assert_eq!(fallback_for("stroke-width"), Some(STROKE_WIDTH));
    }

    #[test]
    fn unknown_attrs_return_none() {
        assert_eq!(fallback_for("d"), None);
        assert_eq!(fallback_for("viewBox"), None);
        assert_eq!(fallback_for(""), None);
    }
}

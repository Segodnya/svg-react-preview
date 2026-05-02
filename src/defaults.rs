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

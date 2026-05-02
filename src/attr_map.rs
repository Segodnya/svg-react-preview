// Maps JSX camelCase attribute names to SVG attribute names.
//
// Strategy:
//   - explicit drops (event handlers, React-only props)
//   - explicit static rewrites (className → class, namespaced xlink:*/xml:*, panose1)
//   - KEEP_CAMEL: SVG 1.1+2 attributes whose canonical form is camelCase
//   - everything else with a capital letter is auto-kebabed (strokeWidth → stroke-width)

pub enum AttrAction {
    /// Use the attribute under this static name.
    Use(&'static str),
    /// Use the attribute under its original (camelCase) name.
    Passthrough,
    /// Convert the original name from camelCase to kebab-case.
    Kebab,
    /// Silently drop (e.g. on-handlers).
    Drop,
    /// Drop and warn the user.
    DropWarn(&'static str),
}

/// SVG attributes that are natively camelCase (must NOT be kebabed).
/// Sourced from the SVG 1.1 + 2 specifications and MDN.
const KEEP_CAMEL: &[&str] = &[
    "attributeName",
    "attributeType",
    "baseFrequency",
    "baseProfile",
    "calcMode",
    "clipPathUnits",
    "contentScriptType",
    "contentStyleType",
    "diffuseConstant",
    "edgeMode",
    "externalResourcesRequired",
    "filterRes",
    "filterUnits",
    "glyphRef",
    "gradientTransform",
    "gradientUnits",
    "kernelMatrix",
    "kernelUnitLength",
    "keyPoints",
    "keySplines",
    "keyTimes",
    "lengthAdjust",
    "limitingConeAngle",
    "markerHeight",
    "markerUnits",
    "markerWidth",
    "maskContentUnits",
    "maskUnits",
    "numOctaves",
    "pathLength",
    "patternContentUnits",
    "patternTransform",
    "patternUnits",
    "pointsAtX",
    "pointsAtY",
    "pointsAtZ",
    "preserveAlpha",
    "preserveAspectRatio",
    "primitiveUnits",
    "refX",
    "refY",
    "repeatCount",
    "repeatDur",
    "requiredExtensions",
    "requiredFeatures",
    "specularConstant",
    "specularExponent",
    "spreadMethod",
    "startOffset",
    "stdDeviation",
    "stitchTiles",
    "surfaceScale",
    "systemLanguage",
    "tableValues",
    "targetX",
    "targetY",
    "textLength",
    "viewBox",
    "viewTarget",
    "xChannelSelector",
    "yChannelSelector",
    "zoomAndPan",
];

pub fn map_attr(name: &str) -> AttrAction {
    if is_event_handler(name) {
        return AttrAction::Drop;
    }
    match name {
        "htmlFor" | "ref" | "key" => AttrAction::Drop,
        "dangerouslySetInnerHTML" => AttrAction::DropWarn(
            "dangerouslySetInnerHTML stripped — arbitrary HTML rendering is not supported",
        ),
        "className" => AttrAction::Use("class"),

        // Digit boundary — heuristic can't infer the dash, so handle explicitly.
        "panose1" => AttrAction::Use("panose-1"),

        // xlink:* and xml:* are namespaced. xlink:* also requires xmlns:xlink on the root.
        "xlinkActuate" => AttrAction::Use("xlink:actuate"),
        "xlinkArcrole" => AttrAction::Use("xlink:arcrole"),
        "xlinkHref" => AttrAction::Use("xlink:href"),
        "xlinkRole" => AttrAction::Use("xlink:role"),
        "xlinkShow" => AttrAction::Use("xlink:show"),
        "xlinkTitle" => AttrAction::Use("xlink:title"),
        "xlinkType" => AttrAction::Use("xlink:type"),
        "xmlnsXlink" => AttrAction::Use("xmlns:xlink"),
        "xmlBase" => AttrAction::Use("xml:base"),
        "xmlLang" => AttrAction::Use("xml:lang"),
        "xmlSpace" => AttrAction::Use("xml:space"),

        _ if KEEP_CAMEL.contains(&name) => AttrAction::Passthrough,
        _ if name.bytes().any(|b| b.is_ascii_uppercase()) => AttrAction::Kebab,
        _ => AttrAction::Passthrough,
    }
}

/// Inserts `-` before each uppercase ASCII letter, then lowercases.
/// `strokeWidth` → `stroke-width`, `accentHeight` → `accent-height`.
pub fn camel_to_kebab(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.char_indices() {
        if i > 0 && c.is_ascii_uppercase() {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

fn is_event_handler(name: &str) -> bool {
    name.strip_prefix("on")
        .and_then(|r| r.chars().next())
        .is_some_and(|c| c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_basics() {
        assert_eq!(camel_to_kebab("strokeWidth"), "stroke-width");
        assert_eq!(camel_to_kebab("accentHeight"), "accent-height");
        assert_eq!(
            camel_to_kebab("glyphOrientationHorizontal"),
            "glyph-orientation-horizontal"
        );
        assert_eq!(camel_to_kebab("vAlphabetic"), "v-alphabetic");
        assert_eq!(camel_to_kebab("xHeight"), "x-height");
    }

    #[test]
    fn keep_camel_passes_through() {
        assert!(matches!(map_attr("viewBox"), AttrAction::Passthrough));
        assert!(matches!(
            map_attr("preserveAspectRatio"),
            AttrAction::Passthrough
        ));
        assert!(matches!(map_attr("gradientUnits"), AttrAction::Passthrough));
        assert!(matches!(map_attr("refX"), AttrAction::Passthrough));
    }

    #[test]
    fn camel_attrs_are_kebabed() {
        assert!(matches!(map_attr("strokeWidth"), AttrAction::Kebab));
        assert!(matches!(map_attr("clipPath"), AttrAction::Kebab));
        assert!(matches!(map_attr("fillOpacity"), AttrAction::Kebab));
    }

    #[test]
    fn lowercase_passes_through() {
        assert!(matches!(map_attr("fill"), AttrAction::Passthrough));
        assert!(matches!(map_attr("d"), AttrAction::Passthrough));
    }
}

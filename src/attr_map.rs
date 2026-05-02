// Maps JSX camelCase attribute names to SVG attribute names (kebab-case or namespaced).
// Attributes not in the table pass through unchanged: viewBox, preserveAspectRatio,
// gradientUnits, refX, attributeName, etc., are natively camelCase in SVG.

pub enum AttrAction {
    /// Use the attribute under this name.
    Use(String),
    /// Silently drop (e.g. on-handlers).
    Drop,
    /// Drop and warn the user.
    DropWarn(&'static str),
}

pub fn map_attr(name: &str) -> AttrAction {
    if is_event_handler(name) {
        return AttrAction::Drop;
    }
    match name {
        "htmlFor" => AttrAction::Drop,
        "dangerouslySetInnerHTML" => {
            AttrAction::DropWarn("dangerouslySetInnerHTML stripped — arbitrary HTML rendering is not supported")
        }
        "ref" | "key" => AttrAction::Drop,
        "className" => AttrAction::Use("class".into()),

        // SVG attributes that require kebab-case.
        "accentHeight" => AttrAction::Use("accent-height".into()),
        "alignmentBaseline" => AttrAction::Use("alignment-baseline".into()),
        "arabicForm" => AttrAction::Use("arabic-form".into()),
        "baselineShift" => AttrAction::Use("baseline-shift".into()),
        "capHeight" => AttrAction::Use("cap-height".into()),
        "clipPath" => AttrAction::Use("clip-path".into()),
        "clipRule" => AttrAction::Use("clip-rule".into()),
        "colorInterpolation" => AttrAction::Use("color-interpolation".into()),
        "colorInterpolationFilters" => AttrAction::Use("color-interpolation-filters".into()),
        "colorProfile" => AttrAction::Use("color-profile".into()),
        "colorRendering" => AttrAction::Use("color-rendering".into()),
        "dominantBaseline" => AttrAction::Use("dominant-baseline".into()),
        "enableBackground" => AttrAction::Use("enable-background".into()),
        "fillOpacity" => AttrAction::Use("fill-opacity".into()),
        "fillRule" => AttrAction::Use("fill-rule".into()),
        "floodColor" => AttrAction::Use("flood-color".into()),
        "floodOpacity" => AttrAction::Use("flood-opacity".into()),
        "fontFamily" => AttrAction::Use("font-family".into()),
        "fontSize" => AttrAction::Use("font-size".into()),
        "fontSizeAdjust" => AttrAction::Use("font-size-adjust".into()),
        "fontStretch" => AttrAction::Use("font-stretch".into()),
        "fontStyle" => AttrAction::Use("font-style".into()),
        "fontVariant" => AttrAction::Use("font-variant".into()),
        "fontWeight" => AttrAction::Use("font-weight".into()),
        "glyphName" => AttrAction::Use("glyph-name".into()),
        "glyphOrientationHorizontal" => AttrAction::Use("glyph-orientation-horizontal".into()),
        "glyphOrientationVertical" => AttrAction::Use("glyph-orientation-vertical".into()),
        "horizAdvX" => AttrAction::Use("horiz-adv-x".into()),
        "horizOriginX" => AttrAction::Use("horiz-origin-x".into()),
        "imageRendering" => AttrAction::Use("image-rendering".into()),
        "letterSpacing" => AttrAction::Use("letter-spacing".into()),
        "lightingColor" => AttrAction::Use("lighting-color".into()),
        "markerEnd" => AttrAction::Use("marker-end".into()),
        "markerMid" => AttrAction::Use("marker-mid".into()),
        "markerStart" => AttrAction::Use("marker-start".into()),
        "overlinePosition" => AttrAction::Use("overline-position".into()),
        "overlineThickness" => AttrAction::Use("overline-thickness".into()),
        "paintOrder" => AttrAction::Use("paint-order".into()),
        "panose1" => AttrAction::Use("panose-1".into()),
        "pointerEvents" => AttrAction::Use("pointer-events".into()),
        "renderingIntent" => AttrAction::Use("rendering-intent".into()),
        "shapeRendering" => AttrAction::Use("shape-rendering".into()),
        "stopColor" => AttrAction::Use("stop-color".into()),
        "stopOpacity" => AttrAction::Use("stop-opacity".into()),
        "strikethroughPosition" => AttrAction::Use("strikethrough-position".into()),
        "strikethroughThickness" => AttrAction::Use("strikethrough-thickness".into()),
        "strokeDasharray" => AttrAction::Use("stroke-dasharray".into()),
        "strokeDashoffset" => AttrAction::Use("stroke-dashoffset".into()),
        "strokeLinecap" => AttrAction::Use("stroke-linecap".into()),
        "strokeLinejoin" => AttrAction::Use("stroke-linejoin".into()),
        "strokeMiterlimit" => AttrAction::Use("stroke-miterlimit".into()),
        "strokeOpacity" => AttrAction::Use("stroke-opacity".into()),
        "strokeWidth" => AttrAction::Use("stroke-width".into()),
        "textAnchor" => AttrAction::Use("text-anchor".into()),
        "textDecoration" => AttrAction::Use("text-decoration".into()),
        "textRendering" => AttrAction::Use("text-rendering".into()),
        "transformOrigin" => AttrAction::Use("transform-origin".into()),
        "underlinePosition" => AttrAction::Use("underline-position".into()),
        "underlineThickness" => AttrAction::Use("underline-thickness".into()),
        "unicodeBidi" => AttrAction::Use("unicode-bidi".into()),
        "unicodeRange" => AttrAction::Use("unicode-range".into()),
        "unitsPerEm" => AttrAction::Use("units-per-em".into()),
        "vAlphabetic" => AttrAction::Use("v-alphabetic".into()),
        "vHanging" => AttrAction::Use("v-hanging".into()),
        "vIdeographic" => AttrAction::Use("v-ideographic".into()),
        "vMathematical" => AttrAction::Use("v-mathematical".into()),
        "vectorEffect" => AttrAction::Use("vector-effect".into()),
        "vertAdvY" => AttrAction::Use("vert-adv-y".into()),
        "vertOriginX" => AttrAction::Use("vert-origin-x".into()),
        "vertOriginY" => AttrAction::Use("vert-origin-y".into()),
        "wordSpacing" => AttrAction::Use("word-spacing".into()),
        "writingMode" => AttrAction::Use("writing-mode".into()),
        "xHeight" => AttrAction::Use("x-height".into()),

        // xlink:* and xml:* are namespaced. xlink:* also requires xmlns:xlink on the root.
        "xlinkActuate" => AttrAction::Use("xlink:actuate".into()),
        "xlinkArcrole" => AttrAction::Use("xlink:arcrole".into()),
        "xlinkHref" => AttrAction::Use("xlink:href".into()),
        "xlinkRole" => AttrAction::Use("xlink:role".into()),
        "xlinkShow" => AttrAction::Use("xlink:show".into()),
        "xlinkTitle" => AttrAction::Use("xlink:title".into()),
        "xlinkType" => AttrAction::Use("xlink:type".into()),
        "xmlnsXlink" => AttrAction::Use("xmlns:xlink".into()),
        "xmlBase" => AttrAction::Use("xml:base".into()),
        "xmlLang" => AttrAction::Use("xml:lang".into()),
        "xmlSpace" => AttrAction::Use("xml:space".into()),

        other => AttrAction::Use(other.to_string()),
    }
}

fn is_event_handler(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 2 && bytes[0] == b'o' && bytes[1] == b'n' && bytes[2].is_ascii_uppercase()
}

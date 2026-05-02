use svg_react_preview::pipeline;
use svg_react_preview::source::Source;

fn render(tsx: &str) -> (String, Vec<String>) {
    let r = pipeline::render(Source::Fragment(tsx.trim().into()))
        .expect("pipeline::render should succeed");
    (r.xml, r.warnings)
}

fn assert_golden(name: &str, tsx: &str, want: &str) {
    let (got, _) = render(tsx);
    assert_eq!(got.trim_end(), want.trim_end(), "fixture {name} mismatch");
}

#[test]
fn full_svg() {
    assert_golden(
        "full_svg",
        include_str!("fixtures/full_svg.tsx"),
        include_str!("fixtures/full_svg.svg"),
    );
}

#[test]
fn path_only() {
    assert_golden(
        "path_only",
        include_str!("fixtures/path_only.tsx"),
        include_str!("fixtures/path_only.svg"),
    );
}

#[test]
fn dynamic_size() {
    assert_golden(
        "dynamic_size",
        include_str!("fixtures/dynamic_size.tsx"),
        include_str!("fixtures/dynamic_size.svg"),
    );
}

#[test]
fn pascal_case_emits_placeholder_with_warning() {
    let (got, warnings) = render(include_str!("fixtures/pascal_case.tsx"));
    assert_eq!(
        got.trim_end(),
        include_str!("fixtures/pascal_case.svg").trim_end()
    );
    assert!(
        warnings.iter().any(|w| w.contains("Icon")),
        "expected unresolved <Icon/> warning, got {warnings:?}"
    );
}

#[test]
fn attr_normalization() {
    assert_golden(
        "attr_normalization",
        include_str!("fixtures/attr_normalization.tsx"),
        include_str!("fixtures/attr_normalization.svg"),
    );
}

#[test]
fn fragment_root() {
    assert_golden(
        "fragment",
        include_str!("fixtures/fragment.tsx"),
        include_str!("fixtures/fragment.svg"),
    );
}

#[test]
fn cond_expression_uses_consequent() {
    assert_golden(
        "cond_expr",
        include_str!("fixtures/cond_expr.tsx"),
        include_str!("fixtures/cond_expr.svg"),
    );
}

#[test]
fn logical_and_renders_rhs() {
    assert_golden(
        "logical_and",
        include_str!("fixtures/logical_and.tsx"),
        include_str!("fixtures/logical_and.svg"),
    );
}

#[test]
fn spread_dropped_xmlns_added() {
    assert_golden(
        "spread",
        include_str!("fixtures/spread.tsx"),
        include_str!("fixtures/spread.svg"),
    );
}

#[test]
fn dangerous_html_dropped_with_warning() {
    let (got, warnings) = render(include_str!("fixtures/dangerous_html.tsx"));
    assert_eq!(
        got.trim_end(),
        include_str!("fixtures/dangerous_html.svg").trim_end()
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("dangerouslySetInnerHTML")),
        "expected dangerouslySetInnerHTML warning, got {warnings:?}"
    );
}

#[test]
fn invalid_jsx_returns_error() {
    let tsx = include_str!("fixtures/invalid_jsx.tsx").trim();
    let res = pipeline::render(Source::Fragment(tsx.into()));
    assert!(res.is_err(), "expected error for {tsx:?}");
}

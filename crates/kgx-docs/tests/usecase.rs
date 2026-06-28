use kgx_docs::usecase::{parse, render, UseCase};

#[test]
fn parses_all_six_use_cases() {
    for name in [
        "research",
        "onboarding",
        "meetings",
        "pkm",
        "agent-memory",
        "team-sharing",
    ] {
        assert!(parse(name).is_some(), "{name}");
    }
}

#[test]
fn research_html_contains_copy_pasteable_flow() {
    let html = render(UseCase::Research);
    assert!(html.contains("<html"));
    assert!(html.contains("kg capture"));
    assert!(html.contains("kg extract"));
    assert!(html.contains("kg ask"));
    assert!(html.contains("```mermaid"));
}

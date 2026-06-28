use kgx_extract::prompt::extract_prompt;

#[test]
fn extract_prompt_prepends_ladder_when_supplied() {
    let prompt = extract_prompt("Source body", Some("Extract only explicit facts."));

    assert!(prompt.starts_with("Extract only explicit facts."));
    assert!(prompt.contains("EXTRACT_FACTS"));
    assert!(prompt.contains("Source body"));
}

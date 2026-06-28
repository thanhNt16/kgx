use kgx_ponytail::{ladder_for, Intensity, Operation};

#[test]
fn ladder_for_returns_operation_intensity_prompt() {
    let lite_extract = ladder_for(Operation::Extract, Intensity::Lite);
    assert!(lite_extract.contains("explicit"));
    assert!(lite_extract.contains("atomic facts"));

    let ultra_dream = ladder_for(Operation::Dream, Intensity::Ultra);
    assert!(ultra_dream.contains("never delete"));
    assert!(ultra_dream.contains("Justify every merge"));

    let ask = ladder_for(Operation::Ask, Intensity::Full);
    assert!(ask.contains("Cite note ids"));

    let review = ladder_for(Operation::Review, Intensity::Full);
    assert!(review.contains("Flag diffs"));
}

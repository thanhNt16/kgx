#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Lite,
    Full,
    Ultra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Extract,
    Dream,
    Ask,
    Review,
}

pub fn ladder_for(op: Operation, intensity: Intensity) -> &'static str {
    match (op, intensity) {
        (Operation::Extract, Intensity::Lite) => {
            "Extract only explicit, atomic facts. One claim per note. No inference."
        }
        (Operation::Extract, _) => {
            "Extract atomic facts with provenance. Add entities only when named. Avoid speculative facts."
        }
        (Operation::Dream, Intensity::Ultra) => {
            "Consolidate aggressively but never delete; supersede or archive. Justify every merge."
        }
        (Operation::Dream, _) => {
            "Propose the minimal consolidation. Prefer no-op over speculative restructuring."
        }
        (Operation::Ask, _) => {
            "Answer only from provided context. Cite note ids. Say 'unknown' if unsupported."
        }
        (Operation::Review, _) => {
            "Flag diffs that add structure beyond what the rationale justifies."
        }
    }
}

use kgx_core::{llm::Embedder, llm::LlmProvider, Note};
use kgx_graph::Brain;

pub struct DreamContext<'a> {
    pub notes: &'a [Note],
    pub brain: &'a Brain,
    pub provider: &'a dyn LlmProvider,
    pub embedder: &'a dyn Embedder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassId {
    Dedup,
    Contradiction,
    Supersession,
    Staleness,
    Community,
    OrphanRepair,
    OpenQuestions,
}

impl PassId {
    pub fn name(self) -> &'static str {
        match self {
            PassId::Dedup => "dedup",
            PassId::Contradiction => "contradiction",
            PassId::Supersession => "supersession",
            PassId::Staleness => "staleness",
            PassId::Community => "community",
            PassId::OrphanRepair => "orphan_repair",
            PassId::OpenQuestions => "open_questions",
        }
    }

    pub fn parse(s: &str) -> Option<PassId> {
        match s {
            "dedup" => Some(Self::Dedup),
            "contradiction" => Some(Self::Contradiction),
            "supersession" => Some(Self::Supersession),
            "staleness" => Some(Self::Staleness),
            "community" => Some(Self::Community),
            "orphan_repair" => Some(Self::OrphanRepair),
            "open_questions" => Some(Self::OpenQuestions),
            _ => None,
        }
    }

    pub fn all() -> [PassId; 7] {
        [
            Self::Dedup,
            Self::Contradiction,
            Self::Supersession,
            Self::Staleness,
            Self::Community,
            Self::OrphanRepair,
            Self::OpenQuestions,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseCase {
    Research,
    Onboarding,
    Meetings,
    Pkm,
    AgentMemory,
    TeamSharing,
}

pub fn parse(s: &str) -> Option<UseCase> {
    match s {
        "research" => Some(UseCase::Research),
        "onboarding" => Some(UseCase::Onboarding),
        "meetings" => Some(UseCase::Meetings),
        "pkm" => Some(UseCase::Pkm),
        "agent-memory" | "agent_memory" => Some(UseCase::AgentMemory),
        "team-sharing" | "team_sharing" => Some(UseCase::TeamSharing),
        _ => None,
    }
}

pub fn render(usecase: UseCase) -> String {
    let spec = spec(usecase);
    let mut ctx = tera::Context::new();
    ctx.insert("title", spec.title);
    ctx.insert("narrative", spec.narrative);
    ctx.insert("commands", spec.commands);
    ctx.insert("mermaid", spec.mermaid);
    tera::Tera::one_off(include_str!("../templates/usecase.html.tera"), &ctx, false)
        .expect("usecase template renders")
}

struct Spec {
    title: &'static str,
    narrative: &'static str,
    commands: &'static str,
    mermaid: &'static str,
}

fn spec(usecase: UseCase) -> Spec {
    match usecase {
        UseCase::Research => Spec {
            title: "Research",
            narrative: "Capture source material, extract claims, and ask cited questions over the local graph.",
            commands: "kg capture --from ./paper.md --type doc\nkg extract --source raw/paper.md\nkg index --full --pagerank\nkg ask \"What are the migration risks?\" --cite",
            mermaid: "flowchart LR\n  Capture --> Extract --> Index --> Ask",
        },
        UseCase::Onboarding => Spec {
            title: "Onboarding",
            narrative: "Build a navigable map of decisions, entities, and source notes for a new teammate.",
            commands: "kg init --template pkm --okf\nkg capture --from ./runbook.md --type doc\nkg extract --source raw/runbook.md\nkg graph --format html --out onboarding.html",
            mermaid: "flowchart LR\n  Init --> Capture --> Extract --> Graph",
        },
        UseCase::Meetings => Spec {
            title: "Meetings",
            narrative: "Turn meeting notes into durable facts, decisions, and follow-up questions.",
            commands: "kg capture --from ./meeting.md --type meeting\nkg extract --source raw/meeting.md\nkg dream --only open-questions --dry-run\nkg review --interactive",
            mermaid: "flowchart LR\n  Meeting --> Extract --> Dream --> Review",
        },
        UseCase::Pkm => Spec {
            title: "PKM",
            narrative: "Maintain a local-first personal knowledge vault with graph search and periodic cleanup.",
            commands: "kg index --full --pagerank\nkg search \"datastore\" --mode hybrid\nkg link --orphans\nkg dream --dry-run",
            mermaid: "flowchart LR\n  Index --> Search --> Link --> Dream",
        },
        UseCase::AgentMemory => Spec {
            title: "Agent Memory",
            narrative: "Expose structured recall to coding agents without making generated indexes canonical.",
            commands: "kg index --full\nkg mcp-server --transport stdio\nkg recall --entity BillingService\nkg tokens --by operation --json",
            mermaid: "flowchart LR\n  Index --> MCP --> Recall --> Tokens",
        },
        UseCase::TeamSharing => Spec {
            title: "Team Sharing",
            narrative: "Ship a portable OKF bundle and import it into another vault namespace.",
            commands: "kg validate --okf\nkg ship --out team.okf.tar.gz\nkg pull team.okf.tar.gz --namespace team\nkg validate --okf",
            mermaid: "flowchart LR\n  Validate --> Ship --> Pull --> Validate2",
        },
    }
}

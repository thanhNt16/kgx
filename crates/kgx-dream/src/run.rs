use crate::{context::DreamContext, passes, PassId};
use kgx_core::{diff::ProposedDiff, Result};

#[derive(Debug, Clone)]
pub struct DreamOptions {
    pub passes: Vec<PassId>,
    pub max_iterations: u32,
}

#[derive(Debug)]
pub struct DreamRun {
    pub diffs: Vec<ProposedDiff>,
    pub iterations: u32,
    pub done_signal: bool,
}

pub async fn dream(ctx: &DreamContext<'_>, opts: DreamOptions) -> Result<DreamRun> {
    let mut all: Vec<ProposedDiff> = Vec::new();
    let mut iterations = 0u32;
    let mut done = false;

    for _ in 0..opts.max_iterations.max(1) {
        iterations += 1;
        let mut round = Vec::new();

        for pid in &opts.passes {
            let d = match pid {
                PassId::Dedup => passes::dedup::run(ctx).await?,
                PassId::Contradiction => passes::contradiction::run(ctx).await?,
                PassId::Supersession => passes::supersession::run(ctx).await?,
                PassId::Staleness => passes::staleness::run(ctx).await?,
                PassId::Community => passes::community::run(ctx).await?,
                PassId::OrphanRepair => passes::orphan_repair::run(ctx).await?,
                PassId::OpenQuestions => passes::open_questions::run(ctx).await?,
            };
            round.extend(d);
        }

        // Deduplicate proposals by (pass, file paths) to detect convergence
        let new: Vec<ProposedDiff> = round
            .into_iter()
            .filter(|d| {
                !all.iter()
                    .any(|e| e.pass == d.pass && diff_paths(e) == diff_paths(d))
            })
            .collect();

        if new.is_empty() {
            done = true; // convergence — equivalent to <promise>DONE</promise>
            break;
        }
        all.extend(new);
    }

    Ok(DreamRun {
        diffs: all,
        iterations,
        done_signal: done,
    })
}

fn diff_paths(d: &ProposedDiff) -> Vec<String> {
    d.files.iter().map(|f| f.rel_path.clone()).collect()
}

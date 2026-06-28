use crate::output::emit;
use kgx_cron::{manage, Job};
use std::time::Instant;

pub fn run(
    json: bool,
    action: &str,
    name: Option<String>,
    command: Option<String>,
    calendar: Option<String>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    match action {
        "add" => {
            let job = Job {
                name: name.ok_or_else(|| anyhow::anyhow!("cron add requires a name"))?,
                command: command.unwrap_or_else(|| "kg dream".into()),
                calendar: calendar.unwrap_or_else(|| "*-*-* 03:00:00".into()),
            };
            let files = manage::add(&job)?;
            emit(
                "cron",
                serde_json::json!({"files": files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()}),
                json,
                start,
                |_| println!("wrote {} unit file(s)", files.len()),
            );
        }
        "list" => {
            let jobs = manage::list()?;
            emit(
                "cron",
                serde_json::json!({"jobs": jobs}),
                json,
                start,
                |_| {
                    for job in &jobs {
                        println!("{job}");
                    }
                },
            );
        }
        "enable" => {
            manage::enable(&name.ok_or_else(|| anyhow::anyhow!("cron enable requires a name"))?)?;
            emit(
                "cron",
                serde_json::json!({"enabled": true}),
                json,
                start,
                |_| println!("enabled"),
            );
        }
        "disable" => {
            manage::disable(&name.ok_or_else(|| anyhow::anyhow!("cron disable requires a name"))?)?;
            emit(
                "cron",
                serde_json::json!({"disabled": true}),
                json,
                start,
                |_| println!("disabled"),
            );
        }
        "run" => {
            manage::run_job(&name.ok_or_else(|| anyhow::anyhow!("cron run requires a name"))?)?;
            emit(
                "cron",
                serde_json::json!({"ran": true}),
                json,
                start,
                |_| println!("ran"),
            );
        }
        other => anyhow::bail!("unknown cron action: {other}"),
    }
    Ok(())
}

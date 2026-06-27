use crate::output::emit;
use std::time::Instant;

pub fn run(
    json: bool,
    _okf: bool,
    _links: bool,
    _frontmatter: bool,
    _bitemporal: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let report = kgx_okf::check_okf(&root)?;
    let ok = report.ok;
    emit("validate", report, json, start, |r| {
        if r.ok {
            println!("\u{2714} vault valid (OKF)");
        } else {
            println!("\u{2718} {} violation(s):", r.errors.len());
            for e in &r.errors {
                println!("  [{}] {} \u{2014} {}", e.code, e.path, e.msg);
            }
        }
    });
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

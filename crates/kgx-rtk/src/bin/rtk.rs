use std::io::{self, Read, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const HEAD_TAIL: usize = 20;
const MAX_LINE_LEN: usize = 2000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("compress") => compress(),
        Some("--version") | Some("-v") => {
            println!("rtk {VERSION}");
        }
        _ => {
            eprintln!("Usage: rtk compress");
            eprintln!("       rtk --version");
            std::process::exit(1);
        }
    }
}

fn compress() {
    let mut input = Vec::new();
    if let Err(e) = io::stdin().lock().read_to_end(&mut input) {
        eprintln!("rtk: read error: {e}");
        std::process::exit(1);
    }
    let text = String::from_utf8_lossy(&input);
    let raw_lines: Vec<&str> = text.lines().collect();
    if raw_lines.len() <= HEAD_TAIL * 2 {
        let _ = io::stdout().write_all(text.as_bytes());
        return;
    }
    let mut collapsed: Vec<(&str, usize)> = Vec::new();
    for line in &raw_lines {
        let display = if line.len() > MAX_LINE_LEN {
            &line[..MAX_LINE_LEN]
        } else {
            line
        };
        match collapsed.last_mut() {
            Some((prev, count)) if *prev == display => {
                if *count < usize::MAX {
                    *count += 1;
                }
            }
            _ => collapsed.push((display, 1)),
        }
    }
    let total = collapsed.len();
    if total <= HEAD_TAIL * 2 {
        for (line, count) in &collapsed {
            emit(line, *count);
        }
        return;
    }
    for (line, count) in collapsed.iter().take(HEAD_TAIL) {
        emit(line, *count);
    }
    let suppressed: usize = collapsed[HEAD_TAIL..total - HEAD_TAIL]
        .iter()
        .map(|(_, c)| c)
        .sum();
    let _ = writeln!(io::stdout(), "... [{suppressed} lines suppressed] ...");
    for (line, count) in collapsed.iter().rev().take(HEAD_TAIL).rev() {
        emit(line, *count);
    }
}

fn emit(line: &str, count: usize) {
    if count == 1 {
        let _ = writeln!(io::stdout(), "{line}");
    } else {
        let _ = writeln!(io::stdout(), "{line}"); // first occurrence
        let _ = writeln!(io::stdout(), "  \u{2514} repeated {count} times");
    }
}

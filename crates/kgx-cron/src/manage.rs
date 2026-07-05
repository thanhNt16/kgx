use crate::unit::{render_launchd, render_systemd, Job};
use kgx_core::{KgError, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Platform {
    Linux,
    Macos,
    Other,
}

impl Platform {
    pub fn detect() -> Platform {
        if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "macos") {
            Platform::Macos
        } else {
            Platform::Other
        }
    }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn dirs_config() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".config"))
}

fn systemd_dir() -> PathBuf {
    dirs_config().join("systemd/user")
}

fn launchd_dir() -> PathBuf {
    home().join("Library/LaunchAgents")
}

pub fn add(job: &Job) -> Result<Vec<PathBuf>> {
    // Validate the calendar on every platform before writing any file.
    crate::calendar::parse_calendar(&job.calendar)?;
    match Platform::detect() {
        Platform::Linux => {
            let d = systemd_dir();
            std::fs::create_dir_all(&d).map_err(|e| KgError::Io {
                path: d.display().to_string(),
                source: e,
            })?;
            let (svc, tmr) = render_systemd(job);
            let sp = d.join(format!("kgx-{}.service", job.name));
            let tp = d.join(format!("kgx-{}.timer", job.name));
            std::fs::write(&sp, svc).map_err(|e| KgError::Io {
                path: sp.display().to_string(),
                source: e,
            })?;
            std::fs::write(&tp, tmr).map_err(|e| KgError::Io {
                path: tp.display().to_string(),
                source: e,
            })?;
            Ok(vec![sp, tp])
        }
        Platform::Macos => {
            let d = launchd_dir();
            std::fs::create_dir_all(&d).map_err(|e| KgError::Io {
                path: d.display().to_string(),
                source: e,
            })?;
            let p = d.join(format!("sh.kgx.{}.plist", job.name));
            std::fs::write(&p, render_launchd(job)?).map_err(|e| KgError::Io {
                path: p.display().to_string(),
                source: e,
            })?;
            Ok(vec![p])
        }
        Platform::Other => Err(KgError::Other("unsupported platform for cron".into())),
    }
}

pub fn enable(name: &str) -> Result<()> {
    shell(&platform_cmd("enable", name))
}

pub fn disable(name: &str) -> Result<()> {
    shell(&platform_cmd("disable", name))
}

/// Delete the unit files for `name` (after a best-effort disable).
/// `disable` keeps the files; `remove` deletes them.
pub fn remove(name: &str) -> Result<Vec<PathBuf>> {
    let candidates: Vec<PathBuf> = match Platform::detect() {
        Platform::Linux => vec![
            systemd_dir().join(format!("kgx-{name}.service")),
            systemd_dir().join(format!("kgx-{name}.timer")),
        ],
        Platform::Macos => vec![launchd_dir().join(format!("sh.kgx.{name}.plist"))],
        Platform::Other => return Err(KgError::Other("unsupported platform for cron".into())),
    };
    let existing: Vec<PathBuf> = candidates.into_iter().filter(|p| p.exists()).collect();
    if existing.is_empty() {
        return Err(KgError::Other(format!(
            "no cron unit named '{name}' — see `kg cron list`"
        )));
    }
    let _ = disable(name); // best-effort unload; files may never have been enabled
    let mut deleted = Vec::new();
    for p in existing {
        std::fs::remove_file(&p).map_err(|e| KgError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        deleted.push(p);
    }
    Ok(deleted)
}

pub fn run_job(name: &str) -> Result<()> {
    shell(&platform_cmd("run", name))
}

pub fn list() -> Result<Vec<String>> {
    let dir = match Platform::detect() {
        Platform::Linux => systemd_dir(),
        Platform::Macos => launchd_dir(),
        Platform::Other => return Ok(vec![]),
    };
    Ok(std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| {
                    e.file_name()
                        .to_str()
                        .filter(|n| n.contains("kgx"))
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default())
}

fn platform_cmd(action: &str, name: &str) -> Vec<String> {
    match Platform::detect() {
        Platform::Linux => {
            let unit = format!("kgx-{name}.timer");
            match action {
                "enable" => ["systemctl", "--user", "enable", "--now", &unit]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                "disable" => ["systemctl", "--user", "disable", "--now", &unit]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                _ => {
                    let svc = format!("kgx-{name}.service");
                    ["systemctl", "--user", "start", &svc]
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                }
            }
        }
        Platform::Macos => {
            let label = format!("sh.kgx.{name}");
            let plist = format!(
                "{}/Library/LaunchAgents/{label}.plist",
                std::env::var("HOME").unwrap_or_default()
            );
            match action {
                "enable" => ["launchctl", "load", "-w", &plist]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                "disable" => ["launchctl", "unload", "-w", &plist]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                _ => ["launchctl", "start", &label]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            }
        }
        Platform::Other => vec![],
    }
}

fn shell(argv: &[String]) -> Result<()> {
    if argv.is_empty() {
        return Err(KgError::Other("unsupported platform".into()));
    }
    let st = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .status()
        .map_err(|e| KgError::Other(e.to_string()))?;
    if st.success() {
        Ok(())
    } else {
        Err(KgError::Other(format!("{argv:?} failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_deletes_unit_files_and_errors_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        // Route HOME/XDG so unit dirs land inside the tempdir on any platform.
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config"));

        let job = Job {
            name: "rmtest".into(),
            command: "kg dream".into(),
            calendar: "03:00".into(),
        };
        let written = add(&job).unwrap();
        assert!(!written.is_empty());
        for f in &written {
            assert!(f.exists());
        }

        let deleted = remove("rmtest").unwrap();
        assert_eq!(deleted.len(), written.len());
        for f in &deleted {
            assert!(!f.exists());
        }

        let err = remove("rmtest").unwrap_err().to_string();
        assert!(err.contains("rmtest"), "error should name the missing unit: {err}");
    }
}

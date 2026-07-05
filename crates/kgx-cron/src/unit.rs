#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub name: String,
    pub command: String,
    pub calendar: String,
}

pub fn render_systemd(j: &Job) -> (String, String) {
    let service = format!(
        "[Unit]\nDescription=KGX {name}\n\n[Service]\nType=oneshot\nExecStart={cmd}\n",
        name = j.name,
        cmd = shell_exec(&j.command)
    );
    let timer = format!(
        "[Unit]\nDescription=KGX {name} timer\n\n[Timer]\nOnCalendar={cal}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        name = j.name,
        cal = j.calendar
    );
    (service, timer)
}

fn shell_exec(cmd: &str) -> String {
    format!("/bin/sh -lc '{}'", cmd.replace('\'', "'\\''"))
}

use crate::calendar::{parse_calendar, Schedule};
use kgx_core::Result;

pub fn render_launchd(j: &Job) -> Result<String> {
    let sched = parse_calendar(&j.calendar)?;
    let interval = match sched {
        Schedule::Hourly { minute } => {
            format!("<dict><key>Minute</key><integer>{minute}</integer></dict>")
        }
        Schedule::Daily { hour, minute } => format!(
            "<dict><key>Hour</key><integer>{hour}</integer><key>Minute</key><integer>{minute}</integer></dict>"
        ),
        Schedule::Weekly { weekday, hour, minute } => format!(
            "<dict><key>Weekday</key><integer>{weekday}</integer><key>Hour</key><integer>{hour}</integer><key>Minute</key><integer>{minute}</integer></dict>"
        ),
    };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n\
  <key>Label</key><string>sh.kgx.{name}</string>\n\
  <key>ProgramArguments</key><array><string>/bin/sh</string><string>-lc</string><string>{cmd}</string></array>\n\
  <key>StartCalendarInterval</key>{interval}\n\
</dict></plist>\n",
        name = j.name,
        cmd = j.command,
        interval = interval,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_timer_has_calendar() {
        let j = Job {
            name: "dream-nightly".into(),
            command: "kg dream --max-iterations 3".into(),
            calendar: "*-*-* 03:00:00".into(),
        };
        let (service, timer) = render_systemd(&j);
        assert!(service.contains("ExecStart="));
        assert!(timer.contains("OnCalendar=*-*-* 03:00:00"));
    }

    #[test]
    fn launchd_plist_daily_hh_mm() {
        let j = Job {
            name: "dream-nightly".into(),
            command: "kg dream".into(),
            calendar: "03:00".into(),
        };
        let plist = render_launchd(&j).unwrap();
        assert!(plist.contains("StartCalendarInterval"));
        assert!(plist.contains("<key>Hour</key><integer>3</integer>"));
        assert!(plist.contains("<key>Minute</key><integer>0</integer>"));
    }

    #[test]
    fn launchd_plist_hourly_has_no_hour_key() {
        let j = Job {
            name: "gc".into(),
            command: "kg index".into(),
            calendar: "hourly".into(),
        };
        let plist = render_launchd(&j).unwrap();
        assert!(plist.contains("<key>Minute</key><integer>0</integer>"));
        assert!(
            !plist.contains("<key>Hour</key>"),
            "hourly must omit Hour so launchd fires every hour"
        );
    }

    #[test]
    fn launchd_plist_weekly_has_weekday() {
        let j = Job {
            name: "wk".into(),
            command: "kg dream".into(),
            calendar: "Mon *-*-* 09:30:00".into(),
        };
        let plist = render_launchd(&j).unwrap();
        assert!(plist.contains("<key>Weekday</key><integer>1</integer>"));
        assert!(plist.contains("<key>Hour</key><integer>9</integer>"));
        assert!(plist.contains("<key>Minute</key><integer>30</integer>"));
    }

    #[test]
    fn launchd_rejects_unsupported_instead_of_malformed_plist() {
        let j = Job {
            name: "bad".into(),
            command: "kg dream".into(),
            calendar: "*/5 * * * *".into(),
        };
        assert!(render_launchd(&j).is_err());
    }

    #[test]
    fn systemd_syntax_no_longer_renders_malformed_launchd_hour() {
        let j = Job {
            name: "sysd".into(),
            command: "kg dream".into(),
            calendar: "*-*-* 03:00:00".into(),
        };
        let plist = render_launchd(&j).unwrap();
        assert!(plist.contains("<key>Hour</key><integer>3</integer>"));
        assert!(
            !plist.contains("*-*-*"),
            "raw systemd tokens must never leak into a plist"
        );
    }
}

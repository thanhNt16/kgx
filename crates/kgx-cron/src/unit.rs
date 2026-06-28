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

pub fn render_launchd(j: &Job) -> String {
    let (h, m) = j.calendar.split_once(':').unwrap_or(("3", "0"));
    let hour = h.trim_start_matches('0');
    let minute = m.trim_start_matches('0');
    let hour = if hour.is_empty() { "0" } else { hour };
    let minute = if minute.is_empty() { "0" } else { minute };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n\
  <key>Label</key><string>sh.kgx.{name}</string>\n\
  <key>ProgramArguments</key><array><string>/bin/sh</string><string>-lc</string><string>{cmd}</string></array>\n\
  <key>StartCalendarInterval</key><dict><key>Hour</key><integer>{h}</integer><key>Minute</key><integer>{m}</integer></dict>\n\
</dict></plist>\n",
        name = j.name,
        cmd = j.command,
        h = hour,
        m = minute,
    )
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
    fn launchd_plist_has_calendar_interval() {
        let j = Job {
            name: "dream-nightly".into(),
            command: "kg dream".into(),
            calendar: "03:00".into(),
        };
        let plist = render_launchd(&j);
        assert!(plist.contains("StartCalendarInterval"));
        assert!(plist.contains("<integer>3</integer>"));
    }
}

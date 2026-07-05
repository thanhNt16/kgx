use kgx_core::{KgError, Result};

/// Parsed schedule, normalized to launchd-compatible fields.
/// weekday: 0=Sun, 1=Mon, ... 6=Sat (launchd numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Hourly { minute: u8 },
    Daily { hour: u8, minute: u8 },
    Weekly { weekday: u8, hour: u8, minute: u8 },
}

const SUPPORTED: &str = "supported calendar forms: 'hourly', 'daily', 'weekly', 'HH:MM', '*-*-* HH:MM:SS', '<Mon..Sun> *-*-* HH:MM:SS'";

fn parse_hm(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.split_once(':')?;
    let h: u8 = h.trim().parse().ok()?;
    let m: u8 = m.trim().parse().ok()?;
    (h < 24 && m < 60).then_some((h, m))
}

fn parse_hms(s: &str) -> Option<(u8, u8)> {
    let mut it = s.splitn(3, ':');
    let h: u8 = it.next()?.trim().parse().ok()?;
    let m: u8 = it.next()?.trim().parse().ok()?;
    let sec: u8 = it.next().unwrap_or("0").trim().parse().ok()?;
    (h < 24 && m < 60 && sec < 60).then_some((h, m))
}

fn weekday_num(s: &str) -> Option<u8> {
    match s.to_ascii_lowercase().as_str() {
        "sun" | "sunday" => Some(0),
        "mon" | "monday" => Some(1),
        "tue" | "tuesday" => Some(2),
        "wed" | "wednesday" => Some(3),
        "thu" | "thursday" => Some(4),
        "fri" | "friday" => Some(5),
        "sat" | "saturday" => Some(6),
        _ => None,
    }
}

pub fn parse_calendar(s: &str) -> Result<Schedule> {
    let s = s.trim();
    match s.to_ascii_lowercase().as_str() {
        "hourly" => return Ok(Schedule::Hourly { minute: 0 }),
        "daily" => return Ok(Schedule::Daily { hour: 0, minute: 0 }),
        "weekly" => {
            return Ok(Schedule::Weekly {
                weekday: 1,
                hour: 0,
                minute: 0,
            })
        }
        _ => {}
    }
    // "HH:MM"
    if let Some((h, m)) = parse_hm(s) {
        return Ok(Schedule::Daily { hour: h, minute: m });
    }
    // "*-*-* HH:MM:SS"
    if let Some(rest) = s.strip_prefix("*-*-*") {
        if let Some((h, m)) = parse_hms(rest.trim()) {
            return Ok(Schedule::Daily { hour: h, minute: m });
        }
    }
    // "<Weekday> *-*-* HH:MM:SS"
    if let Some((day, rest)) = s.split_once(' ') {
        if let Some(wd) = weekday_num(day) {
            if let Some(rest) = rest.trim().strip_prefix("*-*-*") {
                if let Some((h, m)) = parse_hms(rest.trim()) {
                    return Ok(Schedule::Weekly {
                        weekday: wd,
                        hour: h,
                        minute: m,
                    });
                }
            }
        }
    }
    Err(KgError::Other(format!(
        "unsupported calendar spec {s:?}; {SUPPORTED}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keywords() {
        assert_eq!(
            parse_calendar("hourly").unwrap(),
            Schedule::Hourly { minute: 0 }
        );
        assert_eq!(
            parse_calendar("daily").unwrap(),
            Schedule::Daily { hour: 0, minute: 0 }
        );
        assert_eq!(
            parse_calendar("weekly").unwrap(),
            Schedule::Weekly {
                weekday: 1,
                hour: 0,
                minute: 0
            }
        );
    }

    #[test]
    fn parses_hh_mm() {
        assert_eq!(
            parse_calendar("03:00").unwrap(),
            Schedule::Daily { hour: 3, minute: 0 }
        );
        assert_eq!(
            parse_calendar("23:59").unwrap(),
            Schedule::Daily {
                hour: 23,
                minute: 59
            }
        );
    }

    #[test]
    fn parses_systemd_daily() {
        assert_eq!(
            parse_calendar("*-*-* 03:00:00").unwrap(),
            Schedule::Daily { hour: 3, minute: 0 }
        );
    }

    #[test]
    fn parses_systemd_weekday() {
        assert_eq!(
            parse_calendar("Mon *-*-* 09:30:00").unwrap(),
            Schedule::Weekly {
                weekday: 1,
                hour: 9,
                minute: 30
            }
        );
        assert_eq!(
            parse_calendar("sun *-*-* 00:00:00").unwrap(),
            Schedule::Weekly {
                weekday: 0,
                hour: 0,
                minute: 0
            }
        );
    }

    #[test]
    fn rejects_unsupported_with_helpful_error() {
        for bad in [
            "*/5 * * * *",
            "monthly",
            "25:00",
            "*-*-* 99:00:00",
            "Mon..Fri 09:00",
        ] {
            let err = parse_calendar(bad).unwrap_err().to_string();
            assert!(
                err.contains("supported calendar forms"),
                "error for {bad:?} should list supported forms, got: {err}"
            );
        }
    }
}

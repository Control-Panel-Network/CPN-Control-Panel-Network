//! Timestamp detection for host log lines.
//!
//! Logs use several formats (common log format, ISO, syslog, ctime). Each is recognised, removed
//! from the message and shown the Norwegian way: `dd/mm/yyyy HH:MM:SS` (24 hour). The time is
//! displayed as written in the log, which is the server time zone.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Stamp {
    /// `dd/mm/yyyy HH:MM:SS`.
    pub fn display(&self) -> String {
        format!(
            "{:02}/{:02}/{:04} {:02}:{:02}:{:02}",
            self.day, self.month, self.year, self.hour, self.minute, self.second
        )
    }

    /// Seconds since the Unix epoch, treating the stamp as UTC (only used for ordering).
    pub fn epoch(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + i64::from(self.hour) * 3_600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }
}

/// A line with its leading or embedded timestamp split off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub stamp: Stamp,
    pub rest: String,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = i64::from(if month <= 2 { year - 1 } else { year });
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Year and month of "now", used to guess the year of syslog lines.
pub fn current_year_month() -> (i32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (yoe + era * 400) as i32 + i32::from(month <= 2);
    (year, month)
}

fn digits(b: &[u8], start: usize, len: usize) -> Option<u32> {
    let slice = b.get(start..start + len)?;
    if !slice.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut n = 0u32;
    for d in slice {
        n = n * 10 + u32::from(d - b'0');
    }
    Some(n)
}

fn month_number(abbr: &str) -> Option<u32> {
    MONTHS
        .iter()
        .position(|m| *m == abbr)
        .map(|i| (i + 1) as u32)
}

fn valid(stamp: Stamp) -> Option<Stamp> {
    (stamp.month >= 1
        && stamp.month <= 12
        && stamp.day >= 1
        && stamp.day <= 31
        && stamp.hour < 24
        && stamp.minute < 60
        && stamp.second < 61)
        .then_some(stamp)
}

fn hms(b: &[u8], at: usize) -> Option<(u32, u32, u32)> {
    if b.get(at + 2) != Some(&b':') || b.get(at + 5) != Some(&b':') {
        return None;
    }
    Some((
        digits(b, at, 2)?,
        digits(b, at + 3, 2)?,
        digits(b, at + 6, 2)?,
    ))
}

/// `2026-10-01T15:02:01.123+0200` or `2026-10-01 02:35:12.123456` at the start of the line.
fn iso(line: &str) -> Option<Split> {
    let b = line.as_bytes();
    if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    if !matches!(b.get(10), Some(b'T') | Some(b' ')) {
        return None;
    }
    let (hour, minute, second) = hms(b, 11)?;
    let stamp = valid(Stamp {
        year: digits(b, 0, 4)? as i32,
        month: digits(b, 5, 2)?,
        day: digits(b, 8, 2)?,
        hour,
        minute,
        second,
    })?;
    let mut end = 19;
    if b.get(end) == Some(&b'.') || b.get(end) == Some(&b',') {
        end += 1;
        while b.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    if b.get(end) == Some(&b'Z') {
        end += 1;
    } else if matches!(b.get(end), Some(b'+') | Some(b'-'))
        && b.get(end + 1).is_some_and(u8::is_ascii_digit)
    {
        end += 1;
        while b.get(end).is_some_and(|c| c.is_ascii_digit() || *c == b':') {
            end += 1;
        }
    }
    Some(Split {
        stamp,
        rest: line[end..].trim_start().to_string(),
    })
}

/// `Oct  1 15:02:01 host ...` (no year) at byte `at`. Returns the stamp and the end offset.
fn syslog_at(line: &str, at: usize, now: (i32, u32), year: Option<i32>) -> Option<(Stamp, usize)> {
    let b = line.as_bytes();
    let month = month_number(line.get(at..at + 3)?)?;
    if b.get(at + 3) != Some(&b' ') {
        return None;
    }
    let day = if b.get(at + 4) == Some(&b' ') {
        digits(b, at + 5, 1)?
    } else {
        digits(b, at + 4, 2)?
    };
    if b.get(at + 6) != Some(&b' ') {
        return None;
    }
    let (hour, minute, second) = hms(b, at + 7)?;
    let year = year.unwrap_or(if month > now.1 { now.0 - 1 } else { now.0 });
    let stamp = valid(Stamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })?;
    Some((stamp, at + 15))
}

/// `Thu Oct  1 15:02:01 2026 [pid 1] ...` (vsftpd, xferlog).
fn ctime(line: &str, now: (i32, u32)) -> Option<Split> {
    let weekday = line.get(..3)?;
    if !WEEKDAYS.contains(&weekday) || line.as_bytes().get(3) != Some(&b' ') {
        return None;
    }
    let (probe, end) = syslog_at(line, 4, now, Some(1970))?;
    let b = line.as_bytes();
    if b.get(end) != Some(&b' ') {
        return None;
    }
    let year = digits(b, end + 1, 4)? as i32;
    let stamp = Stamp { year, ..probe };
    Some(Split {
        stamp,
        rest: line[end + 5..].trim_start().to_string(),
    })
}

/// `[01/Oct/2026:12:00:00 +0200]` anywhere in the line (common log format).
fn clf(line: &str) -> Option<Split> {
    let b = line.as_bytes();
    let mut from = 0usize;
    while let Some(off) = line[from..].find('[') {
        let i = from + off;
        from = i + 1;
        if b.get(i + 3) != Some(&b'/')
            || b.get(i + 7) != Some(&b'/')
            || b.get(i + 12) != Some(&b':')
        {
            continue;
        }
        let Some(month) = line.get(i + 4..i + 7).and_then(month_number) else {
            continue;
        };
        let (Some(day), Some(year)) = (digits(b, i + 1, 2), digits(b, i + 8, 4)) else {
            continue;
        };
        let Some((hour, minute, second)) = hms(b, i + 13) else {
            continue;
        };
        let Some(stamp) = valid(Stamp {
            year: year as i32,
            month,
            day,
            hour,
            minute,
            second,
        }) else {
            continue;
        };
        let close = line[i..].find(']').map(|c| i + c + 1).unwrap_or(i + 21);
        let rest = format!("{} {}", line[..i].trim_end(), line[close..].trim_start());
        return Some(Split {
            stamp,
            rest: rest.trim().to_string(),
        });
    }
    None
}

/// Split the timestamp off a log line. `now` is `(year, month)` for year-less syslog stamps.
pub fn split_stamp(line: &str, now: (i32, u32)) -> Option<Split> {
    if let Some(s) = iso(line) {
        return Some(s);
    }
    if let Some(s) = ctime(line, now) {
        return Some(s);
    }
    if let Some((stamp, end)) = syslog_at(line, 0, now, None) {
        return Some(Split {
            stamp,
            rest: line[end..].trim_start().to_string(),
        });
    }
    clf(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: (i32, u32) = (2026, 10);

    #[test]
    fn clf_is_localised_and_removed() {
        let s = split_stamp(
            r#"203.0.113.9 - - [01/Oct/2026:14:05:09 +0200] "GET / HTTP/1.1" 200 512"#,
            NOW,
        )
        .unwrap();
        assert_eq!(s.stamp.display(), "01/10/2026 14:05:09");
        assert_eq!(s.rest, r#"203.0.113.9 - - "GET / HTTP/1.1" 200 512"#);
    }

    #[test]
    fn iso_and_ols_error_formats() {
        let s = split_stamp("2026-10-01T15:02:01+0200 host sshd[1]: hi", NOW).unwrap();
        assert_eq!(s.stamp.display(), "01/10/2026 15:02:01");
        assert_eq!(s.rest, "host sshd[1]: hi");
        let e = split_stamp("2026-10-01 02:35:12.123456 [NOTICE] [PID] Started", NOW).unwrap();
        assert_eq!(e.stamp.display(), "01/10/2026 02:35:12");
        assert_eq!(e.rest, "[NOTICE] [PID] Started");
    }

    #[test]
    fn syslog_infers_year_and_pads_day() {
        let s = split_stamp("Oct  1 15:02:01 lab postfix/smtpd[9]: connect", NOW).unwrap();
        assert_eq!(s.stamp.display(), "01/10/2026 15:02:01");
        assert_eq!(s.rest, "lab postfix/smtpd[9]: connect");
        let past = split_stamp("Dec 31 23:59:59 lab x", NOW).unwrap();
        assert_eq!(past.stamp.year, 2025);
    }

    #[test]
    fn ctime_format_used_by_vsftpd() {
        let s = split_stamp("Thu Oct  1 15:02:01 2026 [pid 12] OK LOGIN", NOW).unwrap();
        assert_eq!(s.stamp.display(), "01/10/2026 15:02:01");
        assert_eq!(s.rest, "[pid 12] OK LOGIN");
    }

    #[test]
    fn plain_text_has_no_stamp_and_utf8_is_safe() {
        assert!(split_stamp("no stamp here", NOW).is_none());
        assert!(split_stamp("blåbær blåbær blåbær blåbær", NOW).is_none());
        assert!(split_stamp("[99/Zzz/2026:99:99:99 +0000] x", NOW).is_none());
    }

    #[test]
    fn epoch_orders_and_matches_known_value() {
        let a = split_stamp("2026-10-01 00:00:00 a", NOW).unwrap().stamp;
        let b = split_stamp("2026-10-01 00:00:01 b", NOW).unwrap().stamp;
        assert_eq!(b.epoch() - a.epoch(), 1);
        let unix = split_stamp("1970-01-02 00:00:00 x", NOW).unwrap().stamp;
        assert_eq!(unix.epoch(), 86_400);
    }

    #[test]
    fn current_year_month_is_sane() {
        let (y, m) = current_year_month();
        assert!(y >= 2024 && (1..=12).contains(&m));
    }
}

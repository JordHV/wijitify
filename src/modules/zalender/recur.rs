use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use rrule::{RRuleSet, Tz};

use super::model::CalEvent;

const DAY_CODES: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

/// How a new local event repeats. `unit`: 0 day, 1 week, 2 month, 3 quarter, 4 year.
/// `end`: 0 never, 1 on a date, 2 after a count. `weekdays` is Sunday = 0.
#[derive(Clone, Copy, Debug)]
pub struct RepeatDraft {
    pub interval: u32,
    pub unit: u8,
    pub end: u8,
    pub until: Option<NaiveDate>,
    pub count: u32,
    pub weekdays: [bool; 7],
    /// Month and quarter: true = nth weekday (3rd Saturday), false = day of month.
    pub by_weekday: bool,
}

pub fn parse_tz(name: &str) -> chrono_tz::Tz {
    name.parse().unwrap_or(chrono_tz::UTC)
}

/// Expand an RRULE string into occurrence starts within [from, to].
/// Returns empty on parse failure (bad feed should not crash the app).
pub fn expand_rrule(
    dtstart: DateTime<Utc>,
    rrule_line: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    let start = dtstart.with_timezone(&Tz::UTC);
    let body = rrule_body(rrule_line);
    let parsed = format!("DTSTART:{}\nRRULE:{}", start.format("%Y%m%dT%H%M%SZ"), body);
    match parsed.parse() {
        Ok(set) => dates_between(set, from, to),
        Err(e) => {
            tracing::warn!("rrule parse failed: {e}");
            Vec::new()
        }
    }
}

/// Same expansion, but BYDAY / BYMONTHDAY follow `tz` (the calendar timezone).
pub fn expand_rrule_tz(
    dtstart: DateTime<Utc>,
    rrule_line: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    tz: chrono_tz::Tz,
) -> Vec<DateTime<Utc>> {
    let local = dtstart.with_timezone(&tz);
    let parsed = format!(
        "DTSTART;TZID={}:{}\nRRULE:{}",
        tz.name(),
        local.format("%Y%m%dT%H%M%S"),
        rrule_body(rrule_line)
    );
    match parsed.parse() {
        Ok(set) => dates_between(set, from, to),
        Err(e) => {
            tracing::warn!("rrule tz parse failed: {e}");
            expand_rrule(dtstart, rrule_line, from, to)
        }
    }
}

/// One stored local event becomes the occurrences inside the window.
/// A row with no rule is returned as itself.
pub fn expand_stored(
    ev: &CalEvent,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    tz: chrono_tz::Tz,
) -> Vec<CalEvent> {
    let Some(rule) = ev.rrule.as_deref().filter(|r| !r.is_empty()) else {
        return vec![ev.clone()];
    };
    let duration = ev.end - ev.start;
    expand_rrule_tz(ev.start, rule, from, to, tz)
        .into_iter()
        .map(|occ| {
            let mut copy = ev.clone();
            copy.recurrence_id = Some(occ.format("%Y%m%dT%H%M%SZ").to_string());
            copy.start = occ;
            copy.end = occ + duration;
            copy.rrule = None;
            copy
        })
        .collect()
}

pub fn rrule_from_draft(start: NaiveDate, draft: &RepeatDraft, tz: chrono_tz::Tz) -> String {
    let interval = draft.interval.clamp(1, 99);
    let mut parts: Vec<String> = Vec::new();
    match draft.unit {
        1 => {
            parts.push("FREQ=WEEKLY".into());
            if interval != 1 {
                parts.push(format!("INTERVAL={interval}"));
            }
            parts.push(format!("BYDAY={}", byday_list(start, &draft.weekdays)));
        }
        2 => {
            parts.push("FREQ=MONTHLY".into());
            if interval != 1 {
                parts.push(format!("INTERVAL={interval}"));
            }
            parts.push(month_part(start, draft.by_weekday));
        }
        3 => {
            parts.push("FREQ=MONTHLY".into());
            parts.push(format!("INTERVAL={}", interval * 3));
            parts.push(month_part(start, draft.by_weekday));
        }
        4 => {
            parts.push("FREQ=YEARLY".into());
            if interval != 1 {
                parts.push(format!("INTERVAL={interval}"));
            }
        }
        _ => {
            parts.push("FREQ=DAILY".into());
            if interval != 1 {
                parts.push(format!("INTERVAL={interval}"));
            }
        }
    }
    match draft.end {
        1 => {
            if let Some(day) = draft.until {
                parts.push(format!("UNTIL={}", until_utc(day, tz)));
            }
        }
        2 => parts.push(format!("COUNT={}", draft.count.clamp(1, 999))),
        _ => {}
    }
    parts.join(";")
}

pub fn repeat_summary(start: NaiveDate, draft: &RepeatDraft) -> String {
    let interval = draft.interval.clamp(1, 99);
    let mut text = if interval == 1 {
        match draft.unit {
            1 => "Every week".to_string(),
            2 => "Every month".to_string(),
            3 => "Every quarter".to_string(),
            4 => "Every year".to_string(),
            _ => "Every day".to_string(),
        }
    } else {
        let unit = match draft.unit {
            1 => "weeks",
            2 => "months",
            3 => "quarters",
            4 => "years",
            _ => "days",
        };
        format!("Every {interval} {unit}")
    };
    match draft.unit {
        1 => {
            const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            let picked: Vec<&str> = draft
                .weekdays
                .iter()
                .enumerate()
                .filter(|(_, on)| **on)
                .map(|(i, _)| NAMES[i])
                .collect();
            if !picked.is_empty() {
                text.push_str(" on ");
                text.push_str(&picked.join(", "));
            }
        }
        2 if draft.by_weekday => {
            text.push(' ');
            text.push_str(&on_the_weekday(start));
        }
        2 => {
            text.push_str(&format!(" on day {}", start.day()));
        }
        3 if draft.by_weekday => {
            let month = (start.month() - 1) % 3 + 1;
            text.push(' ');
            text.push_str(&on_the_weekday(start));
            text.push_str(" of the ");
            text.push_str(&ordinal(month));
            text.push_str(" month of each quarter");
        }
        3 => {
            text.push_str(&format!(" on day {} (or the closest day)", start.day()));
        }
        _ => {}
    }
    match draft.end {
        1 => {
            if let Some(day) = draft.until {
                text.push_str(&format!(", until {}", day.format("%b %d, %Y")));
            }
        }
        2 => {
            let n = draft.count.clamp(1, 999);
            let word = if n == 1 { "time" } else { "times" };
            text.push_str(&format!(", {n} {word}"));
        }
        _ => {}
    }
    text
}

/// (day-of-month label, nth-weekday label) for the monthly choices.
pub fn month_choice_labels(start: NaiveDate) -> (String, String) {
    (
        format!("On day {}", start.day()),
        format!(
            "On the {} {}",
            ordinal((start.day() - 1) / 7 + 1),
            start.format("%A")
        ),
    )
}

/// (day-of-month label, nth-weekday of this month-of-quarter label).
pub fn quarter_choice_labels(start: NaiveDate) -> (String, String) {
    let month = (start.month() - 1) % 3 + 1;
    (
        format!("On day {} of each quarter, or the closest day", start.day()),
        format!(
            "On the {} {} of the {} month of each quarter",
            ordinal((start.day() - 1) / 7 + 1),
            start.format("%A"),
            ordinal(month)
        ),
    )
}

fn rrule_body(rrule_line: &str) -> String {
    rrule_line
        .trim()
        .trim_start_matches("RRULE:")
        .trim()
        .to_string()
}

fn dates_between(set: RRuleSet, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let after = from.with_timezone(&Tz::UTC) - Duration::seconds(1);
    set.after(after)
        .before(to.with_timezone(&Tz::UTC))
        .all(512)
        .dates
        .into_iter()
        .map(|d| d.with_timezone(&Utc))
        .filter(|d| *d >= from && *d <= to)
        .collect()
}

fn byday_list(start: NaiveDate, days: &[bool; 7]) -> String {
    let mut codes: Vec<&str> = days
        .iter()
        .enumerate()
        .filter(|(_, on)| **on)
        .map(|(i, _)| DAY_CODES[i])
        .collect();
    if codes.is_empty() {
        codes.push(DAY_CODES[start.weekday().num_days_from_sunday() as usize]);
    }
    codes.join(",")
}

/// Day-of-month, or the closest earlier day when the month is shorter (29–31).
fn month_part(start: NaiveDate, by_weekday: bool) -> String {
    if by_weekday {
        let ord = (start.day() - 1) / 7 + 1;
        let code = DAY_CODES[start.weekday().num_days_from_sunday() as usize];
        return format!("BYDAY={ord}{code}");
    }
    let day = start.day();
    if day <= 28 {
        format!("BYMONTHDAY={day}")
    } else {
        let list = (28..=day)
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("BYMONTHDAY={list};BYSETPOS=-1")
    }
}

fn on_the_weekday(start: NaiveDate) -> String {
    format!(
        "on the {} {}",
        ordinal((start.day() - 1) / 7 + 1),
        start.format("%A")
    )
}

fn ordinal(n: u32) -> String {
    let suffix = match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

fn until_utc(day: NaiveDate, tz: chrono_tz::Tz) -> String {
    let Some(naive) = day.and_hms_opt(23, 59, 59) else {
        return format!("{}T235959Z", day.format("%Y%m%d"));
    };
    let utc = match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) | chrono::LocalResult::Ambiguous(dt, _) => {
            dt.with_timezone(&Utc)
        }
        chrono::LocalResult::None => naive.and_utc(),
    };
    utc.format("%Y%m%dT%H%M%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn draft(unit: u8, by_weekday: bool) -> RepeatDraft {
        RepeatDraft {
            interval: 1,
            unit,
            end: 0,
            until: None,
            count: 1,
            weekdays: [false; 7],
            by_weekday,
        }
    }

    fn ny() -> chrono_tz::Tz {
        chrono_tz::America::New_York
    }

    fn local_days(occ: &[DateTime<Utc>], tz: chrono_tz::Tz) -> Vec<NaiveDate> {
        occ.iter()
            .map(|d| d.with_timezone(&tz).date_naive())
            .collect()
    }

    #[test]
    fn weekly_expands() {
        let start = Utc.with_ymd_and_hms(2026, 1, 5, 15, 0, 0).unwrap();
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let occ = expand_rrule(start, "FREQ=WEEKLY;COUNT=4", from, to);
        assert_eq!(occ.len(), 4);
    }

    #[test]
    fn daily_count() {
        let tz = ny();
        let day = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let mut spec = draft(0, false);
        spec.end = 2;
        spec.count = 3;
        let rule = rrule_from_draft(day, &spec, tz);
        let start = tz
            .with_ymd_and_hms(2026, 1, 5, 9, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let occ = expand_rrule_tz(
            start,
            &rule,
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap(),
            tz,
        );
        assert_eq!(
            local_days(&occ, tz),
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
                NaiveDate::from_ymd_opt(2026, 1, 7).unwrap(),
            ]
        );
    }

    #[test]
    fn weekly_keeps_local_weekday_across_utc_midnight() {
        let tz = ny();
        let day = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let mut spec = draft(1, false);
        spec.weekdays[6] = true;
        spec.end = 2;
        spec.count = 3;
        let rule = rrule_from_draft(day, &spec, tz);
        let start = tz
            .with_ymd_and_hms(2026, 10, 24, 21, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let occ = expand_rrule_tz(
            start,
            &rule,
            Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap(),
            tz,
        );
        assert_eq!(
            local_days(&occ, tz),
            vec![
                NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
                NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
            ]
        );
    }

    #[test]
    fn monthly_third_saturday() {
        let tz = ny();
        let day = NaiveDate::from_ymd_opt(2026, 10, 17).unwrap();
        let mut spec = draft(2, true);
        spec.end = 2;
        spec.count = 2;
        let rule = rrule_from_draft(day, &spec, tz);
        assert!(rule.contains("BYDAY=3SA"));
        let start = tz
            .with_ymd_and_hms(2026, 10, 17, 9, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let occ = expand_rrule_tz(
            start,
            &rule,
            Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap(),
            tz,
        );
        assert_eq!(
            local_days(&occ, tz),
            vec![
                NaiveDate::from_ymd_opt(2026, 10, 17).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
            ]
        );
        assert_eq!(
            repeat_summary(day, &spec),
            "Every month on the 3rd Saturday, 2 times"
        );
    }

    #[test]
    fn quarterly_closest_day_and_nth_weekday() {
        let tz = ny();
        let jan31 = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let mut by_day = draft(3, false);
        by_day.end = 2;
        by_day.count = 4;
        let rule = rrule_from_draft(jan31, &by_day, tz);
        let start = tz
            .with_ymd_and_hms(2026, 1, 31, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let occ = expand_rrule_tz(
            start,
            &rule,
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 11, 15, 0, 0, 0).unwrap(),
            tz,
        );
        assert_eq!(
            local_days(&occ, tz),
            vec![
                NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 31).unwrap(),
                NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
            ]
        );

        // May 16 2026 is the 3rd Saturday, 2nd month of the quarter.
        let may = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();
        let mut by_wd = draft(3, true);
        by_wd.end = 2;
        by_wd.count = 4;
        let rule = rrule_from_draft(may, &by_wd, tz);
        assert!(rule.contains("INTERVAL=3"));
        assert!(rule.contains("BYDAY=3SA"));
        let start = tz
            .with_ymd_and_hms(2026, 5, 16, 12, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let occ = expand_rrule_tz(
            start,
            &rule,
            Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2027, 3, 1, 0, 0, 0).unwrap(),
            tz,
        );
        assert_eq!(
            local_days(&occ, tz),
            vec![
                NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(),
                NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
                NaiveDate::from_ymd_opt(2027, 2, 20).unwrap(),
            ]
        );
    }
}

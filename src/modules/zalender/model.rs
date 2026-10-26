use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalEvent {
    /// Raw ICS UID (series identity).
    pub uid: String,
    /// Present for a recurring occurrence; None for a single event or series row.
    #[serde(default)]
    pub recurrence_id: Option<String>,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    pub feed_id: String,
    pub feed_name: String,
    pub color: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub location: String,
    /// Series rule for a stored local event. Occurrences leave this empty.
    #[serde(default)]
    pub rrule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarSnapshot {
    pub schema_version: u32,
    pub generated_at: DateTime<Utc>,
    pub events: Vec<CalEvent>,
    /// Precomputed per-day lists for the display window.
    pub by_day: BTreeMap<NaiveDate, Vec<usize>>,
}

impl CalendarSnapshot {
    pub const SCHEMA: u32 = 2;

    pub fn from_events(
        events: Vec<CalEvent>,
        window_start: NaiveDate,
        window_end: NaiveDate,
    ) -> Self {
        let mut events = events;
        events.sort_by_key(|e| e.start);

        let mut by_day: BTreeMap<NaiveDate, Vec<usize>> = BTreeMap::new();
        for (idx, ev) in events.iter().enumerate() {
            let start_day = ev.start.date_naive();
            let end_day = if ev.all_day {
                // Exclusive DTEND for all-day: last day is end - 1 day when end > start.
                let end = ev.end.date_naive();
                if end > start_day {
                    end - Duration::days(1)
                } else {
                    start_day
                }
            } else {
                ev.end.date_naive()
            };
            let mut d = start_day.max(window_start);
            let last = end_day.min(window_end);
            while d <= last {
                by_day.entry(d).or_default().push(idx);
                d += Duration::days(1);
            }
        }

        Self {
            schema_version: Self::SCHEMA,
            generated_at: Utc::now(),
            events,
            by_day,
        }
    }

    pub fn events_on(&self, day: NaiveDate) -> Vec<&CalEvent> {
        let mut evs: Vec<&CalEvent> = self
            .by_day
            .get(&day)
            .map(|idxs| idxs.iter().filter_map(|i| self.events.get(*i)).collect())
            .unwrap_or_default();
        // All-day first, then timed by start — keeps month/day stacks gap-free and predictable.
        evs.sort_by(|a, b| {
            b.all_day
                .cmp(&a.all_day)
                .then_with(|| a.start.cmp(&b.start))
        });
        evs
    }
}

/// `week_start`: chrono Monday-based index of the first weekday (0=Mon … 6=Sun).
pub fn month_grid(
    year: i32,
    month: u32,
    today: NaiveDate,
    week_start: u32,
) -> Vec<(NaiveDate, bool, bool)> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
    let start_offset = days_from_week_start(first.weekday(), week_start) as i64;
    let grid_start = first - Duration::days(start_offset);
    (0..42)
        .map(|i| {
            let d = grid_start + Duration::days(i);
            (d, d.month() == month && d.year() == year, d == today)
        })
        .collect()
}

pub fn days_from_week_start(day: Weekday, week_start: u32) -> u32 {
    let day_num = day.num_days_from_monday();
    let start = week_start % 7;
    (day_num + 7 - start) % 7
}

#[allow(dead_code)]
pub fn week_labels(week_start: u32) -> [&'static str; 7] {
    const ALL: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let start = (week_start % 7) as usize;
    let mut out = [""; 7];
    for i in 0..7 {
        out[i] = ALL[(start + i) % 7];
    }
    out
}

/// First day of the week containing `day`.
#[allow(dead_code)]
pub fn start_of_week(day: NaiveDate, week_start: u32) -> NaiveDate {
    let offset = days_from_week_start(day.weekday(), week_start) as i64;
    day - Duration::days(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunday_week_start_october_2026() {
        // 1 Oct 2026 is Thursday. Sunday-first (6) grid starts 27 Sep.
        let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let cells = month_grid(2026, 10, today, 6);
        assert_eq!(cells.len(), 42);
        assert_eq!(cells[0].0, NaiveDate::from_ymd_opt(2026, 9, 27).unwrap());
        assert!(!cells[0].1);
        assert_eq!(cells[4].0.day(), 1);
        assert!(cells[4].1);
    }

    #[test]
    fn monday_week_start_october_2026() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let cells = month_grid(2026, 10, today, 0);
        assert_eq!(cells[0].0, NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());
    }

    #[test]
    fn series_key_fields() {
        let e = CalEvent {
            uid: "abc".into(),
            recurrence_id: Some("20261015T090000Z".into()),
            title: "Meet".into(),
            start: Utc::now(),
            end: Utc::now(),
            all_day: false,
            feed_id: "feed-1".into(),
            feed_name: "Work".into(),
            color: "#3b82f6".into(),
            description: String::new(),
            location: String::new(),
            rrule: None,
        };
        assert_eq!(e.uid, "abc");
        assert!(e.recurrence_id.is_some());
        assert_eq!(e.feed_id, "feed-1");
    }

    #[test]
    fn events_on_all_day_before_timed() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let timed = CalEvent {
            uid: "t".into(),
            recurrence_id: None,
            title: "Timed".into(),
            start: DateTime::parse_from_rfc3339("2026-10-21T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            end: DateTime::parse_from_rfc3339("2026-10-21T09:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            all_day: false,
            feed_id: "f".into(),
            feed_name: "f".into(),
            color: "#000".into(),
            description: String::new(),
            location: String::new(),
            rrule: None,
        };
        let all_day = CalEvent {
            uid: "a".into(),
            recurrence_id: None,
            title: "All day".into(),
            start: DateTime::parse_from_rfc3339("2026-10-21T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            end: DateTime::parse_from_rfc3339("2026-10-22T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            all_day: true,
            feed_id: "f".into(),
            feed_name: "f".into(),
            color: "#000".into(),
            description: String::new(),
            location: String::new(),
            rrule: None,
        };
        // Insert timed first so index order alone would put it first.
        let snap = CalendarSnapshot::from_events(vec![timed, all_day], day, day);
        let on = snap.events_on(day);
        assert_eq!(on.len(), 2);
        assert!(on[0].all_day);
        assert!(!on[1].all_day);
    }
}

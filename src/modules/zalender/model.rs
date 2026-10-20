use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalEvent {
    pub uid: String,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    pub feed_name: String,
    pub color: String,
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
    pub const SCHEMA: u32 = 1;

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
        self.by_day
            .get(&day)
            .map(|idxs| idxs.iter().filter_map(|i| self.events.get(*i)).collect())
            .unwrap_or_default()
    }
}

pub fn month_grid(year: i32, month: u32, today: NaiveDate) -> Vec<(NaiveDate, bool, bool)> {
    // Monday-first cells: (date, in_month, is_today)
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
    let start_offset = first.weekday().num_days_from_monday() as i64;
    let grid_start = first - Duration::days(start_offset);
    (0..42)
        .map(|i| {
            let d = grid_start + Duration::days(i);
            (d, d.month() == month && d.year() == year, d == today)
        })
        .collect()
}

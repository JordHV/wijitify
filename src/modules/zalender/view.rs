use std::collections::HashMap;

use chrono::{Datelike, Duration, Months, NaiveDate};

use crate::services::config::{ViewUnit, WeekStart};

use super::model::{self, start_of_week};

pub fn week_start_num(ws: WeekStart) -> u32 {
    ws.as_chrono_num()
}

pub fn shift_anchor(anchor: NaiveDate, unit: ViewUnit, delta: i32) -> NaiveDate {
    match unit {
        ViewUnit::Day => anchor + Duration::days(i64::from(delta)),
        ViewUnit::Week | ViewUnit::Agenda => {
            // Agenda count is weeks; prev/next steps by one week (caller scales via count).
            anchor + Duration::days(i64::from(delta) * 7)
        }
        ViewUnit::Month => {
            if delta >= 0 {
                anchor
                    .checked_add_months(Months::new(delta as u32))
                    .unwrap_or(anchor)
            } else {
                anchor
                    .checked_sub_months(Months::new((-delta) as u32))
                    .unwrap_or(anchor)
            }
        }
        ViewUnit::Year => {
            let y = anchor.year() + delta;
            NaiveDate::from_ymd_opt(y, anchor.month(), anchor.day().min(28)).unwrap_or(anchor)
        }
    }
}

pub fn title_for_view(
    anchor: NaiveDate,
    _unit: ViewUnit,
    _count: u32,
    _week_start: WeekStart,
) -> String {
    // Every view uses the same header: full month name and year of the date in view.
    anchor.format("%B %Y").to_string()
}

pub fn month_grid(
    year: i32,
    month: u32,
    today: NaiveDate,
    week_start: WeekStart,
) -> Vec<(NaiveDate, bool, bool)> {
    model::month_grid(year, month, today, week_start_num(week_start))
}

/// Week rows for a month preset.
///
/// `from_this_week`: `week_count` rows starting on the viewed week (1 = that week only).
/// Otherwise: from the week of the 1st through the viewed week, plus `week_count - 1`
/// weeks after it. Count 1 ends on the viewed week; count 2 includes the next week.
pub fn month_week_span(
    anchor: NaiveDate,
    week_count: u32,
    from_this_week: bool,
    week_start: WeekStart,
) -> (NaiveDate, u32) {
    let ws = week_start_num(week_start);
    let week_count = week_count.clamp(1, 6);
    if from_this_week {
        return (start_of_week(anchor, ws), week_count);
    }
    let first = NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), 1).unwrap_or(anchor);
    let start = start_of_week(first, ws);
    let anchor_week = start_of_week(anchor, ws);
    let through = anchor_week + Duration::days(7 * i64::from(week_count - 1));
    let days = (through - start).num_days();
    let rows = if days < 0 {
        week_count
    } else {
        (days / 7) as u32 + 1
    };
    (start, rows.max(1))
}

pub fn week_labels(week_start: WeekStart) -> [&'static str; 7] {
    model::week_labels(week_start_num(week_start))
}

/// Identity of one event occurrence. Same key on consecutive columns is one bar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventKey {
    pub uid: String,
    pub recurrence_id: String,
    pub feed_id: String,
}

/// One event on one column of a day strip (a week row, or the day/week header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripInput {
    pub key: EventKey,
    pub all_day: bool,
    /// Inclusive day numbers (`NaiveDate::num_days_from_ce`), for continuation flags.
    pub start: i64,
    pub end: i64,
    pub sort: i32,
}

/// Where to draw that input inside the strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripPlacement {
    /// Columns this bar covers. 0 = continuation; the first column draws the bar.
    pub span_cols: i32,
    pub lane: i32,
    /// Event began before this strip, so the bar's left end stays square.
    pub cont_left: bool,
    /// Event continues after this strip, so the bar's right end stays square.
    pub cont_right: bool,
    /// Lane of the previous event on this day when sorted by lane, or -1.
    pub prev_lane: i32,
    /// Index of this event on its day when sorted by lane.
    pub lane_order: i32,
}

struct SpanItem {
    all_day: bool,
    start_col: usize,
    end_col: usize,
    start: i64,
    end: i64,
    sort: i32,
    key: EventKey,
    refs: Vec<(usize, usize)>,
    lane: i32,
}

/// Pack a horizontal strip of days into lanes.
///
/// All-day events that sit on consecutive columns become one bar. Timed events
/// stay one column wide and, when `pack_timed` is set, fill lanes the bars leave open.
pub fn place_strip(
    columns: &[Vec<StripInput>],
    days: &[i64],
    pack_timed: bool,
) -> Vec<Vec<StripPlacement>> {
    let mut placed: Vec<Vec<StripPlacement>> = columns
        .iter()
        .map(|col| col.iter().map(blank_placement).collect())
        .collect();

    let mut items = all_day_runs(columns);
    if pack_timed {
        for (col, events) in columns.iter().enumerate() {
            for (idx, ev) in events.iter().enumerate() {
                if ev.all_day {
                    continue;
                }
                items.push(SpanItem {
                    all_day: false,
                    start_col: col,
                    end_col: col,
                    start: ev.start,
                    end: ev.end,
                    sort: ev.sort,
                    key: ev.key.clone(),
                    refs: vec![(col, idx)],
                    lane: 0,
                });
            }
        }
    }

    items.sort_by(|a, b| {
        b.all_day
            .cmp(&a.all_day)
            .then(a.start_col.cmp(&b.start_col))
            .then(b.end_col.cmp(&a.end_col))
            .then(a.sort.cmp(&b.sort))
            .then_with(|| a.key.uid.cmp(&b.key.uid))
            .then_with(|| a.key.recurrence_id.cmp(&b.key.recurrence_id))
            .then_with(|| a.key.feed_id.cmp(&b.key.feed_id))
    });
    assign_lanes(&mut items);

    for item in &items {
        let day_first = days.get(item.start_col).copied().unwrap_or(item.start);
        let day_last = days.get(item.end_col).copied().unwrap_or(item.end);
        let span = (item.end_col - item.start_col + 1) as i32;
        for (n, &(col, idx)) in item.refs.iter().enumerate() {
            let slot = &mut placed[col][idx];
            slot.lane = item.lane;
            if n == 0 {
                slot.span_cols = span;
                slot.cont_left = item.all_day && item.start < day_first;
                slot.cont_right = item.all_day && item.end > day_last;
            } else {
                slot.span_cols = 0;
            }
        }
    }

    for (col, events) in columns.iter().enumerate() {
        let mut order: Vec<usize> = (0..events.len())
            .filter(|&idx| events[idx].all_day || pack_timed)
            .collect();
        order.sort_by(|&a, &b| {
            placed[col][a]
                .lane
                .cmp(&placed[col][b].lane)
                .then(a.cmp(&b))
        });
        for (ord, &idx) in order.iter().enumerate() {
            placed[col][idx].lane_order = ord as i32;
            placed[col][idx].prev_lane = if ord == 0 {
                -1
            } else {
                placed[col][order[ord - 1]].lane
            };
        }
    }

    placed
}

fn blank_placement(ev: &StripInput) -> StripPlacement {
    StripPlacement {
        span_cols: if ev.all_day { 0 } else { 1 },
        lane: 0,
        cont_left: false,
        cont_right: false,
        prev_lane: -1,
        lane_order: 0,
    }
}

fn all_day_runs(columns: &[Vec<StripInput>]) -> Vec<SpanItem> {
    let mut cols_of: HashMap<EventKey, Vec<usize>> = HashMap::new();
    for (col, events) in columns.iter().enumerate() {
        for ev in events.iter().filter(|ev| ev.all_day) {
            let cols = cols_of.entry(ev.key.clone()).or_default();
            if cols.last().copied() != Some(col) {
                cols.push(col);
            }
        }
    }

    let mut items = Vec::new();
    for (key, cols) in cols_of {
        let Some(sample) = columns
            .iter()
            .flatten()
            .find(|ev| ev.all_day && ev.key == key)
        else {
            continue;
        };
        let mut run_start = 0;
        for i in 1..=cols.len() {
            if i == cols.len() || cols[i] != cols[i - 1] + 1 {
                let run = &cols[run_start..i];
                let refs = run
                    .iter()
                    .filter_map(|&col| {
                        columns[col]
                            .iter()
                            .position(|ev| ev.all_day && ev.key == key)
                            .map(|idx| (col, idx))
                    })
                    .collect::<Vec<_>>();
                if let (Some(&start_col), Some(&end_col)) = (run.first(), run.last()) {
                    items.push(SpanItem {
                        all_day: true,
                        start_col,
                        end_col,
                        start: sample.start,
                        end: sample.end,
                        sort: sample.sort,
                        key: key.clone(),
                        refs,
                        lane: 0,
                    });
                }
                run_start = i;
            }
        }
    }
    items
}

fn assign_lanes(items: &mut [SpanItem]) {
    let mut lanes: Vec<Vec<(usize, usize)>> = Vec::new();
    for item in items.iter_mut() {
        let found = lanes.iter().position(|occ| {
            occ.iter()
                .all(|&(start, end)| item.end_col < start || item.start_col > end)
        });
        let lane = match found {
            Some(lane) => lane,
            None => {
                lanes.push(Vec::new());
                lanes.len() - 1
            }
        };
        lanes[lane].push((item.start_col, item.end_col));
        item.lane = lane as i32;
    }
}

/// Column indexes in week-start order that `show_days` keeps (Sunday = index 0).
/// Empty input shows every column so a preset cannot blank the grid.
pub fn shown_week_columns(week_start: WeekStart, show_days: &[bool; 7]) -> Vec<usize> {
    let start = week_start.as_chrono_num() as usize;
    let mut cols: Vec<usize> = (0..7)
        .filter(|col| {
            let mon_based = (start + *col) % 7;
            let sun_index = (mon_based + 1) % 7;
            show_days.get(sun_index).copied().unwrap_or(true)
        })
        .collect();
    if cols.is_empty() {
        cols.extend(0..7);
    }
    cols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_and_zoom() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        assert_eq!(
            shift_anchor(d, ViewUnit::Day, 1),
            NaiveDate::from_ymd_opt(2026, 10, 16).unwrap()
        );
        assert_eq!(
            shift_anchor(d, ViewUnit::Week, -1),
            NaiveDate::from_ymd_opt(2026, 10, 8).unwrap()
        );
        let title = title_for_view(d, ViewUnit::Month, 1, WeekStart::Sunday);
        assert_eq!(title, "October 2026");
        assert_eq!(
            title_for_view(d, ViewUnit::Week, 2, WeekStart::Sunday),
            "October 2026"
        );
        assert_eq!(
            title_for_view(d, ViewUnit::Day, 1, WeekStart::Sunday),
            "October 2026"
        );
    }

    #[test]
    fn hidden_weekend_drops_sunday_columns() {
        let mut show = [true; 7];
        show[0] = false;
        show[6] = false;
        let cols = shown_week_columns(WeekStart::Sunday, &show);
        assert_eq!(cols, vec![1, 2, 3, 4, 5]);
        let cols = shown_week_columns(WeekStart::Monday, &show);
        assert_eq!(cols, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn month_span_from_this_week_is_the_week_count() {
        let anchor = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let (start, rows) = month_week_span(anchor, 1, true, WeekStart::Sunday);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 10, 18).unwrap());
        assert_eq!(rows, 1);
        let (start, rows) = month_week_span(anchor, 2, true, WeekStart::Sunday);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 10, 18).unwrap());
        assert_eq!(rows, 2);
        assert_eq!(
            start + Duration::days(13),
            NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()
        );
    }

    #[test]
    fn month_span_from_month_start_runs_through_viewed_week() {
        // 1 Oct 2026 is Thursday; Sunday-first week of the 1st starts 27 Sep.
        // 22 Oct is in the week of 18 Oct. Count 1 ends that week (4 rows).
        // Count 2 adds the week of 25 Oct (5 rows).
        let anchor = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let (start, rows) = month_week_span(anchor, 1, false, WeekStart::Sunday);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 9, 27).unwrap());
        assert_eq!(rows, 4);
        let (start, rows) = month_week_span(anchor, 2, false, WeekStart::Sunday);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 9, 27).unwrap());
        assert_eq!(rows, 5);
    }

    fn key(id: &str) -> EventKey {
        EventKey {
            uid: id.into(),
            recurrence_id: String::new(),
            feed_id: "f".into(),
        }
    }

    fn all_day(id: &str, start: i64, end: i64) -> StripInput {
        StripInput {
            key: key(id),
            all_day: true,
            start,
            end,
            sort: 0,
        }
    }

    fn timed(id: &str, day: i64, sort: i32) -> StripInput {
        StripInput {
            key: key(id),
            all_day: false,
            start: day,
            end: day,
            sort,
        }
    }

    #[test]
    fn multiday_all_day_is_one_bar() {
        let columns = vec![
            vec![all_day("trip", 10, 12)],
            vec![all_day("trip", 10, 12), all_day("meet", 11, 11)],
            vec![all_day("trip", 10, 12)],
        ];
        let placed = place_strip(&columns, &[10, 11, 12], false);
        assert_eq!(placed[0][0].span_cols, 3);
        assert_eq!(placed[0][0].lane, 0);
        assert!(!placed[0][0].cont_left);
        assert!(!placed[0][0].cont_right);
        assert_eq!(placed[1][0].span_cols, 0);
        assert_eq!(placed[1][0].lane, 0);
        assert_eq!(placed[2][0].span_cols, 0);
        assert_eq!(placed[1][1].span_cols, 1);
        assert_eq!(placed[1][1].lane, 1);
        assert_eq!(placed[1][1].prev_lane, 0);
        assert_eq!(placed[1][1].lane_order, 1);
    }

    #[test]
    fn span_square_ends_when_it_continues_outside_the_strip() {
        let columns = vec![vec![all_day("trip", 9, 14)], vec![all_day("trip", 9, 14)]];
        let placed = place_strip(&columns, &[10, 11], false);
        assert_eq!(placed[0][0].span_cols, 2);
        assert!(placed[0][0].cont_left);
        assert!(placed[0][0].cont_right);
        assert_eq!(placed[1][0].span_cols, 0);
    }

    #[test]
    fn hidden_weekday_still_connects_the_visible_columns() {
        let columns = vec![vec![all_day("trip", 10, 12)], vec![all_day("trip", 10, 12)]];
        let placed = place_strip(&columns, &[10, 12], false);
        assert_eq!(placed[0][0].span_cols, 2);
        assert!(!placed[0][0].cont_left);
        assert!(!placed[0][0].cont_right);
    }

    #[test]
    fn timed_event_fills_a_lane_the_span_leaves_open() {
        let columns = vec![
            vec![all_day("trip", 10, 11), timed("call", 10, 9 * 60)],
            vec![all_day("trip", 10, 11)],
            vec![timed("later", 12, 15 * 60)],
        ];
        let placed = place_strip(&columns, &[10, 11, 12], true);
        assert_eq!(placed[0][0].lane, 0);
        assert_eq!(placed[0][0].span_cols, 2);
        assert_eq!(placed[0][1].lane, 1);
        assert_eq!(placed[0][1].span_cols, 1);
        assert_eq!(placed[2][0].lane, 0);
        assert_eq!(placed[2][0].span_cols, 1);
    }

    #[test]
    fn longer_span_keeps_the_top_lane() {
        let columns = vec![
            vec![all_day("long", 1, 3), all_day("short", 1, 1)],
            vec![all_day("long", 1, 3)],
            vec![all_day("long", 1, 3)],
        ];
        let placed = place_strip(&columns, &[1, 2, 3], false);
        assert_eq!(placed[0][0].lane, 0);
        assert_eq!(placed[0][0].span_cols, 3);
        assert_eq!(placed[0][1].lane, 1);
        assert_eq!(placed[0][1].span_cols, 1);
    }
}

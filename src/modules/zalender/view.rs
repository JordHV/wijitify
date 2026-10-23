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
    unit: ViewUnit,
    count: u32,
    week_start: WeekStart,
) -> String {
    let _ws = week_start_num(week_start);
    match unit {
        ViewUnit::Day if count <= 1 => anchor.format("%d %b").to_string(),
        ViewUnit::Day => {
            let end = anchor + Duration::days(i64::from(count.saturating_sub(1)));
            format!("{} – {}", anchor.format("%d %b"), end.format("%d %b %Y"))
        }
        ViewUnit::Week if count <= 1 => {
            let start = start_of_week(anchor, _ws);
            let end = start + Duration::days(6);
            format!("{} – {}", start.format("%d %b"), end.format("%d %b %Y"))
        }
        ViewUnit::Week => {
            let start = start_of_week(anchor, _ws);
            let end = start + Duration::days(7 * i64::from(count) - 1);
            format!("{} – {}", start.format("%d %b"), end.format("%d %b %Y"))
        }
        ViewUnit::Month if count <= 1 => anchor.format("%B %Y").to_string(),
        ViewUnit::Month => {
            let end = shift_anchor(anchor, ViewUnit::Month, count as i32 - 1);
            format!("{} – {}", anchor.format("%b %Y"), end.format("%b %Y"))
        }
        ViewUnit::Year if count <= 1 => anchor.format("%Y").to_string(),
        ViewUnit::Year => {
            let end_y = anchor.year() + count as i32 - 1;
            format!("{} – {}", anchor.year(), end_y)
        }
        ViewUnit::Agenda => {
            let weeks = count.max(1);
            let end = anchor + Duration::days(7 * i64::from(weeks) - 1);
            format!("{} – {}", anchor.format("%d %b"), end.format("%d %b %Y"))
        }
    }
}

pub fn month_grid(
    year: i32,
    month: u32,
    today: NaiveDate,
    week_start: WeekStart,
) -> Vec<(NaiveDate, bool, bool)> {
    model::month_grid(year, month, today, week_start_num(week_start))
}

/// First and last date of `weeks` week-rows starting at the week that contains `anchor`.
pub fn week_window(anchor: NaiveDate, weeks: u32, week_start: WeekStart) -> (NaiveDate, NaiveDate) {
    let start = start_of_week(anchor, week_start_num(week_start));
    let weeks = weeks.max(1);
    let end = start + Duration::days(7 * i64::from(weeks) - 1);
    (start, end)
}

pub fn title_for_week_window(anchor: NaiveDate, weeks: u32, week_start: WeekStart) -> String {
    let (start, end) = week_window(anchor, weeks, week_start);
    format!("{} – {}", start.format("%d %b"), end.format("%d %b %Y"))
}

pub fn week_labels(week_start: WeekStart) -> [&'static str; 7] {
    model::week_labels(week_start_num(week_start))
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
        assert_eq!(ViewUnit::Day.zoom_out(), ViewUnit::Week);
        let title = title_for_view(d, ViewUnit::Month, 1, WeekStart::Sunday);
        assert_eq!(title, "October 2026");
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
    fn week_window_starts_on_this_week() {
        // 22 Oct 2026 is Thursday. Sunday-first week is 18–31 Oct for two weeks.
        let anchor = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let (start, end) = week_window(anchor, 2, WeekStart::Sunday);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 10, 18).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
    }
}

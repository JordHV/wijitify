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
}

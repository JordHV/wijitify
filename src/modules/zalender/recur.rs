use chrono::{DateTime, Duration, Utc};
use rrule::{RRuleSet, Tz};

/// Expand an RRULE string into occurrence starts within [from, to].
/// Returns empty on parse failure (bad feed should not crash the app).
pub fn expand_rrule(
    dtstart: DateTime<Utc>,
    rrule_line: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    let start = dtstart.with_timezone(&Tz::UTC);
    let body = rrule_line
        .trim()
        .trim_start_matches("RRULE:")
        .trim()
        .to_string();
    let parsed = format!("DTSTART:{}\nRRULE:{}", start.format("%Y%m%dT%H%M%SZ"), body);
    let set: RRuleSet = match parsed.parse() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("rrule parse failed: {e}");
            return Vec::new();
        }
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn weekly_expands() {
        let start = Utc.with_ymd_and_hms(2026, 1, 5, 15, 0, 0).unwrap();
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let occ = expand_rrule(start, "FREQ=WEEKLY;COUNT=4", from, to);
        assert_eq!(occ.len(), 4);
    }
}

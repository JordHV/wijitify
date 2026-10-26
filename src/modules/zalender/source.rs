use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use icalendar::{Calendar, CalendarComponent, Component, DatePerhapsTime};

use crate::services::config::FeedConfig;
use crate::services::storage::{self, write_atomic};

use super::model::{CalEvent, CalendarSnapshot};
use super::recur::expand_rrule;

#[derive(Debug, Clone)]
pub struct FeedStatus {
    pub ok: bool,
    pub from_cache: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FeedMeta {
    etag: Option<String>,
    last_modified: Option<String>,
    fetched_at: chrono::DateTime<Utc>,
}

pub async fn refresh_feeds(
    feeds: &[FeedConfig],
    cache_dir: &Path,
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> Result<(CalendarSnapshot, Vec<FeedStatus>)> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("wijitify/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let from = window_start
        .and_hms_opt(0, 0, 0)
        .map(|n| Utc.from_utc_datetime(&n))
        .unwrap_or_else(Utc::now);
    let to = window_end
        .and_hms_opt(23, 59, 59)
        .map(|n| Utc.from_utc_datetime(&n))
        .unwrap_or_else(Utc::now);

    let mut all_events = Vec::new();
    let mut statuses = Vec::new();

    for feed in feeds {
        if feed.local || feed.url.starts_with("local://") {
            statuses.push(FeedStatus {
                ok: true,
                from_cache: true,
            });
            continue;
        }
        match fetch_one(&client, feed, cache_dir, from, to).await {
            Ok((events, status)) => {
                all_events.extend(events);
                statuses.push(status);
            }
            Err(e) => {
                // Fall back to cache; never crash the UI path.
                tracing::warn!("feed '{}' refresh failed: {e:#}", feed.name);
                match load_feed_cache(cache_dir, feed, from, to) {
                    Ok(events) => {
                        all_events.extend(events);
                        statuses.push(FeedStatus {
                            ok: false,
                            from_cache: true,
                        });
                    }
                    Err(_) => {
                        statuses.push(FeedStatus {
                            ok: false,
                            from_cache: false,
                        });
                    }
                }
            }
        }
    }

    let snap = CalendarSnapshot::from_events(all_events, window_start, window_end);
    save_snapshot(cache_dir, &snap)?;
    Ok((snap, statuses))
}

pub fn load_from_cache(cache_dir: &Path) -> Result<Option<CalendarSnapshot>> {
    let path = cache_dir.join("snapshot.json");
    let Some(bytes) = storage::read_optional(&path)? else {
        return Ok(None);
    };
    match serde_json_snapshot(&bytes) {
        Ok(s) if s.schema_version == CalendarSnapshot::SCHEMA => Ok(Some(s)),
        Ok(_) => {
            tracing::warn!("discarding old snapshot schema");
            let _ = std::fs::remove_file(path);
            Ok(None)
        }
        Err(e) => {
            tracing::warn!("corrupt snapshot discarded: {e:#}");
            let _ = std::fs::remove_file(path);
            Ok(None)
        }
    }
}

fn serde_json_snapshot(bytes: &[u8]) -> Result<CalendarSnapshot> {
    // Use toml-free JSON via manual minimal serde — add serde_json dependency.
    Ok(serde_json::from_slice(bytes)?)
}

fn save_snapshot(cache_dir: &Path, snap: &CalendarSnapshot) -> Result<()> {
    let bytes = serde_json::to_vec(snap)?;
    write_atomic(&cache_dir.join("snapshot.json"), &bytes)
}

/// Ensure `#` in Google calendar IDs is path data (`%23`), not an HTTP fragment.
pub fn normalize_ics_url(url: &str) -> Result<String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        anyhow::bail!("URL must start with http:// or https://");
    }
    // Split off a real fragment only if `#` appears after `?`, otherwise encode all `#`.
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b.to_string(), Some(q)),
        None => (url.to_string(), None),
    };
    let base = base.replace('#', "%23");
    Ok(match query {
        Some(q) => {
            let q = q.replace('#', "%23");
            format!("{base}?{q}")
        }
        None => base,
    })
}

async fn fetch_one(
    client: &reqwest::Client,
    feed: &FeedConfig,
    cache_dir: &Path,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> Result<(Vec<CalEvent>, FeedStatus)> {
    let url = normalize_ics_url(&feed.url)?;
    let raw_path = feed_raw_path(cache_dir, feed);
    let meta_path = feed_meta_path(cache_dir, feed);
    let meta: Option<FeedMeta> =
        storage::read_optional(&meta_path)?.and_then(|b| serde_json::from_slice(&b).ok());

    let mut req = client.get(&url);
    if let Some(m) = &meta {
        if let Some(etag) = &m.etag {
            req = req.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        if let Some(lm) = &m.last_modified {
            req = req.header(reqwest::header::IF_MODIFIED_SINCE, lm);
        }
    }

    let resp = req.send().await.context("http get")?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        let events = load_feed_cache(cache_dir, feed, from, to)?;
        return Ok((
            events,
            FeedStatus {
                ok: true,
                from_cache: true,
            },
        ));
    }

    let resp = resp.error_for_status().context("http status")?;
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let last_modified = resp
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = resp.text().await.context("read body")?;

    write_atomic(&raw_path, body.as_bytes())?;
    let new_meta = FeedMeta {
        etag,
        last_modified,
        fetched_at: Utc::now(),
    };
    write_atomic(&meta_path, &serde_json::to_vec(&new_meta)?)?;

    let events = parse_ics(&body, feed, from, to)?;
    Ok((
        events,
        FeedStatus {
            ok: true,
            from_cache: false,
        },
    ))
}

fn load_feed_cache(
    cache_dir: &Path,
    feed: &FeedConfig,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> Result<Vec<CalEvent>> {
    let raw = storage::read_optional(&feed_raw_path(cache_dir, feed))?.context("no cached ics")?;
    let text = String::from_utf8(raw).context("ics utf-8")?;
    parse_ics(&text, feed, from, to)
}

fn feed_key(feed: &FeedConfig) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    feed.url.hash(&mut h);
    format!("{:x}", h.finish())
}

fn feed_raw_path(cache_dir: &Path, feed: &FeedConfig) -> PathBuf {
    cache_dir.join(format!("{}.ics", feed_key(feed)))
}

fn feed_meta_path(cache_dir: &Path, feed: &FeedConfig) -> PathBuf {
    cache_dir.join(format!("{}.meta.json", feed_key(feed)))
}

pub fn parse_ics(
    body: &str,
    feed: &FeedConfig,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> Result<Vec<CalEvent>> {
    let cal: Calendar = body
        .parse()
        .map_err(anyhow::Error::msg)
        .context("parse ics")?;
    let mut out = Vec::new();

    for component in &cal.components {
        let CalendarComponent::Event(ev) = component else {
            continue;
        };
        let title = ev.get_summary().unwrap_or("(no title)").to_string();
        let uid = ev
            .get_uid()
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}-{}", feed.id, title));
        let description = ev.get_description().unwrap_or("").to_string();
        let location = ev.property_value("LOCATION").unwrap_or("").to_string();

        let Some(start_dpt) = ev.get_start() else {
            continue;
        };
        let (start, all_day) = dpt_to_utc(start_dpt);
        let end = ev
            .get_end()
            .map(dpt_to_utc)
            .map(|(t, _)| t)
            .unwrap_or_else(|| {
                if all_day {
                    start + Duration::days(1)
                } else {
                    start + Duration::hours(1)
                }
            });

        let duration = end - start;
        let rrule = ev.property_value("RRULE").map(str::to_string);
        let exdates = collect_exdates(ev);

        if let Some(rule) = rrule {
            for occ in expand_rrule(start, &rule, from, to) {
                if exdates.contains(&occ.timestamp()) {
                    continue;
                }
                out.push(CalEvent {
                    uid: uid.clone(),
                    recurrence_id: Some(occ.format("%Y%m%dT%H%M%SZ").to_string()),
                    title: title.clone(),
                    start: occ,
                    end: occ + duration,
                    all_day,
                    feed_id: feed.id.clone(),
                    feed_name: feed.name.clone(),
                    color: feed.color.clone(),
                    description: description.clone(),
                    location: location.clone(),
                    rrule: None,
                });
            }
        } else if end >= from && start <= to {
            out.push(CalEvent {
                uid,
                recurrence_id: None,
                title,
                start,
                end,
                all_day,
                feed_id: feed.id.clone(),
                feed_name: feed.name.clone(),
                color: feed.color.clone(),
                description,
                location,
                rrule: None,
            });
        }
    }

    Ok(out)
}

fn dpt_to_utc(dpt: DatePerhapsTime) -> (chrono::DateTime<Utc>, bool) {
    match dpt {
        DatePerhapsTime::Date(d) => {
            let dt = d
                .and_hms_opt(0, 0, 0)
                .map(|n| Utc.from_utc_datetime(&n))
                .unwrap_or_else(Utc::now);
            (dt, true)
        }
        DatePerhapsTime::DateTime(dt) => {
            let utc = match dt {
                icalendar::CalendarDateTime::Floating(n) => Utc.from_utc_datetime(&n),
                icalendar::CalendarDateTime::Utc(u) => u,
                icalendar::CalendarDateTime::WithTimezone { date_time, tzid } => {
                    parse_tz_local(&date_time, &tzid)
                }
            };
            (utc, false)
        }
    }
}

fn parse_tz_local(naive: &chrono::NaiveDateTime, tzid: &str) -> chrono::DateTime<Utc> {
    // Map a few Windows / common names; fall back to treating as UTC.
    let mapped = map_tzid(tzid);
    if let Ok(tz) = mapped.parse::<chrono_tz::Tz>() {
        return tz
            .from_local_datetime(naive)
            .single()
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|| Utc.from_utc_datetime(naive));
    }
    Utc.from_utc_datetime(naive)
}

fn map_tzid(tzid: &str) -> &str {
    match tzid {
        "Eastern Standard Time" | "US/Eastern" => "America/New_York",
        "Central Standard Time" | "US/Central" => "America/Chicago",
        "Mountain Standard Time" | "US/Mountain" => "America/Denver",
        "Pacific Standard Time" | "US/Pacific" => "America/Los_Angeles",
        "UTC" | "GMT" => "UTC",
        other => other,
    }
}

fn collect_exdates(ev: &icalendar::Event) -> std::collections::HashSet<i64> {
    let mut set = std::collections::HashSet::new();
    if let Some(v) = ev.property_value("EXDATE") {
        for part in v.split(',') {
            if let Some(ts) = parse_ics_timestamp(part.trim()) {
                set.insert(ts);
            }
        }
    }
    set
}

fn parse_ics_timestamp(s: &str) -> Option<i64> {
    let s = s.trim_end_matches('Z');
    if s.len() >= 15 {
        let dt = chrono::NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%S").ok()?;
        Some(Utc.from_utc_datetime(&dt).timestamp())
    } else if s.len() >= 8 {
        let d = NaiveDate::parse_from_str(&s[..8], "%Y%m%d").ok()?;
        Some(Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0)?).timestamp())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn normalize_encodes_hash_in_path() {
        let u = normalize_ics_url(
            "https://calendar.google.com/calendar/ical/en.usa#holiday@group.v.calendar.google.com/public/basic.ics",
        )
        .unwrap();
        assert!(u.contains("%23holiday"));
        assert!(!u.contains("#holiday"));
    }

    #[test]
    fn normalize_keeps_already_encoded() {
        let u = normalize_ics_url(
            "https://calendar.google.com/calendar/ical/en.usa%23holiday%40group.v.calendar.google.com/public/basic.ics",
        )
        .unwrap();
        assert!(u.contains("%23holiday"));
    }

    #[test]
    fn parse_google_style_all_day_date() {
        let body = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\n\
            DTSTART;VALUE=DATE:20260119\r\nDTEND;VALUE=DATE:20260120\r\n\
            SUMMARY:Martin Luther King Jr. Day\r\nUID:mlk@test\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let feed = FeedConfig {
            id: "feed-1".into(),
            name: "US Holidays".into(),
            url: "https://example.com/basic.ics".into(),
            color: "#ef4444".into(),
            visible: true,
            writable: false,
            local: false,
        };
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 59).unwrap();
        let events = parse_ics(body, &feed, from, to).unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].all_day);
        assert_eq!(events[0].title, "Martin Luther King Jr. Day");
        assert_eq!(events[0].color, "#ef4444");
    }
}

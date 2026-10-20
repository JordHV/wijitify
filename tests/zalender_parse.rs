use std::path::PathBuf;

use chrono::{Duration, NaiveDate, TimeZone, Utc};

#[path = "../src/modules/zalender/model.rs"]
mod model;

#[test]
fn fixture_exists() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.ics");
    assert!(path.is_file());
    let body = std::fs::read_to_string(path).unwrap();
    assert!(body.contains("All Day Event"));
    assert!(body.contains("RRULE"));
}

#[test]
fn snapshot_indexes_days() {
    let start = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
    let events = vec![model::CalEvent {
        uid: "1".into(),
        title: "Test".into(),
        start,
        end: start + Duration::days(1),
        all_day: true,
        feed_name: "f".into(),
        color: "#3b82f6".into(),
    }];
    let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let snap = model::CalendarSnapshot::from_events(events, from, to);
    let day = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    assert_eq!(snap.events_on(day).len(), 1);
}

/// Readers index month grids/snapshots while a dummy refresher swaps in new snapshots.
#[test]
fn month_grid_and_snapshot_survive_concurrent_refresh() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, RwLock};
    use std::thread;

    let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
    let build = move |n: i64| {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
        let events = (0..n * 20)
            .map(|i| model::CalEvent {
                uid: i.to_string(),
                title: "e".into(),
                start: start + Duration::days(i % 300),
                end: start + Duration::days(i % 300) + Duration::hours(1),
                all_day: false,
                feed_name: "f".into(),
                color: "#3b82f6".into(),
            })
            .collect();
        Arc::new(model::CalendarSnapshot::from_events(events, from, to))
    };

    let current = Arc::new(RwLock::new(build(1)));
    let done = Arc::new(AtomicBool::new(false));

    let refresher = {
        let (current, done) = (current.clone(), done.clone());
        thread::spawn(move || {
            for n in 1..=200 {
                let snap = build(n % 10 + 1);
                *current.write().unwrap() = snap;
            }
            done.store(true, Ordering::Release);
        })
    };

    let readers: Vec<_> = (0..4)
        .map(|_| {
            let (current, done) = (current.clone(), done.clone());
            thread::spawn(move || {
                let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
                while !done.load(Ordering::Acquire) {
                    let snap = current.read().unwrap().clone();
                    for month in 1..=12 {
                        let cells = model::month_grid(2026, month, today);
                        assert_eq!(cells.len(), 42);
                        for (d, _, _) in cells {
                            for e in snap.events_on(d) {
                                assert!(e.start.date_naive() <= d);
                            }
                        }
                    }
                }
            })
        })
        .collect();

    refresher.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }
}

use super::*;

fn sec(text: &str) -> i64 {
    text.parse::<Timestamp>().unwrap().as_second()
}

fn due(expr: &str, zone: &str, instant: &str, expected: Option<&str>) {
    let handle = Recurrence::compile(expr, zone).unwrap();
    let now = sec(instant);
    let utc = expected.map(sec);
    assert_eq!(
        handle.query(now, Query::Minute),
        Ok(utc),
        "{expr} {instant}"
    );
    if let Some(occurrence) = utc {
        if occurrence > MIN_SECOND {
            assert_eq!(handle.query(occurrence - 1, Query::Next), Ok(utc));
        }
        assert_eq!(handle.query(occurrence + 1, Query::Previous), Ok(utc));
    }
}

#[test]
fn skip_gap_in_both_search_directions_and_current_minute() {
    for (zone, expr, shifted, prev, next) in [
        (
            "America/New_York",
            "every day at 02:30",
            "2024-03-10T07:30:25Z",
            "2024-03-09T07:30:00Z",
            "2024-03-11T06:30:00Z",
        ),
        (
            "Australia/Lord_Howe",
            "every day at 02:15",
            "2024-10-05T15:45:25Z",
            "2024-10-04T15:45:00Z",
            "2024-10-06T15:15:00Z",
        ),
        (
            "Africa/Monrovia",
            "every day at 00:05",
            "1972-01-07T00:49:42Z",
            "1972-01-06T00:49:30Z",
            "1972-01-08T00:05:00Z",
        ),
        (
            "Pacific/Apia",
            "on 2011-12-30 at 09:00",
            "2011-12-30T19:00:25Z",
            "",
            "",
        ),
    ] {
        due(expr, zone, shifted, None);
        let handle = Recurrence::compile(expr, zone).unwrap();
        let at = sec(shifted);
        assert_eq!(
            handle.query(at - 60, Query::Next),
            Ok((!next.is_empty()).then(|| sec(next)))
        );
        assert_eq!(
            handle.query(at, Query::Previous),
            Ok((!prev.is_empty()).then(|| sec(prev)))
        );
    }
}

#[test]
fn shifted_and_native_occurrences_collide_once() {
    let expr = "every day at 02:30, 03:30 in America/New_York";
    due(
        expr,
        "",
        "2024-03-10T07:30:49Z",
        Some("2024-03-10T07:30:00Z"),
    );
    due(
        "every day at 09:00",
        "Pacific/Apia",
        "2011-12-30T19:00:57Z",
        Some("2011-12-30T19:00:00Z"),
    );
    let handle = Recurrence::compile(expr, "").unwrap();
    assert_eq!(
        handle.query(sec("2024-03-10T07:30:00Z"), Query::Next),
        Ok(Some(sec("2024-03-11T06:30:00Z")))
    );
}

#[test]
fn fold_uses_first_real_instant_only() {
    for expr in ["every day at 01:30", "every 30 min from 01:00 to 02:00"] {
        due(
            expr,
            "America/New_York",
            "2024-11-03T05:30:44Z",
            Some("2024-11-03T05:30:00Z"),
        );
        due(expr, "America/New_York", "2024-11-03T06:30:44Z", None);
    }
}

#[test]
fn supported_range_and_exhaustion_are_distinct() {
    let daily = Recurrence::compile("every 1 min from 00:00 to 23:59", "UTC").unwrap();
    for op in [Query::Minute, Query::Next, Query::Previous] {
        for bad in [i64::MIN, MIN_SECOND - 1, END_SECOND, i64::MAX] {
            assert_eq!(daily.query(bad, op), Err(RANGE));
        }
    }
    assert_eq!(MIN_SECOND, sec("0001-01-02T00:00:00Z"));
    assert_eq!(END_SECOND, sec("9999-12-30T00:00:00Z"));
    assert_eq!(daily.query(MIN_SECOND, Query::Previous), Ok(None));
    assert_eq!(daily.query(END_SECOND - 1, Query::Next), Ok(None));
    assert_eq!(daily.query(MIN_SECOND, Query::Minute), Ok(Some(MIN_SECOND)));
    assert_eq!(
        daily.query(END_SECOND - 1, Query::Minute),
        Ok(Some(END_SECOND - 60))
    );
    for (expr, zone) in [
        ("on 0001-01-01 at 00:00", "UTC"),
        ("on 9999-12-30 at 04:00", "UTC"),
    ] {
        assert_eq!(Recurrence::compile(expr, zone).err(), Some(RANGE));
    }
    due(
        "every day at 20:00",
        "Etc/GMT+4",
        "0001-01-02T00:00:32Z",
        Some("0001-01-02T00:00:00Z"),
    );
    due(
        "every day at 01:00",
        "Etc/GMT-2",
        "9999-12-29T23:00:19Z",
        Some("9999-12-29T23:00:00Z"),
    );
}

#[test]
fn single_date_queries_include_epoch_and_report_exhaustion() {
    let h = Recurrence::compile("on 1970-01-01 at 00:00", "UTC").unwrap();
    for (cursor, op, expected) in [
        (-1, Query::Next, Some(0)),
        (0, Query::Previous, None),
        (1, Query::Previous, Some(0)),
        (0, Query::Next, None),
        (59, Query::Minute, Some(0)),
        (60, Query::Minute, None),
        (MIN_SECOND, Query::Next, Some(0)),
        (END_SECOND - 1, Query::Previous, Some(0)),
    ] {
        assert_eq!(h.query(cursor, op), Ok(expected));
    }
}

#[test]
fn preparation_binds_inline_or_supplied_zone_and_rejects_trailing_tokens() {
    let h = Recurrence::compile("EVERY WEEKDAY AT 9:00", "america/chicago").unwrap();
    let explicit =
        Recurrence::compile("  every weekday at 09:00 in America/Chicago  ", "UTC").unwrap();
    assert!(h.same_meaning(&explicit));
    assert!(!h.same_meaning(&Recurrence::compile("every day at 11:00", "UTC").unwrap()));
    assert_ne!(
        h.effective,
        Recurrence::compile("every weekday at 09:00", "UTC")
            .unwrap()
            .effective
    );
    for z in ["", "Unknown/Zone", "EST"] {
        assert_eq!(
            Recurrence::compile("every day at 09:00", z).err(),
            Some(ZONE)
        );
    }
    assert_eq!(
        Recurrence::compile("every day at 09:00 unexpected", "UTC").err(),
        Some(PARSE)
    );
    due(
        "every weekday at 9:00 in America/Chicago",
        "UTC",
        "2024-01-08T15:00:59Z",
        Some("2024-01-08T15:00:00Z"),
    );
}

#[test]
fn search_limit_is_distinct_from_exhaustion_after_a_skipped_candidate() {
    let h = Recurrence::compile("every day at 02:30 in America/New_York", "").unwrap();
    let cursor = sec("2024-03-10T00:00:00Z");
    assert_eq!(h.query_with_budget(cursor, Query::Next, 1), Err(LIMIT));
    assert_eq!(
        h.query_with_budget(cursor, Query::Next, 100),
        Ok(Some(sec("2024-03-11T06:30:00Z")))
    );
}

#[test]
fn historical_second_offset_returns_the_real_boundary() {
    due(
        "every day at 12:00",
        "Europe/Paris",
        "1900-01-01T11:51:09Z",
        Some("1900-01-01T11:50:39Z"),
    );
}

#[test]
fn explicit_date_modifiers_share_the_effective_zone_range() {
    for expr in [
        "every day at 09:00 except 0001-01-01",
        "every day at 09:00 starting 9999-12-31",
        "every day at 09:00 until 9999-12-30",
        "on 0001-01-02 at 00:00 in Etc/GMT-4",
    ] {
        assert_eq!(Recurrence::compile(expr, "UTC").err(), Some(RANGE));
    }
    assert!(
        Recurrence::compile(
            "every day at 09:00 except dec 25, 2024-02-29 until 2028-02-29 starting 2024-01-01",
            "UTC"
        )
        .is_ok()
    );
}

#[test]
fn nominal_slot_filter_retains_an_independent_civil_slot() {
    for (expr, zone, real, accepted) in [
        (
            "every day at 02:30",
            "America/New_York",
            "2024-03-10T07:30:00Z",
            false,
        ),
        (
            "every day at 02:30, 03:30",
            "America/New_York",
            "2024-03-10T07:30:00Z",
            true,
        ),
        (
            "on 2011-12-30 at 09:00",
            "Pacific/Apia",
            "2011-12-30T19:00:00Z",
            false,
        ),
        (
            "every 30 min from 01:00 to 04:00",
            "America/New_York",
            "2024-03-10T07:30:00Z",
            true,
        ),
    ] {
        let h = Recurrence::compile(expr, zone).unwrap();
        assert_eq!(
            h.query(sec(real), Query::Minute).unwrap().is_some(),
            accepted
        );
    }
}

#[test]
fn retained_zone_rules_match_after_an_isolated_database_change() {
    use std::{fs, process};
    const FIXTURE: &str = "RPTADV_TEST_RECURRENCE_ZONE_FIXTURE";
    if let Some(path) = std::env::var_os(FIXTURE) {
        let file = std::path::Path::new(&path).join("Test/Retained");
        let old = Recurrence::compile("every day at 12:00", "Test/Retained").unwrap();
        let clone = old.clone();
        fs::copy("/usr/share/zoneinfo/Etc/GMT-3", &file).unwrap();
        jiff::tz::db().reset();
        let new = Recurrence::compile("every day at 12:00", "Test/Retained").unwrap();
        assert!(old.same_meaning(&clone));
        assert!(!old.same_meaning(&new));
        let noon = sec("2024-01-02T12:00:00Z");
        for (h, expected) in [(&old, noon), (&clone, noon), (&new, noon - 10_800)] {
            assert_eq!(h.query(expected + 22, Query::Minute), Ok(Some(expected)));
            assert_eq!(h.query(expected - 1, Query::Next), Ok(Some(expected)));
            assert_eq!(h.query(expected + 1, Query::Previous), Ok(Some(expected)));
        }
        return;
    }
    let dir = std::env::temp_dir().join(format!("rptadv-recurrence-zone-{}", process::id()));
    fs::create_dir(&dir).unwrap();
    fs::create_dir(dir.join("Test")).unwrap();
    fs::copy("/usr/share/zoneinfo/UTC", dir.join("Test/Retained")).unwrap();
    let child = process::Command::new(std::env::current_exe().unwrap())
        .env("TZDIR", &dir)
        .env(FIXTURE, &dir)
        .args([
            "--exact",
            "recurrence::tests::retained_zone_rules_match_after_an_isolated_database_change",
            "--nocapture",
        ])
        .output()
        .unwrap();
    fs::remove_dir_all(&dir).unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
fn period_limit_applies_inside_first_native_query() {
    let h = Recurrence::compile("every month on the 31st at 09:00 during feb, mar", "UTC").unwrap();
    let cursor = sec("2026-01-31T12:00:00Z");
    assert_eq!(h.query_with_budget(cursor, Query::Next, 1), Err(LIMIT));
}

#[test]
fn canonical_set_parts_keep_one_occurrence_meaning() {
    for (a, b) in [
        (
            "every 2 weeks on monday, friday at 09:00",
            "every 2 weeks on friday, monday at 9:00",
        ),
        (
            "every month on the last weekday at 9:00, 17:00",
            "every month on the last weekday at 17:00, 09:00",
        ),
        (
            "every monday, friday at 9:00, 09:00",
            "every friday, monday, friday at 09:00",
        ),
        (
            "every day at 09:00 except jan 1, dec 25, 2030-01-02 during jan, feb",
            "every day at 09:00 except 2030-01-02, dec 25, jan 1, jan 1 during feb, jan, feb",
        ),
        (
            "every month on the 2nd, 1st at 17:00, 09:00",
            "every month on the 1st, 2nd, 1st at 09:00, 17:00",
        ),
        (
            "every 3 min from 09:00 to 10:00 on monday, friday",
            "every 3 min from 09:00 to 10:00 on friday, monday",
        ),
        (
            "every year on jan 1 at 18:00, 17:00",
            "every year on jan 1 at 17:00, 18:00",
        ),
    ] {
        let a = Recurrence::compile(a, "UTC").unwrap();
        let b = Recurrence::compile(b, "UTC").unwrap();
        assert!(a.same_meaning(&b));
    }
}

#[test]
fn bounded_search_handles_empty_periods_and_distant_exceptions() {
    let base = "every month on the 31st at 09:00 except 9998-05-01";
    let empty = Recurrence::compile(&format!("{base} during feb"), "UTC").unwrap();
    let at = sec("2026-02-09T12:00:00Z");
    assert_eq!(empty.query(at, Query::Minute), Ok(None));
    assert_eq!(empty.query(at, Query::Next), Err(LIMIT));
    let ended = Recurrence::compile(&format!("{base} until 2026-03-01 during feb"), "UTC").unwrap();
    assert_eq!(ended.query(at, Query::Next), Ok(None));
    let sparse = Recurrence::compile(&format!("{base} during feb, mar"), "UTC").unwrap();
    assert_eq!(
        sparse.query(at, Query::Next),
        Ok(Some(sec("2026-03-31T09:00:00Z")))
    );
}

#[test]
fn original_hron_api_retains_native_dst_resolution() {
    let h = Schedule::parse("every day at 02:30 in America/New_York").unwrap();
    let at = instant(sec("2024-03-10T07:30:00Z"), &TimeZone::UTC);
    assert!(h.matches(&at));
    assert_eq!(
        h.previous_from(&at).unwrap().timestamp().as_second(),
        sec("2024-03-09T07:30:00Z")
    );
    assert_eq!(
        h.next_from(&instant(at.timestamp().as_second() - 1, &TimeZone::UTC))
            .unwrap()
            .timestamp(),
        at.timestamp()
    );
    let outside = Timestamp::from_second(MIN_SECOND - 1)
        .unwrap()
        .to_zoned(TimeZone::UTC);
    assert!(
        captured_occurrence(&h, &outside, Query::Minute, 100)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_candidate_is_returned_only_after_the_order_is_proven() {
    let h = Recurrence::compile("every day at 09:00, 17:00 starting 2026-01-01", "UTC").unwrap();
    let at = sec("2026-01-10T00:00:00Z");
    let mut reached = false;
    for budget in 0..32 {
        match h.query_with_budget(at, Query::Next, budget) {
            Err(LIMIT) => assert!(!reached),
            Ok(Some(s)) => {
                assert_eq!(s, sec("2026-01-10T09:00:00Z"));
                reached = true;
            }
            other => panic!("unexpected result {other:?}"),
        }
    }
    assert!(reached);
    assert_eq!(
        h.query(sec("2025-11-01T00:00:00Z"), Query::Previous),
        Ok(None)
    );
}

#[test]
fn previous_across_timezone_law_change_beyond_gregorian_cycle() {
    let h = Recurrence::compile(
        "every year on the second sunday of march at 02:30 in America/New_York",
        "",
    )
    .unwrap();
    // Since 2007 this slot has fallen in the gap. In 2006 DST started in April.
    assert_eq!(
        h.query(sec("2500-06-01T00:00:00Z"), Query::Previous),
        Ok(Some(sec("2006-03-12T07:30:00Z")))
    );
}

#[test]
fn native_hron_iterators_remain_send_and_sync() {
    fn assert_traits<T: Send + Sync>() {}
    assert_traits::<hron::Occurrences<'static>>();
    assert_traits::<hron::BoundedOccurrences<'static>>();
}

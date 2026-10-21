use super::*;
use crate::schedule::{CivilTime, ScheduledEvent, ScheduledWindow, Weekday};
use std::{ffi::CString, mem::size_of, ptr, str::FromStr};

fn api() -> &'static ScheduleDescriptor {
    unsafe { rptadv_schedule_descriptor().as_ref().unwrap() }
}

fn civil(year: u16, month: u8, day: u8, weekday: u8, hour: u8, minute: u8) -> ScheduleCivil {
    let mut result = ScheduleCivil::default();
    assert_eq!(
        unsafe { (api().civil_new)(year, month, day, weekday, hour, minute, &mut result) },
        0
    );
    result
}

fn window(
    days: Option<&str>,
    dates: Option<&str>,
    start: &str,
    end: &str,
) -> *mut ScheduleWindowHandle {
    let days_text = days.map(|value| CString::new(value).unwrap());
    let dates_text = dates.map(|value| CString::new(value).unwrap());
    let start = CString::new(start).unwrap();
    let end = CString::new(end).unwrap();
    let mut result = ptr::null_mut();
    assert_eq!(
        unsafe {
            (api().window_create)(
                u32::from(days.is_some()),
                days_text.as_ref().map_or(ptr::null(), |text| text.as_ptr()),
                u32::from(dates.is_some()),
                dates_text
                    .as_ref()
                    .map_or(ptr::null(), |text| text.as_ptr()),
                start.as_ptr(),
                end.as_ptr(),
                &mut result,
            )
        },
        0
    );
    assert!(!result.is_null());
    result
}

fn boundary(
    f: unsafe extern "C" fn(
        *const ScheduleWindowHandle,
        *const ScheduleCivil,
        u8,
        *mut u32,
        *mut u64,
        *mut u64,
    ) -> i32,
    handle: *const ScheduleWindowHandle,
    local: &ScheduleCivil,
    second: u8,
) -> Option<(u64, u64)> {
    let (mut present, mut date, mut remaining) = (9, 9, 9);
    assert_eq!(
        unsafe {
            f(
                handle,
                local,
                second,
                &mut present,
                &mut date,
                &mut remaining,
            )
        },
        0
    );
    if present == 0 {
        None
    } else {
        Some((date, remaining))
    }
}

#[test]
fn descriptor_and_civil_event_records_preserve_parsing_and_occurrence() {
    let api = api();
    assert_eq!(api.struct_size as usize, size_of::<ScheduleDescriptor>());
    assert_eq!(api.abi_version, 1);
    assert_eq!(
        unsafe { std::ffi::CStr::from_ptr(api.capability_name) }.to_bytes(),
        b"rptadv.schedule"
    );
    let leap = civil(2028, 2, 29, 2, 23, 59);
    assert_eq!(
        (
            leap.year,
            leap.month,
            leap.day,
            leap.weekday,
            leap.hour,
            leap.minute
        ),
        (2028, 2, 29, 2, 23, 59)
    );
    assert_eq!(
        unsafe { (api.civil_new)(2027, 2, 29, 1, 0, 0, &mut ScheduleCivil::default()) },
        -1
    );
    assert_eq!(
        unsafe { (api.civil_new)(2028, 2, 29, 7, 0, 0, &mut ScheduleCivil::default()) },
        -1
    );
    for text in [
        "daily 23:59",
        "weekly Tuesday 23:59",
        "once 2028-02-29 23:59",
    ] {
        let mut event = ScheduleEvent::default();
        let text_c = CString::new(text).unwrap();
        assert_eq!(unsafe { (api.event_parse)(text_c.as_ptr(), &mut event) }, 0);
        let mut present = 9;
        let mut occurrence = 9;
        assert_eq!(
            unsafe { (api.event_occurrence)(&event, &leap, &mut present, &mut occurrence) },
            0
        );
        let expected = ScheduledEvent::from_str(text)
            .unwrap()
            .occurrence(&CivilTime::new(2028, 2, 29, Weekday::Tuesday, 23, 59).unwrap());
        assert_eq!((present != 0).then_some(occurrence), expected);
    }
}

#[test]
fn selectors_preserve_none_empty_and_calendar_intersections() {
    let none = window(None, None, "09:00", "10:00");
    let empty = window(Some(""), Some(""), "09:00", "10:00");
    let monday = window(Some("Monday"), None, "09:00", "10:00");
    let leap = window(None, Some("2028-02-29"), "09:00", "10:00");
    let mut equal = 9;
    assert_eq!(unsafe { (api().window_equal)(none, empty, &mut equal) }, 0);
    assert_eq!(equal, 1);
    let mut overlap = 9;
    assert_eq!(
        unsafe { (api().window_overlaps)(monday, leap, &mut overlap) },
        0
    );
    assert_eq!(overlap, 0);
    assert_eq!(
        unsafe { (api().window_overlaps)(none, leap, &mut overlap) },
        0
    );
    assert_eq!(overlap, 1);
    let tuesday = civil(2028, 2, 29, 2, 9, 0);
    let mut matches = 9;
    assert_eq!(
        unsafe { (api().window_matches)(leap, &tuesday, &mut matches) },
        0
    );
    assert_eq!(matches, 1);
    let mut clone = ptr::null_mut();
    assert_eq!(unsafe { (api().window_clone)(leap, &mut clone) }, 0);
    assert_eq!(unsafe { (api().window_equal)(leap, clone, &mut equal) }, 0);
    assert_eq!(equal, 1);
    for handle in [none, empty, monday, leap, clone] {
        unsafe { (api().window_destroy)(handle) };
    }
}

#[test]
fn elapsed_end_seconds_and_boundary_order_preserve_the_abi() {
    let midnight = civil(2026, 9, 15, 2, 0, 1);
    let original_midnight = CivilTime::new(2026, 9, 15, Weekday::Tuesday, 0, 1).unwrap();
    for (end, cases) in [
        (
            "23:59",
            [(0, Some(120_000)), (59, Some(179_000)), (60, None)],
        ),
        (
            "24:00",
            [(0, Some(60_000)), (59, Some(119_000)), (60, None)],
        ),
    ] {
        let original = ScheduledWindow::parse(Some("Monday"), None, "23:00", end).unwrap();
        let shared = window(Some("Monday"), None, "23:00", end);
        for (second, expected) in cases {
            let mut present = 9;
            let mut elapsed = 9;
            assert_eq!(
                unsafe {
                    (api().window_elapsed_after_end)(
                        shared,
                        &midnight,
                        second,
                        &mut present,
                        &mut elapsed,
                    )
                },
                0
            );
            assert_eq!((present != 0).then_some(elapsed), expected);
            assert_eq!(elapsed, expected.unwrap_or(0));
            assert_eq!(
                original.elapsed_after_end(&original_midnight, second),
                expected
            );
            assert_eq!(
                boundary(api().window_next_start, shared, &midnight, second),
                original.next_start(&original_midnight, second)
            );
            assert_eq!(
                boundary(api().window_next_end, shared, &midnight, second),
                original.next_end(&original_midnight, second)
            );
        }
        unsafe { (api().window_destroy)(shared) };
    }
    let selected = window(None, Some("2028-02-29,2026-09-18"), "12:00", "13:00");
    let now = civil(2026, 9, 15, 2, 11, 0);
    let original_selected =
        ScheduledWindow::parse(None, Some("2028-02-29,2026-09-18"), "12:00", "13:00").unwrap();
    let original_now = CivilTime::new(2026, 9, 15, Weekday::Tuesday, 11, 0).unwrap();
    assert_eq!(
        boundary(api().window_next_start, selected, &now, 59),
        original_selected.next_start(&original_now, 59)
    );
    assert_eq!(
        boundary(api().window_next_end, selected, &now, 59),
        original_selected.next_end(&original_now, 59)
    );
    let start = boundary(api().window_next_start, selected, &now, 59).unwrap();
    let end = boundary(api().window_next_end, selected, &now, 59).unwrap();
    assert_eq!(start.0, end.0);
    assert_eq!(end.1 - start.1, 3_600_000);
    unsafe {
        (api().window_destroy)(selected);
    }
}

#[test]
fn maximum_calendar_date_and_past_dates_keep_boundary_order() {
    let maximal = window(None, Some("9999-12-31"), "23:59", "24:00");
    let minimum = civil(1, 1, 1, 1, 0, 0);
    assert_eq!(
        boundary(api().window_next_end, maximal, &minimum, 0),
        Some((3_652_058, 315_537_897_600_000))
    );
    let past_and_future = window(
        None,
        Some("2026-09-14,2026-09-15,2026-09-18"),
        "09:00",
        "10:00",
    );
    let now = civil(2026, 9, 15, 2, 11, 0);
    let expected = ScheduledWindow::parse(
        None,
        Some("2026-09-14,2026-09-15,2026-09-18"),
        "09:00",
        "10:00",
    )
    .unwrap();
    let civil_now = CivilTime::new(2026, 9, 15, Weekday::Tuesday, 11, 0).unwrap();
    assert_eq!(
        boundary(api().window_next_start, past_and_future, &now, 0),
        expected.next_start(&civil_now, 0)
    );
    unsafe {
        (api().window_destroy)(maximal);
        (api().window_destroy)(past_and_future);
    }
}

#[test]
fn abi_rejects_invalid_records_strings_and_required_pointers() {
    let api = api();
    let good = civil(2028, 2, 29, 2, 9, 30);
    let mut out_civil = ScheduleCivil::default();
    for weekday in 0..=6 {
        assert_eq!(
            unsafe { (api.civil_new)(2028, 2, 29, weekday, 9, 30, &mut out_civil) },
            0
        );
    }
    assert_eq!(
        unsafe { (api.civil_new)(2028, 2, 29, 2, 9, 30, ptr::null_mut()) },
        -1
    );
    let event_text = CString::new("weekly Tuesday 09:30").unwrap();
    let bad_text = CString::new("weekly Tue 09:30").unwrap();
    let mut event = ScheduleEvent::default();
    assert_eq!(unsafe { (api.event_parse)(ptr::null(), &mut event) }, -1);
    assert_eq!(
        unsafe { (api.event_parse)(bad_text.as_ptr(), &mut event) },
        -1
    );
    assert_eq!(
        unsafe { (api.event_parse)(event_text.as_ptr(), ptr::null_mut()) },
        -1
    );
    assert_eq!(
        unsafe { (api.event_parse)(event_text.as_ptr(), &mut event) },
        0
    );
    let (mut present, mut occurrence) = (9, 9);
    assert_eq!(
        unsafe { (api.event_occurrence)(ptr::null(), &good, &mut present, &mut occurrence) },
        -1
    );
    assert_eq!(
        unsafe { (api.event_occurrence)(&event, &good, ptr::null_mut(), &mut occurrence) },
        -1
    );
    for forged in [
        ScheduleEvent { hour: 24, ..event },
        ScheduleEvent {
            minute: 60,
            ..event
        },
        ScheduleEvent {
            kind: 2,
            weekday: 7,
            ..event
        },
        ScheduleEvent {
            kind: 3,
            year: 2027,
            month: 2,
            day: 29,
            ..event
        },
        ScheduleEvent { kind: 4, ..event },
    ] {
        assert_eq!(
            unsafe { (api.event_occurrence)(&forged, &good, &mut present, &mut occurrence) },
            -1
        );
    }
    let bad_civil = ScheduleCivil { weekday: 7, ..good };
    assert_eq!(
        unsafe { (api.event_occurrence)(&event, &bad_civil, &mut present, &mut occurrence) },
        -1
    );
    let not_due = civil(2028, 2, 29, 2, 9, 31);
    assert_eq!(
        unsafe { (api.event_occurrence)(&event, &not_due, &mut present, &mut occurrence) },
        0
    );
    assert_eq!((present, occurrence), (0, 0));

    let start = CString::new("09:00").unwrap();
    let end = CString::new("10:00").unwrap();
    let bad_selector = CString::new("Monday-").unwrap();
    assert_eq!(unsafe { (api.start_time_valid)(ptr::null()) }, 0);
    assert_eq!(unsafe { (api.end_time_valid)(ptr::null()) }, 0);
    assert_eq!(unsafe { (api.weekday_selector_valid)(ptr::null()) }, 0);
    assert_eq!(unsafe { (api.date_selector_valid)(ptr::null()) }, 0);
    assert_eq!(unsafe { (api.start_time_valid)(start.as_ptr()) }, 1);
    assert_eq!(unsafe { (api.end_time_valid)(end.as_ptr()) }, 1);
    assert_eq!(
        unsafe { (api.weekday_selector_valid)(bad_selector.as_ptr()) },
        0
    );
    assert_eq!(
        unsafe { (api.date_selector_valid)(bad_selector.as_ptr()) },
        0
    );
    let non_utf8 = [0xff_u8, 0];
    assert_eq!(
        unsafe { (api.start_time_valid)(non_utf8.as_ptr().cast()) },
        0
    );

    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe {
            (api.window_create)(
                0,
                ptr::null(),
                0,
                ptr::null(),
                start.as_ptr(),
                end.as_ptr(),
                ptr::null_mut(),
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            (api.window_create)(
                1,
                ptr::null(),
                0,
                ptr::null(),
                start.as_ptr(),
                end.as_ptr(),
                &mut handle,
            )
        },
        -1
    );
    assert!(handle.is_null());
    assert_eq!(
        unsafe {
            (api.window_create)(
                0,
                ptr::null(),
                1,
                ptr::null(),
                start.as_ptr(),
                end.as_ptr(),
                &mut handle,
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            (api.window_create)(
                0,
                ptr::null(),
                0,
                ptr::null(),
                ptr::null(),
                end.as_ptr(),
                &mut handle,
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            (api.window_create)(
                0,
                ptr::null(),
                0,
                ptr::null(),
                start.as_ptr(),
                start.as_ptr(),
                &mut handle,
            )
        },
        -1
    );
    handle = window(None, None, "09:00", "10:00");
    let mut bool_out = 9;
    assert_eq!(
        unsafe { (api.window_clone)(ptr::null(), &mut ptr::null_mut()) },
        -1
    );
    assert_eq!(unsafe { (api.window_clone)(handle, ptr::null_mut()) }, -1);
    assert_eq!(
        unsafe { (api.window_equal)(ptr::null(), handle, &mut bool_out) },
        -1
    );
    assert_eq!(
        unsafe { (api.window_matches)(handle, &bad_civil, &mut bool_out) },
        -1
    );
    assert_eq!(
        unsafe { (api.window_matches)(handle, &good, ptr::null_mut()) },
        -1
    );
    assert_eq!(
        unsafe { (api.window_overlaps)(handle, ptr::null(), &mut bool_out) },
        -1
    );
    let late = window(None, None, "11:00", "12:00");
    assert_eq!(
        unsafe { (api.window_overlaps)(late, handle, &mut bool_out) },
        0
    );
    assert_eq!(bool_out, 0);
    let _ = boundary(api.window_next_start, handle, &good, 0);
    let mut elapsed = 9;
    assert_eq!(
        unsafe {
            (api.window_elapsed_after_end)(handle, &bad_civil, 0, &mut present, &mut elapsed)
        },
        -1
    );
    assert_eq!(
        unsafe { (api.window_elapsed_after_end)(handle, &good, 0, ptr::null_mut(), &mut elapsed) },
        -1
    );
    let (mut date, mut remaining) = (9, 9);
    assert_eq!(
        unsafe {
            (api.window_next_start)(
                ptr::null(),
                &good,
                0,
                &mut present,
                &mut date,
                &mut remaining,
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            (api.window_next_end)(
                handle,
                &bad_civil,
                0,
                &mut present,
                &mut date,
                &mut remaining,
            )
        },
        -1
    );
    assert_eq!(
        unsafe {
            (api.window_next_start)(handle, &good, 60, &mut present, &mut date, &mut remaining)
        },
        0
    );
    assert_eq!((present, date, remaining), (0, 0, 0));
    unsafe {
        (api.window_destroy)(late);
        (api.window_destroy)(handle);
        (api.window_destroy)(ptr::null_mut());
    }
}

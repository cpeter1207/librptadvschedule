//! Generic local civil-time scheduling behind an independently versioned C ABI.

#![deny(warnings)]
#![cfg_attr(coverage, feature(coverage_attribute))]

pub mod schedule;

use schedule::{CivilTime, ScheduledEvent, ScheduledWindow, Weekday};
use std::{
    ffi::{CStr, c_char},
    mem::size_of,
    str::FromStr,
};

const ABI_VERSION: u32 = 1;
const CAPABILITY_NAME: &[u8] = b"rptadv.schedule\0";

/// Explicit local civil minute with a Sunday-zero weekday.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ScheduleCivil {
    year: u16,
    month: u8,
    day: u8,
    weekday: u8,
    hour: u8,
    minute: u8,
}

/// Parsed event fields: kind 1 daily, 2 weekly, or 3 one-time.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ScheduleEvent {
    kind: u32,
    year: u16,
    month: u8,
    day: u8,
    weekday: u8,
    hour: u8,
    minute: u8,
}

/// Opaque owned schedule window.
pub struct ScheduleWindowHandle(ScheduledWindow);

/// ABI 1 generic civil scheduling function table.
#[repr(C)]
pub struct ScheduleDescriptor {
    struct_size: u32,
    abi_version: u32,
    capability_name: *const c_char,
    civil_new: unsafe extern "C" fn(u16, u8, u8, u8, u8, u8, *mut ScheduleCivil) -> i32,
    event_parse: unsafe extern "C" fn(*const c_char, *mut ScheduleEvent) -> i32,
    event_occurrence:
        unsafe extern "C" fn(*const ScheduleEvent, *const ScheduleCivil, *mut u32, *mut u64) -> i32,
    start_time_valid: unsafe extern "C" fn(*const c_char) -> u32,
    end_time_valid: unsafe extern "C" fn(*const c_char) -> u32,
    weekday_selector_valid: unsafe extern "C" fn(*const c_char) -> u32,
    date_selector_valid: unsafe extern "C" fn(*const c_char) -> u32,
    window_create: unsafe extern "C" fn(
        u32,
        *const c_char,
        u32,
        *const c_char,
        *const c_char,
        *const c_char,
        *mut *mut ScheduleWindowHandle,
    ) -> i32,
    window_destroy: unsafe extern "C" fn(*mut ScheduleWindowHandle),
    window_clone:
        unsafe extern "C" fn(*const ScheduleWindowHandle, *mut *mut ScheduleWindowHandle) -> i32,
    window_equal: unsafe extern "C" fn(
        *const ScheduleWindowHandle,
        *const ScheduleWindowHandle,
        *mut u32,
    ) -> i32,
    window_matches:
        unsafe extern "C" fn(*const ScheduleWindowHandle, *const ScheduleCivil, *mut u32) -> i32,
    window_overlaps: unsafe extern "C" fn(
        *const ScheduleWindowHandle,
        *const ScheduleWindowHandle,
        *mut u32,
    ) -> i32,
    window_elapsed_after_end: unsafe extern "C" fn(
        *const ScheduleWindowHandle,
        *const ScheduleCivil,
        u8,
        *mut u32,
        *mut u64,
    ) -> i32,
    window_next_start: unsafe extern "C" fn(
        *const ScheduleWindowHandle,
        *const ScheduleCivil,
        u8,
        *mut u32,
        *mut u64,
        *mut u64,
    ) -> i32,
    window_next_end: unsafe extern "C" fn(
        *const ScheduleWindowHandle,
        *const ScheduleCivil,
        u8,
        *mut u32,
        *mut u64,
        *mut u64,
    ) -> i32,
}

// SAFETY: the descriptor and every address it references are immutable.
unsafe impl Sync for ScheduleDescriptor {}

static DESCRIPTOR: ScheduleDescriptor = ScheduleDescriptor {
    struct_size: size_of::<ScheduleDescriptor>() as u32,
    abi_version: ABI_VERSION,
    capability_name: CAPABILITY_NAME.as_ptr().cast::<c_char>(),
    civil_new,
    event_parse,
    event_occurrence,
    start_time_valid,
    end_time_valid,
    weekday_selector_valid,
    date_selector_valid,
    window_create,
    window_destroy,
    window_clone,
    window_equal,
    window_matches,
    window_overlaps,
    window_elapsed_after_end,
    window_next_start,
    window_next_end,
};

/// Return the immutable ABI 1 schedule descriptor.
#[unsafe(no_mangle)]
pub extern "C" fn rptadv_schedule_descriptor() -> *const ScheduleDescriptor {
    &DESCRIPTOR
}

fn weekday(value: u8) -> Option<Weekday> {
    Some(match value {
        0 => Weekday::Sunday,
        1 => Weekday::Monday,
        2 => Weekday::Tuesday,
        3 => Weekday::Wednesday,
        4 => Weekday::Thursday,
        5 => Weekday::Friday,
        6 => Weekday::Saturday,
        _ => return None,
    })
}

fn civil(record: &ScheduleCivil) -> Option<CivilTime> {
    CivilTime::new(
        record.year,
        record.month,
        record.day,
        weekday(record.weekday)?,
        record.hour,
        record.minute,
    )
    .ok()
}

// SAFETY: a non-null pointer must name a valid NUL-terminated C string.
unsafe fn text<'a>(value: *const c_char) -> Option<&'a str> {
    if value.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(value) }.to_str().ok()
    }
}

unsafe extern "C" fn civil_new(
    year: u16,
    month: u8,
    day: u8,
    weekday_value: u8,
    hour: u8,
    minute: u8,
    out: *mut ScheduleCivil,
) -> i32 {
    let (Some(out), Some(weekday)) = (unsafe { out.as_mut() }, weekday(weekday_value)) else {
        return -1;
    };
    let Ok(local) = CivilTime::new(year, month, day, weekday, hour, minute) else {
        return -1;
    };
    let (year, month, day, weekday, hour, minute) = local.components();
    *out = ScheduleCivil {
        year,
        month,
        day,
        weekday: weekday as u8,
        hour,
        minute,
    };
    0
}

unsafe extern "C" fn event_parse(value: *const c_char, out: *mut ScheduleEvent) -> i32 {
    let (Some(value), Some(out)) = (unsafe { text(value) }, unsafe { out.as_mut() }) else {
        return -1;
    };
    let Ok(event) = ScheduledEvent::from_str(value) else {
        return -1;
    };
    *out = match event {
        ScheduledEvent::Daily { hour, minute } => ScheduleEvent {
            kind: 1,
            hour,
            minute,
            ..Default::default()
        },
        ScheduledEvent::Weekly {
            weekday,
            hour,
            minute,
        } => ScheduleEvent {
            kind: 2,
            weekday: weekday as u8,
            hour,
            minute,
            ..Default::default()
        },
        ScheduledEvent::Once {
            year,
            month,
            day,
            hour,
            minute,
        } => ScheduleEvent {
            kind: 3,
            year,
            month,
            day,
            hour,
            minute,
            ..Default::default()
        },
    };
    0
}

fn event(record: &ScheduleEvent) -> Option<ScheduledEvent> {
    if record.hour >= 24 || record.minute >= 60 {
        return None;
    }
    Some(match record.kind {
        1 => ScheduledEvent::Daily {
            hour: record.hour,
            minute: record.minute,
        },
        2 => ScheduledEvent::Weekly {
            weekday: weekday(record.weekday)?,
            hour: record.hour,
            minute: record.minute,
        },
        3 => {
            CivilTime::new(
                record.year,
                record.month,
                record.day,
                Weekday::Sunday,
                record.hour,
                record.minute,
            )
            .ok()?;
            ScheduledEvent::Once {
                year: record.year,
                month: record.month,
                day: record.day,
                hour: record.hour,
                minute: record.minute,
            }
        }
        _ => return None,
    })
}

unsafe extern "C" fn event_occurrence(
    event_record: *const ScheduleEvent,
    civil_record: *const ScheduleCivil,
    present: *mut u32,
    occurrence: *mut u64,
) -> i32 {
    let (Some(event_record), Some(civil_record), Some(present), Some(occurrence)) = (
        unsafe { event_record.as_ref() },
        unsafe { civil_record.as_ref() },
        unsafe { present.as_mut() },
        unsafe { occurrence.as_mut() },
    ) else {
        return -1;
    };
    let (Some(event), Some(local)) = (event(event_record), civil(civil_record)) else {
        return -1;
    };
    let result = event.occurrence(&local);
    *present = u32::from(result.is_some());
    *occurrence = result.unwrap_or(0);
    0
}

unsafe extern "C" fn start_time_valid(value: *const c_char) -> u32 {
    u32::from(unsafe { text(value) }.is_some_and(schedule::start_time_valid))
}
unsafe extern "C" fn end_time_valid(value: *const c_char) -> u32 {
    u32::from(unsafe { text(value) }.is_some_and(schedule::end_time_valid))
}
unsafe extern "C" fn weekday_selector_valid(value: *const c_char) -> u32 {
    u32::from(unsafe { text(value) }.is_some_and(schedule::weekday_selector_valid))
}
unsafe extern "C" fn date_selector_valid(value: *const c_char) -> u32 {
    u32::from(unsafe { text(value) }.is_some_and(schedule::date_selector_valid))
}

unsafe extern "C" fn window_create(
    days_present: u32,
    days: *const c_char,
    dates_present: u32,
    dates: *const c_char,
    start: *const c_char,
    end: *const c_char,
    out: *mut *mut ScheduleWindowHandle,
) -> i32 {
    let Some(out) = (unsafe { out.as_mut() }) else {
        return -1;
    };
    *out = std::ptr::null_mut();
    let days = if days_present == 0 {
        Some(None)
    } else {
        unsafe { text(days) }.map(Some)
    };
    let dates = if dates_present == 0 {
        Some(None)
    } else {
        unsafe { text(dates) }.map(Some)
    };
    let (Some(days), Some(dates), Some(start), Some(end)) =
        (days, dates, unsafe { text(start) }, unsafe { text(end) })
    else {
        return -1;
    };
    let Ok(window) = ScheduledWindow::parse(days, dates, start, end) else {
        return -1;
    };
    *out = Box::into_raw(Box::new(ScheduleWindowHandle(window)));
    0
}

unsafe extern "C" fn window_destroy(handle: *mut ScheduleWindowHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

unsafe extern "C" fn window_clone(
    handle: *const ScheduleWindowHandle,
    out: *mut *mut ScheduleWindowHandle,
) -> i32 {
    let (Some(handle), Some(out)) = (unsafe { handle.as_ref() }, unsafe { out.as_mut() }) else {
        return -1;
    };
    *out = Box::into_raw(Box::new(ScheduleWindowHandle(handle.0.clone())));
    0
}

unsafe extern "C" fn window_equal(
    left: *const ScheduleWindowHandle,
    right: *const ScheduleWindowHandle,
    out: *mut u32,
) -> i32 {
    let (Some(left), Some(right), Some(out)) = (
        unsafe { left.as_ref() },
        unsafe { right.as_ref() },
        unsafe { out.as_mut() },
    ) else {
        return -1;
    };
    *out = u32::from(left.0 == right.0);
    0
}

unsafe extern "C" fn window_matches(
    handle: *const ScheduleWindowHandle,
    civil_record: *const ScheduleCivil,
    out: *mut u32,
) -> i32 {
    let (Some(handle), Some(civil_record), Some(out)) = (
        unsafe { handle.as_ref() },
        unsafe { civil_record.as_ref() },
        unsafe { out.as_mut() },
    ) else {
        return -1;
    };
    let Some(local) = civil(civil_record) else {
        return -1;
    };
    *out = u32::from(handle.0.matches(&local));
    0
}

unsafe extern "C" fn window_overlaps(
    left: *const ScheduleWindowHandle,
    right: *const ScheduleWindowHandle,
    out: *mut u32,
) -> i32 {
    let (Some(left), Some(right), Some(out)) = (
        unsafe { left.as_ref() },
        unsafe { right.as_ref() },
        unsafe { out.as_mut() },
    ) else {
        return -1;
    };
    *out = u32::from(left.0.overlaps(&right.0));
    0
}

unsafe extern "C" fn window_elapsed_after_end(
    handle: *const ScheduleWindowHandle,
    civil_record: *const ScheduleCivil,
    second: u8,
    present: *mut u32,
    elapsed: *mut u64,
) -> i32 {
    let (Some(handle), Some(civil_record), Some(present), Some(elapsed)) = (
        unsafe { handle.as_ref() },
        unsafe { civil_record.as_ref() },
        unsafe { present.as_mut() },
        unsafe { elapsed.as_mut() },
    ) else {
        return -1;
    };
    let Some(local) = civil(civil_record) else {
        return -1;
    };
    let result = handle.0.elapsed_after_end(&local, second);
    *present = u32::from(result.is_some());
    *elapsed = result.unwrap_or(0);
    0
}

unsafe extern "C" fn window_next_start(
    handle: *const ScheduleWindowHandle,
    civil_record: *const ScheduleCivil,
    second: u8,
    present: *mut u32,
    date: *mut u64,
    remaining: *mut u64,
) -> i32 {
    unsafe {
        window_boundary(
            handle,
            civil_record,
            second,
            present,
            date,
            remaining,
            ScheduledWindow::next_start,
        )
    }
}

unsafe extern "C" fn window_next_end(
    handle: *const ScheduleWindowHandle,
    civil_record: *const ScheduleCivil,
    second: u8,
    present: *mut u32,
    date: *mut u64,
    remaining: *mut u64,
) -> i32 {
    unsafe {
        window_boundary(
            handle,
            civil_record,
            second,
            present,
            date,
            remaining,
            ScheduledWindow::next_end,
        )
    }
}

unsafe fn window_boundary(
    handle: *const ScheduleWindowHandle,
    civil_record: *const ScheduleCivil,
    second: u8,
    present: *mut u32,
    date: *mut u64,
    remaining: *mut u64,
    calculate: fn(&ScheduledWindow, &CivilTime, u8) -> Option<(u64, u64)>,
) -> i32 {
    let (Some(handle), Some(civil_record), Some(present), Some(date), Some(remaining)) = (
        unsafe { handle.as_ref() },
        unsafe { civil_record.as_ref() },
        unsafe { present.as_mut() },
        unsafe { date.as_mut() },
        unsafe { remaining.as_mut() },
    ) else {
        return -1;
    };
    let Some(local) = civil(civil_record) else {
        return -1;
    };
    let result = calculate(&handle.0, &local, second);
    *present = u32::from(result.is_some());
    (*date, *remaining) = result.unwrap_or((0, 0));
    0
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod abi_tests;

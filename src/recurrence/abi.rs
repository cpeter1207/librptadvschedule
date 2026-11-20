//! Independent C table for immutable compiled recurrence handles.
use super::{Query, Recurrence};
use std::{ffi::c_char, mem::size_of, ptr, slice, str};

const ARGUMENT: i32 = -1;
const TEXT: i32 = -2;

type QueryFn = unsafe extern "C" fn(*const Recurrence, i64, *mut i64) -> i32;

/// The field order and C declarations live in rptadvschedule/recurrence.h.
#[repr(C)]
pub struct Descriptor {
    struct_size: u32,
    abi_version: u32,
    capability_name: *const c_char,
    min_second: i64,
    end_second: i64,
    create: unsafe extern "C" fn(*const u8, u32, *const u8, u32, *mut *mut Recurrence) -> i32,
    destroy: unsafe extern "C" fn(*mut Recurrence),
    clone_handle: unsafe extern "C" fn(*const Recurrence, *mut *mut Recurrence) -> i32,
    equal: unsafe extern "C" fn(*const Recurrence, *const Recurrence, *mut u32) -> i32,
    effective_zone: unsafe extern "C" fn(*const Recurrence) -> *const c_char,
    match_minute: QueryFn,
    next_after: QueryFn,
    previous_before: QueryFn,
}

// SAFETY: All table entries refer to static read-only data or functions.
unsafe impl Sync for Descriptor {}

static TABLE: Descriptor = Descriptor {
    struct_size: size_of::<Descriptor>() as u32,
    abi_version: 1,
    capability_name: c"rptadv.recurrence".as_ptr(),
    min_second: super::MIN_SECOND,
    end_second: super::END_SECOND,
    create,
    destroy,
    clone_handle,
    equal,
    effective_zone,
    match_minute,
    next_after,
    previous_before,
};

/// Return the process-lifetime recurrence table. Validate it before use.
#[unsafe(no_mangle)]
pub extern "C" fn rptadv_recurrence_descriptor() -> *const Descriptor {
    &TABLE
}

/// A non-empty span must point to that many readable bytes. Check the cap
/// before touching caller memory. NULL/0 represents an absent span.
unsafe fn text<'a>(data: *const u8, len: u32, limit: u32) -> Result<&'a str, i32> {
    if len > limit {
        return Err(TEXT);
    }
    if len == 0 {
        return Ok("");
    }
    if data.is_null() {
        return Err(ARGUMENT);
    }
    let bytes = unsafe { slice::from_raw_parts(data, len as usize) };
    if bytes.contains(&0) {
        return Err(TEXT);
    }
    str::from_utf8(bytes).map_err(|_| TEXT)
}

unsafe extern "C" fn create(
    data: *const u8,
    len: u32,
    zone: *const u8,
    zone_len: u32,
    out: *mut *mut Recurrence,
) -> i32 {
    if out.is_null() {
        return ARGUMENT;
    }
    unsafe { out.write(ptr::null_mut()) };
    let result = (|| {
        let source = unsafe { text(data, len, 16_384) }?;
        if !(1..=4096).contains(&source.chars().count()) {
            return Err(TEXT);
        }
        let host_zone = unsafe { text(zone, zone_len, 255) }?;
        Recurrence::compile(source, host_zone)
    })();
    match result {
        Ok(value) => {
            unsafe { out.write(Box::into_raw(Box::new(value))) };
            0
        }
        Err(status) => status,
    }
}

unsafe extern "C" fn destroy(handle: *mut Recurrence) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

unsafe extern "C" fn clone_handle(handle: *const Recurrence, out: *mut *mut Recurrence) -> i32 {
    if out.is_null() {
        return ARGUMENT;
    }
    unsafe { out.write(ptr::null_mut()) };
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ARGUMENT;
    };
    unsafe { out.write(Box::into_raw(Box::new(handle.clone()))) };
    0
}

unsafe extern "C" fn equal(
    left: *const Recurrence,
    right: *const Recurrence,
    out: *mut u32,
) -> i32 {
    if out.is_null() {
        return ARGUMENT;
    }
    unsafe { out.write(0) };
    let (Some(left), Some(right)) = (unsafe { left.as_ref() }, unsafe { right.as_ref() }) else {
        return ARGUMENT;
    };
    unsafe { out.write(u32::from(left.same_meaning(right))) };
    0
}

unsafe extern "C" fn effective_zone(handle: *const Recurrence) -> *const c_char {
    unsafe { handle.as_ref() }.map_or(ptr::null(), |h| h.zone_name.as_ptr())
}

unsafe fn query(handle: *const Recurrence, second: i64, out: *mut i64, kind: Query) -> i32 {
    if out.is_null() {
        return ARGUMENT;
    }
    unsafe { out.write(0) };
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ARGUMENT;
    };
    match handle.query(second, kind) {
        Ok(Some(occurrence)) => {
            unsafe { out.write(occurrence) };
            1
        }
        Ok(None) => 0,
        Err(status) => status,
    }
}
unsafe extern "C" fn match_minute(h: *const Recurrence, s: i64, out: *mut i64) -> i32 {
    unsafe { query(h, s, out, Query::Minute) }
}
unsafe extern "C" fn next_after(h: *const Recurrence, s: i64, out: *mut i64) -> i32 {
    unsafe { query(h, s, out, Query::Next) }
}
unsafe extern "C" fn previous_before(h: *const Recurrence, s: i64, out: *mut i64) -> i32 {
    unsafe { query(h, s, out, Query::Previous) }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn checked_owned_handles_and_bad_outputs() {
        let api = unsafe { &*rptadv_recurrence_descriptor() };
        let s = b"every day at 0:00";
        let z = b"UTC";
        let mut h = ptr::null_mut();
        let mut copy = ptr::null_mut();
        let mut value = 9;
        let mut equal = 9;
        unsafe {
            assert_eq!(
                (api.create)(s.as_ptr(), s.len() as u32, z.as_ptr(), 3, &mut h),
                0
            );
            assert_eq!(CStr::from_ptr((api.effective_zone)(h)), c"UTC");
            assert_eq!((api.clone_handle)(h, &mut copy), 0);
            assert_eq!((api.equal)(h, copy, &mut equal), 0);
            assert_eq!(equal, 1);
            (api.destroy)(h);
            assert_eq!((api.match_minute)(copy, 23, &mut value), 1);
            assert_eq!(value, 0);
            assert_eq!((api.next_after)(copy, 0, &mut value), 1);
            assert_eq!(value, 86_400);
            assert_eq!((api.previous_before)(copy, 0, &mut value), 1);
            assert_eq!(value, -86_400);
            assert_eq!((api.match_minute)(copy, 80, &mut value), 0);
            assert_eq!(value, 0);
            assert_eq!(
                (api.next_after)(copy, api.end_second, &mut value),
                super::super::RANGE
            );
            assert_eq!((api.match_minute)(copy, 0, ptr::null_mut()), ARGUMENT);
            assert_eq!((api.previous_before)(ptr::null(), 0, &mut value), ARGUMENT);
            assert_eq!((api.clone_handle)(copy, ptr::null_mut()), ARGUMENT);
            assert_eq!((api.clone_handle)(ptr::null(), &mut h), ARGUMENT);
            assert!(h.is_null());
            assert_eq!((api.equal)(copy, copy, ptr::null_mut()), ARGUMENT);
            assert_eq!((api.equal)(ptr::null(), copy, &mut equal), ARGUMENT);
            assert_eq!((api.equal)(copy, ptr::null(), &mut equal), ARGUMENT);
            assert_eq!(equal, 0);
            assert!((api.effective_zone)(ptr::null()).is_null());
            assert_eq!(
                (api.create)(s.as_ptr(), s.len() as u32, z.as_ptr(), 3, ptr::null_mut()),
                ARGUMENT
            );
            for (data, len, expected) in [
                (s.as_ptr(), 16385, TEXT),
                (ptr::null(), 0, TEXT),
                (ptr::null(), 1, ARGUMENT),
                (c"".as_ptr().cast(), 1, TEXT),
                (b"\xff".as_ptr(), 1, TEXT),
                (b"nonsense".as_ptr(), 8, super::super::PARSE),
            ] {
                assert_eq!((api.create)(data, len, z.as_ptr(), 3, &mut h), expected);
                assert!(h.is_null());
            }
            assert_eq!(
                (api.create)(s.as_ptr(), s.len() as u32, ptr::null(), 1, &mut h),
                ARGUMENT
            );
            (api.destroy)(copy);
            (api.destroy)(ptr::null_mut());
        }
    }
}

//! Prepared hron schedule with one immutable timezone and bounded occurrence queries.
use hron::{
    CapturedQuery as Query, Schedule, ScheduleExpr,
    ast::{DateSpec, Exception, UntilSpec},
    captured_occurrence, normalize_sets,
};
use jiff::{Timestamp, Zoned, civil::Date, tz::TimeZone};
use std::ffi::CString;

mod abi;

const MIN_SECOND: i64 = -62_135_510_400;
const END_SECOND: i64 = 253_402_128_000;
const MAX_WORK: usize = 16_384;
const ZONE: i32 = -3;
const PARSE: i32 = -4;
const RANGE: i32 = -5;
const LIMIT: i32 = -6;

#[derive(Clone)]
struct Recurrence {
    effective: Schedule,
    zone: TimeZone,
    zone_name: CString,
}

impl Recurrence {
    fn compile(source: &str, host_zone: &str) -> Result<Self, i32> {
        let mut parts = Schedule::parse(source).map_err(|_| PARSE)?.to_parts();
        if parts.timezone.is_none() {
            parts.timezone = Some(host_zone.to_owned());
        }
        normalize_sets(&mut parts);
        let effective = Schedule::from_parts(parts).map_err(|_| ZONE)?;
        let name = effective.timezone().expect("explicitly bound timezone");
        let zone = TimeZone::get(name).map_err(|_| ZONE)?;
        let zone_name = CString::new(name).expect("validated IANA name");
        check_dates(&effective, &zone)?;
        Ok(Self {
            effective,
            zone,
            zone_name,
        })
    }

    fn same_meaning(&self, other: &Self) -> bool {
        self.effective == other.effective && self.zone == other.zone
    }

    fn query(&self, second: i64, kind: Query) -> Result<Option<i64>, i32> {
        self.query_with_budget(second, kind, MAX_WORK)
    }

    fn query_with_budget(
        &self,
        second: i64,
        kind: Query,
        budget: usize,
    ) -> Result<Option<i64>, i32> {
        if !(MIN_SECOND..END_SECOND).contains(&second) {
            return Err(RANGE);
        }
        let now = instant(second, &self.zone);
        captured_occurrence(&self.effective, &now, kind, budget)
            .map(|found| found.map(|z| z.timestamp().as_second()))
            .map_err(|_| LIMIT)
    }
}

fn instant(second: i64, zone: &TimeZone) -> Zoned {
    Timestamp::from_second(second)
        .expect("checked hron range")
        .to_zoned(zone.clone())
}

fn check_dates(schedule: &Schedule, zone: &TimeZone) -> Result<(), i32> {
    let first = instant(MIN_SECOND, zone).date();
    let last = instant(END_SECOND - 1, zone).date();
    let check = |date: Date| (first..=last).contains(&date).then_some(()).ok_or(RANGE);
    let date = |s: &str| s.parse::<Date>().expect("validated ISO date");
    if let Some(start) = schedule.starting() {
        check(start)?;
    }
    if let Some(UntilSpec::Iso(s)) = schedule.until() {
        check(date(s))?;
    }
    for ex in schedule.except() {
        if let Exception::Iso(s) = ex {
            check(date(s))?;
        }
    }
    if let ScheduleExpr::SingleDate {
        date: DateSpec::Iso(s),
        times,
    } = schedule.expression()
    {
        let date = date(s);
        check(date)?;
        for time in times {
            let stamp = date
                .at(time.hour as i8, time.minute as i8, 0, 0)
                .to_zoned(zone.clone())
                .map_err(|_| RANGE)?
                .timestamp()
                .as_second();
            if !(MIN_SECOND..END_SECOND).contains(&stamp) {
                return Err(RANGE);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests;

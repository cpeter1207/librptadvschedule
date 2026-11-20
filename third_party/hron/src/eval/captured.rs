// SPDX-License-Identifier: GPL-2.0-only
//! Downstream captured-zone queries. The unmodified public entry points use
//! native DST resolution and traverse their original complete search horizon.
use super::*;
use std::cell::Cell;

/// Select the current local minute or a strictly adjacent occurrence.
#[derive(Clone, Copy)]
pub enum CapturedQuery {
    Minute,
    Next,
    Previous,
}

/// The search budget was consumed before an answer could be proven.
#[derive(Debug)]
pub struct SearchLimit;

/// Evaluate using only the caller's retained timezone. Budget errors never
/// return a partial nearest candidate. Fixed and interval slots skip gaps.
pub fn captured_occurrence(
    schedule: &Schedule,
    now: &Zoned,
    query: CapturedQuery,
    budget: usize,
) -> Result<Option<Zoned>, SearchLimit> {
    if !in_supported_range(now) {
        return Ok(None);
    }
    let work = SearchWork::limited(budget);
    let mut search = Search::in_zone(schedule, now.time_zone().clone());
    let result = match query {
        CapturedQuery::Minute => matches_in(&mut search, now, &work).then(|| {
            now.checked_sub(
                Span::new()
                    .seconds(now.second())
                    .nanoseconds(now.time().subsec_nanosecond()),
            )
            .expect("a supported instant has a day of margin")
        }),
        CapturedQuery::Next => search.nearest_with_work(now, Direction::Forward, &work),
        CapturedQuery::Previous => search.nearest_with_work(now, Direction::Backward, &work),
    };
    if work.failed.get() {
        Err(SearchLimit)
    } else {
        Ok(result)
    }
}

/// None selects unchanged upstream behavior. A finite balance also selects
/// the captured provider's first-fold/skip-gap compatibility policy.
pub(super) struct SearchWork {
    remaining: Cell<Option<usize>>,
    failed: Cell<bool>,
}
impl SearchWork {
    pub(super) fn native() -> Self {
        Self {
            remaining: Cell::new(None),
            failed: Cell::new(false),
        }
    }
    fn limited(units: usize) -> Self {
        Self {
            remaining: Cell::new(Some(units)),
            failed: Cell::new(false),
        }
    }
    pub(super) fn charge(&self, units: usize) -> bool {
        let Some(remaining) = self.remaining.get() else {
            return true;
        };
        if let Some(next) = remaining.checked_sub(units) {
            self.remaining.set(Some(next));
            true
        } else {
            self.failed.set(true);
            false
        }
    }
    pub(super) fn period_bound(
        &self,
        cadence: &Cadence,
        clauses: &Clauses,
        zone: &TimeZone,
        direction: Direction,
    ) -> Option<i64> {
        self.remaining.get()?;
        // Nearest-weekday adjustment can cross one period boundary. Keeping
        // that neighboring period preserves every candidate inside the clause.
        // A Gregorian cycle does not prove that historical timezone rules
        // repeat. Search through the clause/range endpoint or report limit.
        let date = match direction {
            Direction::Forward => clauses
                .until
                .unwrap_or_else(|| RANGE_END.to_zoned(zone.clone()).date()),
            Direction::Backward => clauses
                .starting
                .unwrap_or_else(|| RANGE_START.to_zoned(zone.clone()).date()),
        };
        Some(cadence.period_of(date) + direction.sign())
    }
    pub(super) fn visit_period(
        &self,
        period: i64,
        bound: Option<i64>,
        direction: Direction,
    ) -> bool {
        bound.is_none_or(|bound| !direction.precedes(&bound, &period)) && self.charge(1)
    }
    pub(super) fn charge_date(&self, times: &DailyTimes) -> bool {
        // Fixed lists make one timezone query per time. Interval lists use a
        // partition search followed by a scan across at most every slot.
        self.charge(match times {
            DailyTimes::Fixed(times) => times.len(),
            DailyTimes::Slots(slots) => slots.len() + slots.len().ilog2() as usize + 1,
        })
    }
    pub(super) fn fixed_time_on(&self, date: Date, time: Time, zone: &TimeZone) -> Option<Zoned> {
        let candidate = fixed_time_on(date, time, zone)?;
        if self.remaining.get().is_none() || candidate.datetime() == date.to_datetime(time) {
            Some(candidate)
        } else {
            None
        }
    }
}

/// Put every set-valued part into canonical order before compiling a handle.
/// Date and time evaluation uses membership in these lists.
pub fn normalize_sets(parts: &mut ScheduleParts) {
    fn days(list: &mut Vec<Weekday>) {
        list.sort_unstable_by_key(|d| d.number());
        list.dedup();
    }
    fn filter(f: &mut DayFilter) {
        if let DayFilter::Days(list) = f {
            days(list);
        }
    }
    let times = match &mut parts.expression {
        ScheduleExpr::DayRepeat { days, times, .. } => {
            filter(days);
            Some(times)
        }
        ScheduleExpr::WeekRepeat {
            days: list, times, ..
        } => {
            days(list);
            Some(times)
        }
        ScheduleExpr::MonthRepeat { target, times, .. } => {
            if let MonthTarget::Days(specs) = target {
                let mut expanded: Vec<_> = specs.iter().flat_map(DayOfMonthSpec::expand).collect();
                expanded.sort_unstable();
                expanded.dedup();
                *specs = expanded.into_iter().map(DayOfMonthSpec::Single).collect();
            }
            Some(times)
        }
        ScheduleExpr::YearRepeat { times, .. } | ScheduleExpr::SingleDate { times, .. } => {
            Some(times)
        }
        ScheduleExpr::IntervalRepeat { day_filter, .. } => {
            if let Some(f) = day_filter {
                filter(f);
            }
            None
        }
    };
    if let Some(times) = times {
        times.sort_unstable();
        times.dedup();
    }
    parts.during.sort_unstable_by_key(|m| m.number());
    parts.during.dedup();
    fn exception_key(e: &Exception) -> (u8, u8, &str) {
        match e {
            Exception::Named { month, day } => (month.number(), *day, ""),
            Exception::Iso(s) => (0, 0, s),
        }
    }
    parts
        .except
        .sort_unstable_by(|a, b| exception_key(a).cmp(&exception_key(b)));
    parts.except.dedup();
}

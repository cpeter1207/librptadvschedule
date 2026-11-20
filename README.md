# librptadvschedule

Generic local civil-time scheduling for rpt_advanced. The immutable ABI 1
descriptor exposes civil and event records plus an opaque, cloneable window.
Callers supply the civil minute and second; this library owns neither clocks
nor controller actions. Optional selectors and results use explicit presence
flags. The end of each bounded same-date window is exclusive.

Elapsed-end results measure civil milliseconds since the most recent selected
window ended, including across midnight and unselected calendar days. Active
windows, times before every selected end, and invalid seconds have no result.
This correction preserves ABI 1 and the existing SONAME.

The parser, calculations, and calendar tests originate from
`rpt_advanced` commit `c63db09`,
`rust/core/src/schedule.rs` and `rust/core/src/schedule/tests.rs`. The
`ScheduledAction` parser and action-vocabulary test remain in the controller.
The moved calculations retain Sunday-zero weekdays, date limits, and the
`(date identity, remaining milliseconds)` next-boundary result.

The alpha.3 source adds an independent hron recurrence table for captured Unix
seconds. The two C contracts are `include/rptadvschedule/rptadvschedule.h`
and `include/rptadvschedule/recurrence.h`. Both use the existing
`librptadvschedule.so.1` SONAME. See `QUALITY.md` for the timezone, search
and build contracts. Build and test through the repository's `Makefile`.

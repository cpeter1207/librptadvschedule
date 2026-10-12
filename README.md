# librptadvschedule

Generic local civil-time scheduling for rpt_advanced. The immutable ABI 1
descriptor exposes civil and event records plus an opaque, cloneable window.
Callers supply the civil minute and second; this library owns neither clocks
nor controller actions. Optional selectors and results use explicit presence
flags. The end of each bounded same-date window is exclusive.

The parser, calculations, and calendar tests originate from
`rpt_advanced` commit `c63db09`,
`rust/core/src/schedule.rs` and `rust/core/src/schedule/tests.rs`. The
`ScheduledAction` parser and action-vocabulary test remain in the controller.
The moved calculations retain Sunday-zero weekdays, date limits, and the
`(date identity, remaining milliseconds)` next-boundary result.

Build and test through the repository's `Makefile`; the public contract is
`include/rptadvschedule/rptadvschedule.h`. The SONAME is
`librptadvschedule.so.1`, and the first release is `0.1.0-alpha.1`.

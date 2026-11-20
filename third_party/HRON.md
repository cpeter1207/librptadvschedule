# Pinned hron source

`hron/` contains the MIT-licensed hron 2.2.1 crate from crates.io. The
unmodified archive SHA-256 is
`1ab1a002aa9125ac92cc0e9cc784ed4c1858610afc162b0e61ef4430a96dbc3a`.
The published VCS revision is `cfe57bc4c1cde3d79d4289b3cc247d6407e9da4f`,
at `rust/hron` in [simpllyf/hron](https://github.com/simpllyf/hron).
The upstream license is in `hron/LICENSE`. The provider-owned changes are
covered by the provider's GPL-2.0-only license. Cargo's local patch override
ensures direct Cargo and Debian builds use the same reviewed engine.

To reproduce the changes, compare a hash-verified extraction of
[the original crate](https://static.crates.io/crates/hron/hron-2.2.1.crate)
with `hron/`. The modifications to existing upstream files are recorded in
`hron-2.2.1.patch`, relative to the provider root. The only added crate file
is `hron/src/eval/captured.rs`. It owns the captured evaluator and canonical
ordering of set-valued parsed parts. Keep the patch in sync when changing
upstream files. Review calendar behavior against the upstream public APIs
and the provider's Rust and C tests.

The upstream getter-based evaluator resolves a timezone name on each call.
Its public iterators apply bounds after candidate generation. They cannot
accept retained timezone rules or stop inside a candidate-free period
search. The local extension uses the same cadence, calendar and time-slot
code with a supplied zone and a per-query work budget. The upstream
occurrence iterators retain their original Send and Sync traits. Only the
captured entry point rejects fixed slots shifted into a gap.

The coverage gate inventories production Rust modules from the source tree
and requires full reported line and branch coverage for each. It reconciles
instrumented additions to the two existing engine files across LLVM's JSON
and LCOV exports. The quality check includes mutation fixtures for omitted
production modules, mapped lines and branch arms.

When an upstream release supplies the captured timezone, gap policy and
bounded period/slot evaluation, replace the override with a version pin,
keep set normalization in the provider if upstream lacks it, and exercise
the captured-rule, budget, ABI and upstream-compatibility tests. Remove
the local sources, patch and corresponding coverage selection together.

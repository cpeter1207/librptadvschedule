# Quality checks

`make quality` runs formatting, strict Clippy, C smoke-source analysis,
Doxygen, and warning-free Rustdoc. `make test` runs Rust and C ABI tests.
`make coverage` checks production Rust line and branch coverage on Debian 13
amd64. `make platform-verify` checks staged installation, Debian packages, and
the source archive on each native Debian 13 architecture.

The required GitHub pull request gate runs those checks through the shared
workflow. Run local targeted checks in the project-owned labeled container
using `tools/run-in-quality-container.sh` and a freshly pulled quality image.

## Recurrence provider

The independently discoverable `rptadv.recurrence` ABI 1 uses the same
`librptadvschedule.so.1` as the legacy window descriptor. The checked table,
status codes, ownership rules and timestamp units are in
`include/rptadvschedule/recurrence.h`. A consumer requiring calendar triggers
checks the getter, table size, version, identity and operations before
activation. Each immutable handle captures a normalized hron expression and
the effective IANA zone. Equality uses normalized set-valued parts, the bound
name and captured zone rules. All handles in this ABI use the same skip-gap/first-fold policy. The
caller retains authored text and resolves its default zone at preparation.

The provider evaluates only the supplied POSIX second. Current-minute
matching returns the real beginning of that local minute. Next and previous
queries are strict from the supplied second; for startup exactly on a
boundary, check the current minute before looking back. For an end paired
with a known start, search strictly after that actual start second. Hron's
range is [0001-01-02T00:00:00Z, 9999-12-30T00:00:00Z). Each query has
16384 work units shared by the entire native search. It pays for raw period attempts before filtering, candidate dates,
and a conservative bound on clock-slot lookups. Exhausting this budget
returns the limit status and discards any partial result. Parsed list sizes
are bounded by the expression cap. Absolute starting/until bounds also
stop candidate-free periods, including current-minute queries.

The source-owned hron 2.2.1 patch evaluates every operation in the handle's
captured Jiff timezone. Updating system tzdata can change a freshly prepared
handle; existing handles and their clones retain their captured rules.
The same hron candidate enumeration serves match and strict searches.
The captured evaluator accepts a fixed slot only when its resolved local
date/time equals its nominal slot, retaining the first fold and skipping
gaps. Interval slots already skip gaps. Hron's original public operations
retain their native gap resolution. See `third_party/HRON.md` for the patch,
provenance, removal criteria and coverage gate.

Debian runtime packages require system tzdata. Provider builds use the
pinned Rust 1.93 toolchain. The staged installation tests exercise each
descriptor through its installed header, pkg-config and dynamic SONAME.

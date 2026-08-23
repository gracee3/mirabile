# 0007: XALEN default and Swiss isolation

Status: Accepted

## Decision

The exactly pinned XALEN adapter is Mirabile's current default real browser backend. XALEN remains
an external implementation adapted to Mirabile request, result, and provenance types. Its current
feature discipline, notice assets, no-download behavior, and narrow capability surface remain
fixed for this consolidation.

Swiss Ephemeris remains optional and distribution-isolated. No Swiss code, data, native constants,
or dependency is added here.

## Consequences

XALEN and Swiss/native-library types never enter canonical structures. Swiss licensing must be
resolved at a distribution boundary before integration; a Rust crate boundary alone is not
sufficient.

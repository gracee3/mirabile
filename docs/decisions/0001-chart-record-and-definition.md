# 0001: Chart source facts and calculation semantics

Status: Accepted

## Decision

`ChartRecord` preserves factual/source assertions, including asserted time, location, provenance,
and notes. `ChartDefinition` preserves calculation and derivation semantics and may reference a
record. They remain distinct revisioned canonical resources.

## Consequences

Multiple definitions may share one record. Editing shared facts can therefore affect multiple
definitions; future UX may update the shared source or create an independent copy. Unsaved chart
creation is an application draft, not a reason to collapse these resources. Saving a draft creates
the record and definition atomically; later edits must still preserve the possibility of shared
records rather than treating the pair as one canonical object.

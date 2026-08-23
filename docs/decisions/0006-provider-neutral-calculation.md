# 0006: Provider-neutral calculation boundary

Status: Accepted

## Decision

Canonical payloads and application drafts resolve into Mirabile-owned calculation semantics.
`CalculationEngine` and the Worker/runtime contract consume those semantics and produce
`CalculationValue`. Canonical identities, titles, and revisions separately form
`SnapshotContext`.

## Consequences

No repository envelope or provider-native type crosses the Worker boundary. Calculation values
remain cacheable without making cached context stale. Unsupported provider capabilities fail
explicitly.

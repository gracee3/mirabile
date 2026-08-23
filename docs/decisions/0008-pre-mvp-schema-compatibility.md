# 0008: Pre-MVP persistence compatibility

Status: Accepted until MVP schema freeze

## Decision

Persisted schemas before the explicitly declared Mirabile MVP freeze are development formats and
may change incompatibly. Cheap, useful migration is allowed, but compatibility with every prior
development schema is not guaranteed. Incompatible data fails clearly and development IndexedDB
may be reset.

## Consequences

`SchemaVersion::V1` is not a permanent public promise. The product model can be corrected now
without a generic migration framework. A future public compatibility policy requires a separate,
explicit schema-freeze decision.

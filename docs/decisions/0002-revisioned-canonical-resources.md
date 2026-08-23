# 0002: Revisioned locally authoritative resources

Status: Accepted

## Decision

Canonical resources use stable IDs, explicit revisions, validated envelopes, and a locally
authoritative repository. Optimistic saves create the next revision; history and tombstones remain
addressable through repository semantics.

## Consequences

Canonical user intent is never replaced by a calculated value or UI projection. Sync, if added,
must compose with local revision authority rather than redefining canonical resource identity.

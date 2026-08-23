# 0004: Application and read-model authority

Status: Accepted

## Decision

One `Application` owns authoritative catalogs, repository access, resources, calculation runtime,
preferences/startup policy, workspace documents, and workspace sessions. Presentation dispatches
typed `AppIntent` and consumes authoritative read models and `Scene`; it does not mutate domain or
repository state directly.

`ProjectionVersion` orders projections within one application instance. `snapshot()` is an
immediate read and does not execute pending work. `wait_for_update()` is non-consuming observation.

## Consequences

A workspace is below the application, not synonymous with it. Multiple sessions can be added
without duplicating the catalog or calculation architecture. Latest calculation wins and the last
good scene survives refresh or failure.

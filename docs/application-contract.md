# Application interface contract

`astra-app` is the shared boundary between presentation adapters and application implementations.
The current Leptos adapter uses `MockApplication`; a later `RealApplication` must implement the
same `Application` interface and preserve the semantics below. The contract does not implement or
replace canonical domain state, repositories, calculation, analysis, or persistence.

## Authority and dependency boundary

Read models are authoritative UI projections. Components render them and dispatch `AppIntent`;
they do not mutate a local `Workspace`, `ViewInstance`, or canonical resource and assume it was
accepted. The initial `dispatch` result is a full projection for correctness. Read models may later
be queried independently rather than remaining one permanent, database-sized aggregate.

`astra-app` re-exports Astra's existing stable IDs and current astrology-free `Scene` primitives.
It does not create parallel frontend identity domains or duplicate `Scene`. Normal UI modules
depend on `astra-app`; the feature-gated IndexedDB browser-contract harness remains an isolated
test-only exception with direct foundation dependencies.

No `AppEvent` stream exists in this slice. If events are added, they announce that authoritative
application state changed; read models remain authoritative. Missing an event must be recoverable
by requesting a new projection.

## Application and view status

`ApplicationStatus::{Initializing, Ready, Error}` describes application hydration. It is
independent from `ViewComputationState`, so a ready application can contain a loading, refreshing,
or failed view without becoming a shell-level error.

`ViewReadModel` combines `scene: Option<Scene>` with:

| Scene | Computation | Meaning |
| --- | --- | --- |
| none | `Loading` | No successful computation exists yet. |
| some | `Fresh` | The Scene is current. |
| some | `Refreshing` | Display the last good Scene while new work runs. |
| some | `Failed(error)` | Keep displaying the last good Scene and surface the scoped error. |
| none | `Failed(error)` | Surface that the view has never computed successfully. |

The mock returns accepted intermediate states from `dispatch` and deterministically completes one
queued operation on the next `snapshot`. The frontend publishes the intermediate projection before
re-querying. This proves last-good-Scene behavior without sleeps and does not prescribe the future
real application's worker/event mechanism.

## Workspace selection policy

Active and selected charts are separate contract fields. The mock uses this provisional policy:

- activating a chart does not add or remove selection;
- selecting or deselecting a chart does not activate it;
- opening a library chart activates it but preserves selection;
- closing removes only that chart from selection and slot assignments;
- closing the active chart activates the next chart at the same rail position, or the preceding
  chart when there is no next chart;
- a required slot that referenced a closed chart receives the resulting active chart when one
  exists; an optional slot becomes unassigned.

There is deliberately no invariant that the active chart must be selected. Shift-range selection
is deferred. This policy is easy to revise after shared architecture review because the transitions
live in the application adapter and tests, not DOM handlers.

## Intents, drafts, and form buffers

`AppIntent` expresses user-level application intentions. It is not the persistence-oriented
`astra_core::Command`; a real adapter will translate intents into orchestration, domain commands,
repository calls, and engine work behind this boundary.

Draft mutations are typed. This slice uses `AspectSetDraftMutation::{SetOrb, SetEnabled}` with a
typed `AspectId`, `Angle`, and boolean. String field paths and untyped JSON values are not part of
the application API.

`AspectSetDraftReadModel` projects `Clean`, `Dirty`, `Saving`, and `Conflict` plus base/current or
remote revision metadata. The application owns that resource-level lifecycle. The frontend owns
only HTML mechanics such as focus, input text, temporary invalid syntax, and validation display.
It parses a valid buffer into a typed mutation; it does not create a second resource editor state
machine. Save advances the canonical mock revision and returns the draft to `Clean`; Cancel restores
the currently projected canonical value. The Wide fixture produces one deterministic save conflict
to exercise retained local draft and remote-revision UI.

Changing a valid Aspect Set draft queues a view refresh while retaining the previous `Scene`. The
mock changes opaque fixture Scene geometry only. It does not reproduce astrology or computation-key
logic; calculation identity and invalidation correctness remain foundation-owned.

## Capabilities and presentation commands

The application projects `CommandCapability { action, availability }`, where availability is
`Enabled`, `Disabled { reason }`, or `Hidden`. Only actions used by this slice exist. Disabled
reasons remain application-owned and are exposed to assistive and visual presentation.

Labels, shortcuts, grouping, keywords, and eventual command-palette presentation live in the web
adapter's command registry. The current shortcuts support primary-modifier Save, Escape Cancel
outside text-entry controls, Alt+1 chart-rail focus, and primary-modifier Refresh outside text-entry
controls. The frontend does not infer executability from draft details; it consumes capabilities.

## Error classes and interchangeability

`AppErrorKind` distinguishes initialization, view computation, conflict, invalid intent, missing
state, and unavailable operations. Initialization errors replace the shell with Retry. View errors
remain scoped to the view and retain its last successful Scene. Draft conflicts remain scoped to
the editor with both revisions visible.

Frontend construction injects an `Application` trait object. Normal shell components know only
`AppIntent`, read models, capabilities, errors, stable IDs, and `Scene`; they do not know whether the
implementation uses a mock, IndexedDB, memory, workers, a calculation engine, or future sync. A
`RealApplication` can therefore replace `MockApplication` without redesigning frontend state.

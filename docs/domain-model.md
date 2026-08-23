# Domain model

## Authoritative nouns

- `ChartRecord` preserves the asserted civil time, timezone form, coordinates, provenance, and notes.
- `ChartDefinition` references a radix record or records a derivation recipe and stores concrete calculation choices.
- `ResourceEnvelope<T>` supplies stable identity, schema version, revision, metadata, and a typed payload.
- `Workspace` is an intentional working set distinct from the resource library and can contain saved or ephemeral chart definitions.
- `ResourceBinding<T>` distinguishes following a resource, pinning a revision, and embedding a value.
- `QueryDefinition` stores a reusable boolean `QueryExpr` tree.
- `ViewDocument` composes chart slots and view objects; `ViewInstance` binds workspace chart instances to those slots.

`ChartSnapshot`, `ChartAnalysis`, layouts, and scenes are derived engine products. They are never the only representation of a chart.

## Reproducibility rules

Chart definitions store the zodiac, house system, coordinate system, node choice, Black Moon choice, fortune formula, and correction choices that affect their meaning. Defaults only seed a new definition. A later default change cannot mutate an existing definition.

Reusable settings have explicit binding semantics:

- `Follow { id }` resolves the current revision.
- `Pinned { id, revision }` resolves exactly that revision.
- `Inline(value)` carries a self-contained value.

Displayed points and aspected points are separate bindings, so visible points do not have to participate in aspect analysis.

## Time and location

`TemporalAssertion` retains civil date/time, calendar, timezone assertion, and optional ambiguity choice. `ResolvedTime` is a derived calculation value and includes the applied offset and timezone-data version.

`LocationAssertion` retains the exact latitude/longitude used plus optional atlas provenance. An atlas display name is never a substitute for coordinates.

The milestone validates civil fields and coordinate bounds but deliberately does not implement historical timezone/calendar resolution.

# Domain model

## Authoritative nouns

- `ChartRecord` preserves the asserted civil time, timezone form, coordinates, provenance, and notes.
- `ChartDefinition` references a radix record or records a derivation recipe and stores concrete calculation choices.
- `ResourceEnvelope<T>` supplies stable identity, schema version, revision, metadata, and a typed payload.
- `WorkspaceDocument` is durable intentional composition: saved chart membership/order,
  `ViewInstance` values and slot assignments, workspace bindings, and saved display overrides. It
  references only saved `ChartDefinition` resources.
- `ResourceBinding<T>` distinguishes following a resource, pinning a revision, and embedding a value.
- `QueryDefinition` stores a reusable boolean `QueryExpr` tree.
- `ViewDocument` composes chart slots and view objects; `ViewInstance` binds workspace chart instances to those slots.

`ChartSnapshot`, `ChartAnalysis`, layouts, and scenes are derived engine products. They are never the only representation of a chart.

`WorkspaceSession` is application state above this canonical model. It owns active/selected charts,
the active view, temporary overrides, and eventually draft charts. Its working document projection
can become dirty without writing a canonical revision.

Every canonical payload implements `DomainValidate`. Validation failures carry a structured issue
and a path such as `life_events[0].time.civil_datetime.date`. Envelopes additionally require schema
v1, a nonempty title, nonempty unique tags, and `modified_at >= created_at`. Repository create/save,
portable serialization/import, and IndexedDB decoding all invoke the same validation boundary.

This is structural validation: one object can validate without repository or catalog access.
Duplicate workspace instance IDs, duplicate view slot IDs, malformed inline values, and references
between fields inside one `ViewDocument` belong here. Referential validation belongs to
`mirabile-app`, where the hydrated catalog and `WorkspaceSession` are available. Missing Follow
heads, missing Pinned revisions, absent chart definitions/records, session identities, and slot
assignments against a resolved external `ViewDocument` are application graph errors.

## Reproducibility rules

Chart definitions store the zodiac, house system, coordinate system, node choice, Black Moon choice, fortune formula, and correction choices that affect their meaning. Defaults only seed a new definition. A later default change cannot mutate an existing definition.

Reusable settings have explicit binding semantics:

- `Follow { id }` resolves the current revision.
- `Pinned { id, revision }` resolves exactly that revision.
- `Inline(value)` carries a self-contained value.

Displayed points and aspected points are separate bindings, so visible points do not have to participate in aspect analysis.

Resolved settings retain independent provenance dimensions: `ConfigurationLayer` describes the
precedence layer that won, and `ValueSource` describes whether the material value was Inline,
Follow, or Pinned (including the actual resource revision). This lets an inspector answer both
"why did this win?" and "where did it come from?"

## Time and location

`TemporalAssertion` retains civil date/time, calendar, timezone assertion, and optional ambiguity choice. `ResolvedTime` is a derived calculation value and includes an explicit astronomical time-scale label, the applied offset, and timezone-data version. The current resolver produces a UTC-clock Julian day; backends must perform and record any TT or UT1 conversion instead of relabelling it. Years use astronomical numbering: year 0 is 1 BCE, year -1 is 2 BCE.

`CivilDate` construction enforces calendar-independent structure, so February 29 is representable.
`TemporalAssertion` applies proleptic-Gregorian century rules or the Julian every-four-years rule.
Historical-transition calendars receive structural validation only and remain unsupported by the
calculation engine.

`LocationAssertion` retains the exact latitude/longitude used plus optional atlas provenance. An atlas display name is never a substitute for coordinates.

Named historical timezone resolution, local apparent time, and historical calendar-transition
calculation remain deliberately unimplemented and fail explicitly.

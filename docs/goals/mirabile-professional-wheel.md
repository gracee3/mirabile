# Mirabile Professional Wheel

## Goal state

- Base: `8e7b610fb6afd8d72510cb1a0431d946a8c912d8`, the verified squash merge of
  control-cockpit PR #2 on `main`.
- Branch: `goal/mirabile-professional-wheel`.
- Current phase: complete and squash-merged through PR #3.
- Delivery: final feature head `3763bfd1d3a7648b5ef672ebbfba5f2b785539cd` was squash-merged as
  `1341b95f39046cb340670b5a735bdc4a475d9220` without deleting the feature branch.

## Rendering contract

- Screen angle is `normalize(longitude + rotation - 90 degrees)`, with clockwise-increasing
  longitude. With an actual Ascendant, rotation is `270 degrees - ASC`; without actual angles,
  rotation is zero.
- Zodiac boundaries and identities are independent from real calculated house cusps. House numbers
  use directed cusp-arc midpoints, preserving Equal, Placidus, and NoHouses distinctions.
- ASC and MC are actual calculated angles. DSC and IC are derived opposites only when their actual
  bases exist and the Scene identifies them as derived.
- Point anchors remain at true longitudes. Deterministic circular label placement may move labels
  through bounded radial lanes and leader lines, but never changes semantic longitude/latitude or
  retrograde state.
- Every calculated aspect remains a semantic Scene entry, including conjunctions without chords;
  no missing aspect is synthesized.
- Scene and layout additions remain provider-neutral and serde-compatible. Automation exposes only
  a bounded semantic manifest, never full Scene geometry or mutation access.

## Presentation contract

- The existing dark palette remains the base: charcoal/navy surfaces, warm ivory text,
  antique-gold accents, and restrained dusty-rose/cool-blue aspect distinctions.
- Unicode zodiac and supported point glyphs have full accessible names and system-font fallbacks;
  unknown point/aspect IDs remain readable text with neutral styling.
- The active wheel is the first-screen primary surface. Cockpit/builders and developer diagnostics
  remain mounted behind native keyboard-accessible disclosures, and responsive inspectors cannot
  crush or overlap the scalable semantic SVG.
- Stable SVG groups and data attributes identify zodiac signs, houses, angles, true anchors,
  labels, retrograde markers, leaders, and aspect IDs.

## Verification matrix

- Native geometry tests: orientation/normalization; Equal, Placidus, and NoHouses; actual/derived
  angles; degree-minute carry; Unicode/fallback metadata; retrograde truth; aspect identity and
  conjunction retention; dense, equal, seam-crossing, and reversed point inputs; compact/regular
  bounds; and repeatability.
- Browser journey: normal demo with fresh XALEN output; twelve signs; real houses/angles; point
  anchors/labels; semantic aspects; accessibility; diagnostics access; retrograde assertion from a
  real demo chart; house-system controls; and zero console errors.
- Responsive acceptance: 1600x1000, 1366x768, T14 1920x1080, and below 850 px, with DOM bounding
  boxes plus ignored screenshots.
- Required gates: focused engine/web/XALEN tests, relevant browser journeys, `scripts/check.sh`, a
  disk-gated `scripts/verify.sh`, clean remote merge state, and clean synchronized `main` after the
  authorized squash merge.

## Frozen boundaries

- No provider expansion or XALEN changes, new font or asset dependency, schema redesign, bi-wheel,
  interpretation, atlas/timezone sprint, unrelated refactor, additional checkout, worktree, or
  Cargo target.
- Generic Scene primitives remain only for compatibility; newly consumed semantic/display inputs
  participate in layout/render keys with a new algorithm revision.
- The existing `proc-macro-error2` future-compatibility output remains a warning unless behavior
  changes.

## Progress

- 2026-08-29: branch created from the exact verified PR #2 merge in commit `9c29df2` and draft PR
  #3 opened immediately.
- 2026-08-29: code milestone `01ffc98` added the provider-neutral semantic Scene and wheel layout,
  actual houses/angles, semantic aspects, deterministic label lanes/leaders, formatted positions,
  accessible SVG, bounded automation Scene manifest, view rotation plumbing, and wheel-first native
  disclosure composition. The existing XALEN adapter and calculation boundary were unchanged.
- 2026-08-29: browser milestone `26d4177` added fixed read-only DOM geometry assertions and the
  37-step professional-wheel journey. The journey uses real Worker/XALEN output for Example Natal,
  exercises NoHouses, Equal, and Placidus through typed chart controls, and switches the wheel to
  Example Event to assert a real retrograde Jupiter and two retained conjunction entries without
  chords.
- Focused native evidence: 19 engine architecture/geometry tests and three automation manifest
  tests pass; strict Clippy passes. The fast gate passes with 189 workspace tests, seven CDP-client
  tests, formatting, strict Clippy, and clean diffs.
- The first full-gate run exposed that CDP keyboard focus could not reach a buffered control inside
  a closed native disclosure. The constrained driver now reveals enclosing disclosures before
  keyboard focus (without toggling a summary's own disclosure); its regression test, the complete
  92-step new-chart control journey, and the professional-wheel journey all pass.
- Responsive browser evidence: the professional journey passes at 1366x768, 1600x1000, the T14's
  current 1920x1080 display mode, and 800x900. Each run reports twelve zodiac signs, twelve real
  houses for the final Equal chart, four angles, six true anchors and labels, one actual retrograde
  marker, two semantic conjunctions, zero label overlaps, no clipping or horizontal overflow, a
  centered first-surface wheel, accessible title/description, and zero console errors.
- Final disk gate: after the measured repository-local Cargo cleanup, 29,903,974,400 bytes were
  available, above the 17 GiB reserve (`15 GiB + max(2 GiB, observed focused-build growth)`).
- Exact-head `scripts/verify.sh` passed in 664.90 seconds. The focused XALEN adapter suite passed
  8/8 tests, the professional geometry suite passed 19/19, the automation manifest suite passed
  3/3, and the exact-head fast gate passed 189 Rust plus seven constrained-driver tests.
- A normal production session with no automation bridge loaded Example Natal, exercised NoHouses
  and Equal through native controls, reopened cockpit and diagnostics disclosures, and confirmed
  fresh Worker/XALEN output, accessible SVG metadata, no horizontal overflow, and zero browser
  errors.
- Ignored responsive captures were inspected at 1600x1000, 1366x768, the T14's 1920x1080 mode,
  and 800x900. Their SHA-256 values and bounded DOM evidence are recorded on PR #3.

## Merge closure

- PR #3's exact pushed feature head was
  `3763bfd1d3a7648b5ef672ebbfba5f2b785539cd`; remote state was `MERGEABLE` and `CLEAN` with no
  blocking review or status check before promotion.
- PR #3 was marked ready and squash-merged without branch deletion or administrative bypass on
  2026-08-29. The verified merge commit is `1341b95f39046cb340670b5a735bdc4a475d9220`;
  the remote feature branch remains preserved at the exact feature head.
- Local `main` was fast-forwarded to the verified remote merge and the post-merge
  `scripts/check.sh` gate passed in 13.88 seconds. This documentation-only closure synchronizes the
  committed goal records with that already verified delivery state.

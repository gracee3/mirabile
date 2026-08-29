# Mirabile Professional Wheel

## Goal state

- Base: `8e7b610fb6afd8d72510cb1a0431d946a8c912d8`, the verified squash merge of
  control-cockpit PR #2 on `main`.
- Branch: `goal/mirabile-professional-wheel`.
- Current phase: architecture and geometry inventory.
- Delivery: preserve the provider-neutral calculation boundary, extend semantic layout and Scene
  data additively, make the wheel the responsive primary surface, verify the exact pushed head, and
  squash-merge the feature PR without deleting the branch.

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

- 2026-08-29: branch created from the exact verified PR #2 merge. No implementation change has yet
  been made.

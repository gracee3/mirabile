# XALEN calculation backend

Status: implementation handoff audit, pinned for the first offline analytical radix slice.

## Upstream and dependency policy

- Repository: `https://github.com/vedika-io/xalen-ephemeris`
- Revision: `cc6edbec1f748ebdc4950ae6198f575c5ada73fa`
- XALEN workspace/package version: `0.6.0`
- Workspace license: Apache-2.0, except that `xalen-coords` also carries BSD-3-Clause
  material identified in XALEN's `NOTICE`.
- The `xalen-ephemeris` umbrella crate is not used.
- Direct leaf crates: `xalen-ephem`, `xalen-time`, `xalen-coords`, and
  `xalen-houses`, all from the same exact Git revision.
- `xalen-ephem` is always selected with `default-features = false`.
- `hip-catalog` and `kernel-autodownload` are not enabled. The separately licensed
  `xalen-stars-hip-data` crate must be absent from Astra's resolved graph.
- No DE440 kernel, Swiss dependency/data, network acquisition, cloud service,
  interpretation layer, or XALEN Western/Vedic/chart layer participates.

## Semantics mapping audit

| Astra semantic | Pinned XALEN API | Exact? | Notes and limitations |
| --- | --- | --- | --- |
| `AstroInstant` produced by `resolve_time` | `xalen_time::JdUTC` | Yes with the provider-neutral `TimeScale::Utc` label | Astra subtracts the asserted civil offset and therefore computes a UTC-clock Julian date. It is not UT1 or TT. |
| Celestial ephemeris time | `JdUTC::to_tt()` then `Almanac::geocentric_*_tt` | Yes for ordinary post-1972 UTC instants | XALEN applies its embedded IERS leap-second table for UTC to TAI, then the defined TT-TAI offset of 32.184 seconds. The backend passes TT, not relabelled UTC/UT1, to the low-level analytical provider. Pre-1972 UTC is rejected because rubber seconds are not modeled; a civil `23:59:60` label is outside Astra's current civil-time schema. |
| House rotation time | `JdTT::to_ut1(&StephensonMorrisonHohenkerk2016)` | Model-derived | House computation receives UT1 derived iteratively from the same TT using XALEN's SMH2016 Delta-T implementation. The time-conversion implementation and model identities are retained in the pre-execution backend fingerprint, `CalcKey`, and result provenance. |
| Geocentric coordinates | `Almanac::geocentric_ecliptic_tt` | Yes | Apparent geocentric ecliptic-of-date longitude/latitude. |
| Right ascension and declination | `Almanac::geocentric_equatorial_tt` | Yes | Apparent ecliptic place rotated by XALEN with the true obliquity of date. |
| Longitude speed | `Almanac::geocentric_speed_tt` | Yes within XALEN's definition | Central finite difference over plus/minus 0.5 TT day, returned in degrees per day. |
| Retrograde | `EclipticSpeed::is_retrograde` | Yes | True exactly when apparent longitude speed is negative. |
| Requested bodies | private `PointId` to `xalen_ephem::Body` match | Exact for advertised IDs | This slice maps `sun`, `moon`, `mercury`, `venus`, `mars`, and `jupiter` only. Unknown/unadvertised IDs fail with `UnsupportedCapability`. |
| Tropical zodiac | apparent ecliptic-of-date result unchanged | Yes | Supported. |
| Sidereal/ayanamsa | not called | No in this slice | Typed unsupported. No XALEN ayanamsa enum crosses the adapter boundary. |
| Aberration/light-time/nutation | XALEN analytical apparent-place pipeline | Exact only for `{aberration: true, light_time: true, nutation: true}` | XALEN's selected API always returns its defined apparent place: IAU 2000B nutation plus body-appropriate aberration/light-time. Other flag combinations are rejected; Astra does not recreate or subtract corrections. |
| Equal houses | `xalen_houses::compute_houses` with `HouseSystem::Equal` | Yes to the documented XALEN mean-frame semantics | Uses model-derived UT1, XALEN mean sidereal time, and IAU 2006 mean obliquity. |
| Placidus houses | `xalen_houses::compute_houses` with `HouseSystem::Placidus` | Yes within the non-polar domain | Requests above XALEN's documented 66.5 degree polar limit are rejected instead of silently accepting its Porphyry fallback. |
| Ascendant | `HouseCusps::ascendant` | Yes | Converted locally from radians to Astra `Angle`. |
| Midheaven | `HouseCusps::mc` | Yes | Converted locally from radians to Astra `Angle`. |
| Lunar node choice | no point mapping | No in this slice | The request configuration is retained in provenance, but no node point is advertised or returned. |
| Black Moon choice | no point mapping | No in this slice | The request configuration is retained in provenance, but no apogee point is advertised or returned. |
| Derived formulas | not called | No | `derived = None`; XALEN Western/Vedic layers are intentionally absent. |

## Analytical model identity

The backend uses XALEN's data-file-free analytical apparent-place path. For the
advertised bodies this is VSOP87A for the Sun/planets and XALEN's Meeus chapter 47,
ELP2000-82-derived truncated lunar series for the Moon, followed by XALEN's IAU
2006/P03 precession, IAU 2000B nutation, and body-appropriate light-time/aberration
pipeline. It is not DE440. Reference validation against DE440 does not change the
calculation model's identity.

## Known-answer policy

Adapter mapping tests may compare the adapter with direct calls into the pinned
XALEN API. Accuracy tests instead pin independent numeric references:

- apparent geocentric ecliptic-of-date longitudes from NASA/JPL Horizons DE440
  quantity 31, as committed in XALEN's `swiss_eph_crossval.rs` at the pinned
  revision;
- house cusp, Ascendant, and Midheaven values generated by pyswisseph 2.10.03
  `houses_armc` using the same XALEN RAMC and IAU 2006 mean obliquity, as committed
  in XALEN's `swiss_houses_oracle.rs` at the pinned revision.

The Astra fixture records the source, epoch, location, frame, values, tolerances,
and XALEN revision next to the assertions. Angular comparisons are wrap-aware.

The complete fixture is `2000-01-01 12:00:00 UTC`, latitude `28.0 N`, longitude
`73.85 E`, tropical geocentric apparent place, and Placidus houses. The expected
longitudes in degrees are Sun `280.3689` (tolerance `0.001`), Moon `223.3238`
(`0.006`), Mercury `271.8893` (`0.001`), Venus `241.5658` (`0.001`), Mars
`327.9633` (`0.001`), and Jupiter `25.2531` (`0.001`). The expected cusps are
`96.907359`, `119.672032`, `144.432212`, `173.802738`, `208.270696`,
`244.138944`, `276.907359`, `299.672032`, `324.432212`, `353.802738`,
`28.270696`, and `64.138944`, each with a `0.01` degree tolerance. Ascendant is
`96.907359` and Midheaven is `353.802738`, also with `0.01` degree tolerances.

## Runtime boundary

The feature-gated implementation lives inside `astra-engine`. Its private XALEN
types are converted to Astra-owned request/result types before returning. Normal
browser wiring constructs the XALEN descriptor on the UI thread and the XALEN
backend inside the calculation Worker; the worker protocol, canonical resources,
`CalculationValue`, analysis, layout, and frontend remain XALEN-free.
Native construction uses `RealApplication::with_xalen_backend`, which selects the
same apparent-place bootstrap profile as the browser constructor.

The static time fingerprint records `xalen-time`, UTC input, TT celestial time,
UT1 house time, the embedded IERS 1972-2017 leap-second table, and SMH2016
Delta-T before execution. All of this material participates in `CalcKey`.
Completion rejects time provenance that differs from the selected fingerprint.

## Distribution notices

The browser distribution includes `THIRD_PARTY_NOTICES.md`, the complete XALEN
Apache-2.0 license, the ERFA BSD-3-Clause notice, and the `vsop87` MIT license.
Trunk copies these files into every distribution and the browser contract checks
their presence and exact content. The notice explicitly states that Astra does
not bundle XALEN's optional Hipparcos catalog or NC data crate.

`scripts/check-xalen-dependencies.sh` audits the actual `astra-engine` and
`astra-web` feature trees. XALEN's own `xalen-ephem` manifest necessarily brings
`xalen-ayanamsa`, `xalen-star-anchors`, and `xalen-stars`; `xalen-stars` is built
with defaults disabled and the non-commercial `xalen-stars-hip-data` package is
absent.

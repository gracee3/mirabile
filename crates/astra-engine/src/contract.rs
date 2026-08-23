use std::collections::{BTreeMap, BTreeSet};

use astra_core::{
    Angle, AngleState, BlackMoonType, CoordinateSystem, CorrectionSpec, FortuneFormula,
    HouseSystem, LunarNodeType, PointId, PointState, ResolvedTime,
};
use serde::{Deserialize, Serialize};

/// Resolved factual inputs shared by every calculation responsibility.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalculationContext {
    pub time: ResolvedTime,
    pub location: NumericLocation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NumericLocation {
    pub latitude: astra_core::Latitude,
    pub longitude: astra_core::Longitude,
}

/// Provider-neutral zodiac semantics after Astra resolves `CalculationSpec`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ZodiacCalculationRequest {
    Tropical,
    Sidereal { ayanamsa: AyanamsaConfiguration },
}

/// Astra-owned ayanamsa identity and optional deterministic configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AyanamsaConfiguration {
    pub id: String,
    pub parameters: BTreeMap<String, String>,
}

/// Astronomical/celestial work requested from a backend.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CelestialPositionsRequest {
    pub requested_points: Vec<PointId>,
    pub coordinates: CoordinateSystem,
    pub corrections: CorrectionSpec,
    pub lunar_node: LunarNodeType,
    pub black_moon: BlackMoonType,
}

/// House cusps and chart angles are deliberately separate from body positions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseCalculationRequest {
    pub system: HouseSystem,
    pub zodiac: ZodiacCalculationRequest,
}

/// Astrology-specific formulas requested independently of astronomical work.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedPointsRequest {
    pub points: Vec<DerivedPointRequest>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedPointRequest {
    pub point: PointId,
    pub formula: DerivedFormula,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivedFormula {
    PartOfFortune {
        formula: FortuneFormula,
    },
    Named {
        id: String,
        parameters: BTreeMap<String, String>,
    },
}

/// The complete provider-neutral execution request. It contains no canonical resources.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResolvedCalculationRequest {
    pub context: CalculationContext,
    pub zodiac: ZodiacCalculationRequest,
    pub celestial: CelestialPositionsRequest,
    pub houses: Option<HouseCalculationRequest>,
    pub derived: DerivedPointsRequest,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CelestialPositionsResult {
    pub positions: BTreeMap<PointId, PointState>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HouseCalculationResult {
    pub cusps: Vec<Angle>,
    pub angles: AngleState,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DerivedPointsResult {
    pub positions: BTreeMap<PointId, PointState>,
}

/// Provider-neutral identity for linked, external, or remotely hosted implementations.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ImplementationIdentity {
    pub id: String,
    pub version: String,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EphemerisModelKind {
    Analytic,
    NumericalEphemeris,
    Tables,
    Other,
}

/// Identity for the model/data actually underlying celestial calculation.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EphemerisModelIdentity {
    pub kind: EphemerisModelKind,
    pub id: String,
    pub version: Option<String>,
    pub revision: Option<String>,
    pub data_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CelestialBackendFingerprint {
    pub implementation: ImplementationIdentity,
    pub model: Option<EphemerisModelIdentity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseBackendFingerprint {
    pub implementation: ImplementationIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedBackendFingerprint {
    pub implementation: ImplementationIdentity,
}

/// Deterministic pre-execution identity used in `CalcKey`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendFingerprint {
    pub backend: ImplementationIdentity,
    pub celestial: Option<CelestialBackendFingerprint>,
    pub houses: Option<HouseBackendFingerprint>,
    pub derived: Option<DerivedBackendFingerprint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CelestialCapabilities {
    pub supported_points: BTreeSet<PointId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseCapabilities {
    pub supported_systems: Vec<HouseSystem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedCapabilities {
    pub supported_formula_ids: BTreeSet<String>,
}

/// One backend descriptor may advertise any combination of responsibilities.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendCapabilities {
    pub celestial: Option<CelestialCapabilities>,
    pub houses: Option<HouseCapabilities>,
    pub derived: Option<DerivedCapabilities>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendDescriptor {
    pub fingerprint: BackendFingerprint,
    pub capabilities: BackendCapabilities,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AstraCalculationProvenance {
    pub calculation_engine: ImplementationIdentity,
    pub timezone_data_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CelestialCalculationProvenance {
    pub implementation: ImplementationIdentity,
    pub model: Option<EphemerisModelIdentity>,
    pub coordinates: CoordinateSystem,
    pub corrections: CorrectionSpec,
    pub zodiac: ZodiacCalculationRequest,
    pub lunar_node: LunarNodeType,
    pub black_moon: BlackMoonType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseCalculationProvenance {
    pub implementation: ImplementationIdentity,
    pub system: HouseSystem,
    pub zodiac: ZodiacCalculationRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedFormulaProvenance {
    pub point: PointId,
    pub formula: DerivedFormula,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DerivedCalculationProvenance {
    pub implementation: ImplementationIdentity,
    pub formulas: Vec<DerivedFormulaProvenance>,
}

/// Provenance supplied by the backend for the work it actually performed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendCalculationProvenance {
    pub backend: ImplementationIdentity,
    pub celestial: CelestialCalculationProvenance,
    pub houses: Option<HouseCalculationProvenance>,
    pub derived: Option<DerivedCalculationProvenance>,
}

/// Full reproducibility material retained with each calculated value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalculationProvenance {
    pub astra: AstraCalculationProvenance,
    pub backend: ImplementationIdentity,
    pub celestial: CelestialCalculationProvenance,
    pub houses: Option<HouseCalculationProvenance>,
    pub derived: Option<DerivedCalculationProvenance>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalculationBackendResult {
    pub celestial: CelestialPositionsResult,
    pub houses: Option<HouseCalculationResult>,
    pub derived: DerivedPointsResult,
    pub provenance: BackendCalculationProvenance,
}

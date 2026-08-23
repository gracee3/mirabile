use serde::{Deserialize, Serialize};

use crate::{
    Angle, AngularVelocity, AstroInstant, Latitude, Longitude, PointId, ResourceId,
    TemporalAssertion, Timestamp,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Birth,
    Event,
    Ingress,
    Question,
    Other(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubjectInfo {
    pub display_name: String,
    pub pronouns: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AtlasRef {
    pub provider: String,
    pub record_id: Option<String>,
    pub data_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocationAssertion {
    pub display_name: String,
    pub country_region: Option<String>,
    pub latitude: Latitude,
    pub longitude: Longitude,
    pub atlas_provenance: Option<AtlasRef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResolvedLocation {
    pub display_name: String,
    pub latitude: Latitude,
    pub longitude: Longitude,
    pub role: LocationRole,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationRole {
    Asserted,
    Relocated,
    Derived,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceProvenance {
    pub description: String,
    pub source_type: SourceType,
    pub recorded_by: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    BirthCertificate,
    Memory,
    Published,
    Research,
    UserAssertion,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Note {
    pub text: String,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LifeEvent {
    pub title: String,
    pub time: TemporalAssertion,
    pub location: Option<LocationAssertion>,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartRecord {
    pub event_kind: EventKind,
    pub subject: Option<SubjectInfo>,
    pub time: TemporalAssertion,
    pub location: LocationAssertion,
    pub source: SourceProvenance,
    pub notes: Vec<Note>,
    pub life_events: Vec<LifeEvent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartDefinition {
    pub source: ChartSource,
    pub calculation: CalculationSpec,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChartSource {
    Radix { record: ResourceId },
    Derived { recipe: DerivationSpec },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DerivationSpec {
    Transit {
        at: TemporalAssertion,
        location: LocationAssertion,
    },
    Harmonic {
        radix: ResourceId,
        harmonic: f64,
    },
    Relocation {
        radix: ResourceId,
        location: LocationAssertion,
    },
    Composite {
        charts: Vec<ResourceId>,
        method: CompositeMethod,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositeMethod {
    Midpoint,
    Davison,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalculationSpec {
    pub zodiac: ZodiacSpec,
    pub houses: HouseSystem,
    pub coordinates: CoordinateSystem,
    pub lunar_node: LunarNodeType,
    pub black_moon: BlackMoonType,
    pub fortune_formula: FortuneFormula,
    pub corrections: CorrectionSpec,
}

impl Default for CalculationSpec {
    fn default() -> Self {
        Self {
            zodiac: ZodiacSpec::Tropical,
            houses: HouseSystem::Placidus,
            coordinates: CoordinateSystem::Geocentric,
            lunar_node: LunarNodeType::True,
            black_moon: BlackMoonType::Mean,
            fortune_formula: FortuneFormula::DayNight,
            corrections: CorrectionSpec::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ZodiacSpec {
    Tropical,
    Sidereal { ayanamsha: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HouseSystem {
    Placidus,
    WholeSign,
    Equal,
    NoHouses,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSystem {
    Geocentric,
    Topocentric,
    Heliocentric,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LunarNodeType {
    Mean,
    True,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlackMoonType {
    Mean,
    Osculating,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FortuneFormula {
    DayNight,
    AlwaysAscendantPlusMoonMinusSun,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrectionSpec {
    pub aberration: bool,
    pub light_time: bool,
    pub nutation: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointState {
    pub longitude: Angle,
    pub latitude: Angle,
    pub declination: Angle,
    pub right_ascension: Angle,
    pub speed_longitude: AngularVelocity,
    pub retrograde: bool,
    pub azimuth: Option<Angle>,
    pub altitude: Option<Angle>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HouseState {
    pub cusps: Vec<Angle>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AngleState {
    pub ascendant: Option<Angle>,
    pub midheaven: Option<Angle>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointPosition {
    pub point: PointId,
    pub instant: AstroInstant,
}

use std::fmt;

use astra_core::{
    AnalysisProfile, Angle, AspectId, AspectSet, CalculationSpec, ChartDefinition, ChartRecord,
    DomainValidate, DomainValidationError, Latitude, Longitude, PointId, PointSelector, PointSet,
    ResourceEnvelope, ResourceError, TemporalAssertion, Theme,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ProviderIdentity;

macro_rules! content_key {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

content_key!(CalcKey);
content_key!(AnalysisKey);
content_key!(LayoutKey);
content_key!(RenderKey);

#[derive(Serialize)]
struct CalcKeyInput<'a> {
    temporal_assertion: &'a TemporalAssertion,
    numeric_location: NumericLocationMaterial,
    calculation: &'a CalculationSpec,
    engine_version: &'a str,
    provider: &'a ProviderIdentity,
    timezone_data_version: &'a str,
}

#[derive(Serialize)]
struct NumericLocationMaterial {
    latitude: Latitude,
    longitude: Longitude,
}

impl CalcKey {
    pub fn derive(
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
        engine_version: &str,
        provider: &ProviderIdentity,
        timezone_data_version: &str,
    ) -> Result<Self, KeyError> {
        definition.validate()?;
        record.validate()?;
        hash(
            "calc",
            &CalcKeyInput {
                temporal_assertion: &record.payload.time,
                numeric_location: NumericLocationMaterial {
                    latitude: record.payload.location.latitude,
                    longitude: record.payload.location.longitude,
                },
                calculation: &definition.payload.calculation,
                engine_version,
                provider,
                timezone_data_version,
            },
        )
        .map(Self)
    }
}

#[derive(Serialize)]
struct AnalysisKeyInput {
    calc_keys: Vec<CalcKey>,
    resolved_points: Vec<PointId>,
    enabled_aspects: Vec<AspectRuleMaterial>,
    profile: AnalysisProfileMaterial,
    analyzer_version: &'static str,
}

#[derive(Serialize)]
struct AspectRuleMaterial {
    id: AspectId,
    angle: Angle,
    maximum_orb: Angle,
    applying_multiplier: f64,
}

#[derive(Serialize)]
struct AnalysisProfileMaterial {
    include_applying_state: bool,
    maximum_hits: Option<u32>,
}

impl AnalysisKey {
    pub fn derive(
        calc_keys: &[CalcKey],
        points: &PointSet,
        aspects: &AspectSet,
        profile: &AnalysisProfile,
    ) -> Result<Self, KeyError> {
        points.domain_validate()?;
        aspects.domain_validate()?;
        profile.domain_validate()?;
        let mut resolved_points = Vec::with_capacity(points.points.len());
        for selector in &points.points {
            match selector {
                PointSelector::Point(point) => resolved_points.push(point.clone()),
                PointSelector::Category(category) => {
                    return Err(KeyError::UnresolvedPointCategory(category.clone()));
                }
            }
        }
        resolved_points.sort();
        let mut enabled_aspects = aspects
            .aspects
            .iter()
            .filter(|aspect| aspect.enabled)
            .map(|aspect| AspectRuleMaterial {
                id: aspect.id.clone(),
                angle: aspect.angle,
                maximum_orb: aspect.orbs.maximum,
                applying_multiplier: aspect.orbs.applying_multiplier,
            })
            .collect::<Vec<_>>();
        enabled_aspects.sort_by(|lhs, rhs| {
            lhs.id
                .cmp(&rhs.id)
                .then_with(|| lhs.angle.degrees().total_cmp(&rhs.angle.degrees()))
                .then_with(|| {
                    lhs.maximum_orb
                        .degrees()
                        .total_cmp(&rhs.maximum_orb.degrees())
                })
                .then_with(|| lhs.applying_multiplier.total_cmp(&rhs.applying_multiplier))
        });
        let mut sorted_calc_keys = calc_keys.to_vec();
        sorted_calc_keys.sort();
        hash(
            "analysis",
            &AnalysisKeyInput {
                calc_keys: sorted_calc_keys,
                resolved_points,
                enabled_aspects,
                profile: AnalysisProfileMaterial {
                    include_applying_state: profile.include_applying_state,
                    maximum_hits: profile.maximum_hits,
                },
                analyzer_version: "aspect-analyzer-v1",
            },
        )
        .map(Self)
    }
}

#[derive(Serialize)]
struct LayoutKeyInput<'a> {
    points: &'a [(PointId, Angle)],
    aspect_pairs: &'a [(PointId, PointId)],
    zodiac_radius: f64,
    aspect_radius: f64,
    layout_version: &'a str,
}

impl LayoutKey {
    pub fn derive(
        points: &[(PointId, Angle)],
        aspect_pairs: &[(PointId, PointId)],
        zodiac_radius: f64,
        aspect_radius: f64,
        layout_version: &str,
    ) -> Result<Self, KeyError> {
        hash(
            "layout",
            &LayoutKeyInput {
                points,
                aspect_pairs,
                zodiac_radius,
                aspect_radius,
                layout_version,
            },
        )
        .map(Self)
    }
}

#[derive(Serialize)]
struct RenderKeyInput<'a> {
    layout_key: &'a LayoutKey,
    theme: &'a Theme,
    renderer_version: &'a str,
}

impl RenderKey {
    pub fn derive(
        layout_key: &LayoutKey,
        theme: &Theme,
        renderer_version: &str,
    ) -> Result<Self, KeyError> {
        hash(
            "render",
            &RenderKeyInput {
                layout_key,
                theme,
                renderer_version,
            },
        )
        .map(Self)
    }
}

fn hash<T: Serialize>(namespace: &str, value: &T) -> Result<String, KeyError> {
    let encoded = serde_json::to_vec(value)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(namespace.as_bytes());
    hasher.update(&[0]);
    hasher.update(&encoded);
    Ok(hasher.finalize().to_hex().to_string())
}

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("could not serialize computation-key input: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    InvalidDomain(#[from] DomainValidationError),
    #[error(transparent)]
    InvalidResource(#[from] ResourceError),
    #[error("point category {0:?} must be resolved before computation")]
    UnresolvedPointCategory(String),
}

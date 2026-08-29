use std::fmt;

use mirabile_core::{
    AnalysisProfile, Angle, AspectClass, AspectId, AspectSet, DomainValidate,
    DomainValidationError, PointId, PointSelector, PointSet, ResourceError, Theme,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{BackendFingerprint, ImplementationIdentity, ResolvedCalculationRequest};

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
    request: &'a ResolvedCalculationRequest,
    calculation_engine: &'a ImplementationIdentity,
    backend: &'a BackendFingerprint,
}

impl CalcKey {
    pub fn derive(
        request: &ResolvedCalculationRequest,
        calculation_engine: &ImplementationIdentity,
        backend: &BackendFingerprint,
    ) -> Result<Self, KeyError> {
        hash(
            "calc",
            &CalcKeyInput {
                request,
                calculation_engine,
                backend,
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
    name: String,
    classification: AspectClass,
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
                name: aspect.name.clone(),
                classification: aspect.classification,
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
struct LayoutKeyInput<'a, T> {
    material: &'a T,
    layout_version: &'a str,
}

impl LayoutKey {
    pub fn derive<T: Serialize>(material: &T, layout_version: &str) -> Result<Self, KeyError> {
        hash(
            "layout",
            &LayoutKeyInput {
                material,
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

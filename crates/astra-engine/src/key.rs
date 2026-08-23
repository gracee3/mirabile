use std::fmt;

use astra_core::{
    AnalysisProfile, AspectSet, ChartDefinition, ChartRecord, PointSet, ResourceEnvelope, Theme,
    ViewDocument, WheelTemplate,
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
    definition: &'a ChartDefinition,
    record: &'a ChartRecord,
    engine_version: &'a str,
    provider: &'a ProviderIdentity,
    timezone_data_version: &'a str,
}

impl CalcKey {
    pub fn derive(
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
        engine_version: &str,
        provider: &ProviderIdentity,
        timezone_data_version: &str,
    ) -> Result<Self, KeyError> {
        hash(
            "calc",
            &CalcKeyInput {
                definition: &definition.payload,
                record: &record.payload,
                engine_version,
                provider,
                timezone_data_version,
            },
        )
        .map(Self)
    }
}

#[derive(Serialize)]
struct AnalysisKeyInput<'a> {
    calc_keys: &'a [CalcKey],
    points: &'a PointSet,
    aspects: &'a AspectSet,
    profile: &'a AnalysisProfile,
}

impl AnalysisKey {
    pub fn derive(
        calc_keys: &[CalcKey],
        points: &PointSet,
        aspects: &AspectSet,
        profile: &AnalysisProfile,
    ) -> Result<Self, KeyError> {
        hash(
            "analysis",
            &AnalysisKeyInput {
                calc_keys,
                points,
                aspects,
                profile,
            },
        )
        .map(Self)
    }
}

#[derive(Serialize)]
struct LayoutKeyInput<'a> {
    analysis_key: &'a AnalysisKey,
    displayed_points: &'a PointSet,
    wheel: &'a WheelTemplate,
    view: Option<&'a ViewDocument>,
    layout_version: &'a str,
}

impl LayoutKey {
    pub fn derive(
        analysis_key: &AnalysisKey,
        displayed_points: &PointSet,
        wheel: &WheelTemplate,
        view: Option<&ViewDocument>,
        layout_version: &str,
    ) -> Result<Self, KeyError> {
        hash(
            "layout",
            &LayoutKeyInput {
                analysis_key,
                displayed_points,
                wheel,
                view,
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
}

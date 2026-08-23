use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AnalysisProfile, AspectSet, CalculationSpec, PointSet, ResourceBinding, ResourceEnvelope,
    ResourceId, Revision, Theme, WheelTemplate,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationLayer {
    BuiltIn,
    UserDefault,
    Workspace,
    ChartDefinition,
    View,
    Preview,
}

/// Material origin of an effective value, independent of why it won precedence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValueSource {
    Inline,
    Follow {
        resource_id: ResourceId,
        revision: Revision,
    },
    Pinned {
        resource_id: ResourceId,
        revision: Revision,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Resolved<T> {
    pub value: T,
    pub layer: ConfigurationLayer,
    pub source: ValueSource,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ConfigurationStack<T> {
    pub built_in: T,
    pub user_default: Option<T>,
    pub workspace: Option<T>,
    pub chart_definition: Option<T>,
    pub view_override: Option<T>,
    pub editor_preview: Option<T>,
}

impl<T> ConfigurationStack<T> {
    pub fn resolve(self) -> Resolved<T> {
        if let Some(value) = self.editor_preview {
            return Resolved::inline(value, ConfigurationLayer::Preview);
        }
        if let Some(value) = self.view_override {
            return Resolved::inline(value, ConfigurationLayer::View);
        }
        if let Some(value) = self.chart_definition {
            return Resolved::inline(value, ConfigurationLayer::ChartDefinition);
        }
        if let Some(value) = self.workspace {
            return Resolved::inline(value, ConfigurationLayer::Workspace);
        }
        if let Some(value) = self.user_default {
            return Resolved::inline(value, ConfigurationLayer::UserDefault);
        }
        Resolved::inline(self.built_in, ConfigurationLayer::BuiltIn)
    }
}

impl<T> Resolved<T> {
    fn inline(value: T, layer: ConfigurationLayer) -> Self {
        Self {
            value,
            layer,
            source: ValueSource::Inline,
        }
    }
}

pub fn resolve_binding<T: Clone>(
    binding: &ResourceBinding<T>,
    current: impl FnOnce(ResourceId) -> Option<ResourceEnvelope<T>>,
    historical: impl FnOnce(ResourceId, Revision) -> Option<ResourceEnvelope<T>>,
    layer: ConfigurationLayer,
) -> Result<Resolved<T>, BindingResolutionError> {
    match binding {
        ResourceBinding::Follow { id } => {
            let envelope = current(*id).ok_or(BindingResolutionError::MissingResource(*id))?;
            Ok(Resolved {
                value: envelope.payload,
                layer,
                source: ValueSource::Follow {
                    resource_id: envelope.id,
                    revision: envelope.revision,
                },
            })
        }
        ResourceBinding::Pinned { id, revision } => {
            let envelope =
                historical(*id, *revision).ok_or(BindingResolutionError::MissingRevision {
                    id: *id,
                    revision: *revision,
                })?;
            Ok(Resolved {
                value: envelope.payload,
                layer,
                source: ValueSource::Pinned {
                    resource_id: envelope.id,
                    revision: envelope.revision,
                },
            })
        }
        ResourceBinding::Inline { value } => Ok(Resolved {
            value: value.clone(),
            layer,
            source: ValueSource::Inline,
        }),
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EffectiveConfiguration {
    pub calculation: Resolved<CalculationSpec>,
    pub displayed_points: Resolved<PointSet>,
    pub aspected_points: Resolved<PointSet>,
    pub aspect_set: Resolved<AspectSet>,
    pub analysis: Resolved<AnalysisProfile>,
    pub wheel: Resolved<WheelTemplate>,
    pub theme: Resolved<Theme>,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum BindingResolutionError {
    #[error("bound resource {0} is missing")]
    MissingResource(ResourceId),
    #[error("bound resource {id} revision {revision} is missing")]
    MissingRevision { id: ResourceId, revision: Revision },
}

#[cfg(test)]
mod tests {
    use crate::{PointSet, Timestamp};

    use super::*;

    #[test]
    fn precedence_selects_preview_without_mutating_lower_layers() {
        let chart = CalculationSpec::default();
        let mut preview = chart.clone();
        preview.houses = crate::HouseSystem::WholeSign;
        let resolved = ConfigurationStack {
            built_in: CalculationSpec::default(),
            user_default: None,
            workspace: None,
            chart_definition: Some(chart.clone()),
            view_override: None,
            editor_preview: Some(preview),
        }
        .resolve();

        assert_eq!(resolved.layer, ConfigurationLayer::Preview);
        assert_eq!(resolved.source, ValueSource::Inline);
        assert_eq!(resolved.value.houses, crate::HouseSystem::WholeSign);
        assert_eq!(chart.houses, crate::HouseSystem::Placidus);
    }

    #[test]
    fn follow_and_pinned_bindings_resolve_different_revisions() {
        let id = ResourceId::new();
        let first = ResourceEnvelope::with_id(
            id,
            "Points",
            PointSet { points: Vec::new() },
            Timestamp::from_unix_millis(0),
        );
        let second = first
            .next_with_payload(
                PointSet {
                    points: vec![crate::PointSelector::Category("planets".into())],
                },
                Timestamp::from_unix_millis(1),
            )
            .expect("next revision");

        let followed = resolve_binding(
            &ResourceBinding::Follow { id },
            |_| Some(second.clone()),
            |_, _| None,
            ConfigurationLayer::Workspace,
        )
        .expect("follow current");
        let pinned = resolve_binding(
            &ResourceBinding::Pinned {
                id,
                revision: Revision::INITIAL,
            },
            |_| None,
            |_, _| Some(first),
            ConfigurationLayer::Workspace,
        )
        .expect("pin first");

        assert_eq!(followed.layer, ConfigurationLayer::Workspace);
        assert_eq!(pinned.layer, ConfigurationLayer::Workspace);
        assert_eq!(
            followed.source,
            ValueSource::Follow {
                resource_id: id,
                revision: second.revision,
            }
        );
        assert_eq!(
            pinned.source,
            ValueSource::Pinned {
                resource_id: id,
                revision: Revision::INITIAL,
            }
        );
        assert_ne!(followed.value, pinned.value);
    }
}

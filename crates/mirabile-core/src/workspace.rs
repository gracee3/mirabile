use serde::{Deserialize, Serialize};

use crate::{
    AnalysisProfile, AspectSet, DomainValidate, DomainValidationError, DomainValidationIssue,
    InstanceId, PanelId, PointSet, ResourceBinding, ResourceId, Theme, ViewInstance, WheelTemplate,
};

/// Durable, portable workspace composition.
///
/// Active/selected charts, the active view, drafts, and temporary overrides belong to an
/// application-owned workspace session instead of this canonical resource.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceDocument {
    pub chart_instances: Vec<WorkspaceDocumentChart>,
    pub views: Vec<ViewInstance>,
    pub profile: WorkspaceProfile,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceDocumentChart {
    pub instance_id: InstanceId,
    /// Stable identity of a saved canonical `ChartDefinition`.
    pub definition: ResourceId,
}

impl WorkspaceDocumentChart {
    pub const fn instance_id(&self) -> InstanceId {
        self.instance_id
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceProfile {
    pub displayed_points: ResourceBinding<PointSet>,
    pub aspected_points: ResourceBinding<PointSet>,
    pub transit_points: ResourceBinding<PointSet>,
    pub aspects: ResourceBinding<AspectSet>,
    pub analysis: ResourceBinding<AnalysisProfile>,
    pub theme: ResourceBinding<Theme>,
    pub wheel: ResourceBinding<WheelTemplate>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceUiState {
    pub sidebar_width: f32,
    pub inspector_open: bool,
    pub focused_panel: Option<PanelId>,
    pub scroll_positions: Vec<(PanelId, f64)>,
}

impl DomainValidate for WorkspaceDocument {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        let mut instance_ids = self
            .chart_instances
            .iter()
            .map(WorkspaceDocumentChart::instance_id)
            .collect::<Vec<_>>();
        instance_ids.sort_unstable();
        if instance_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "chart_instances.instance_id",
                DomainValidationIssue::Duplicate,
            ));
        }
        let mut view_ids = self.views.iter().map(|view| view.id).collect::<Vec<_>>();
        view_ids.sort_unstable();
        if view_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "views.id",
                DomainValidationIssue::Duplicate,
            ));
        }
        for (index, view) in self.views.iter().enumerate() {
            if let ResourceBinding::Inline { value } = &view.document {
                value
                    .domain_validate()
                    .map_err(|error| error.prepend(&format!("views[{index}].document")))?;
            }
        }
        self.profile.domain_validate()
    }
}

impl DomainValidate for WorkspaceProfile {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        validate_binding(&self.displayed_points, "displayed_points")?;
        validate_binding(&self.aspected_points, "aspected_points")?;
        validate_binding(&self.transit_points, "transit_points")?;
        validate_binding(&self.aspects, "aspects")?;
        validate_binding(&self.analysis, "analysis")?;
        validate_binding(&self.theme, "theme")?;
        validate_binding(&self.wheel, "wheel")
    }
}

fn validate_binding<T: DomainValidate>(
    binding: &ResourceBinding<T>,
    path: &str,
) -> Result<(), DomainValidationError> {
    if let ResourceBinding::Inline { value } = binding {
        value
            .domain_validate()
            .map_err(|error| error.prepend(path))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalysisProfile, AspectFieldSpec, HouseDisplaySpec, LabelSpec, PointSet, RingSpec,
        ZodiacDisplaySpec,
    };

    fn profile() -> WorkspaceProfile {
        let empty_points = || ResourceBinding::Inline {
            value: PointSet { points: Vec::new() },
        };
        WorkspaceProfile {
            displayed_points: empty_points(),
            aspected_points: empty_points(),
            transit_points: empty_points(),
            aspects: ResourceBinding::Inline {
                value: AspectSet {
                    aspects: Vec::new(),
                },
            },
            analysis: ResourceBinding::Inline {
                value: AnalysisProfile::default(),
            },
            theme: ResourceBinding::Inline {
                value: Theme {
                    background: "white".into(),
                    foreground: "black".into(),
                    muted: "gray".into(),
                    accent: "blue".into(),
                    aspect_color: "red".into(),
                },
            },
            wheel: ResourceBinding::Inline {
                value: WheelTemplate {
                    rings: Vec::<RingSpec>::new(),
                    aspect_field: AspectFieldSpec { radius: 1.0 },
                    houses: HouseDisplaySpec {
                        show_cusps: true,
                        show_numbers: true,
                    },
                    zodiac: ZodiacDisplaySpec {
                        show_boundaries: true,
                        show_labels: true,
                    },
                    labels: LabelSpec {
                        show_degrees: true,
                        show_retrograde: true,
                    },
                },
            },
        }
    }

    #[test]
    fn document_contains_only_saved_chart_references_and_no_interaction_state() {
        let chart = WorkspaceDocumentChart {
            instance_id: InstanceId::new(),
            definition: ResourceId::new(),
        };
        let workspace = WorkspaceDocument {
            chart_instances: vec![chart],
            views: Vec::new(),
            profile: profile(),
        };

        let json = serde_json::to_value(workspace).expect("document serializes");
        assert!(json.get("active_chart").is_none());
        assert!(json.get("selected_charts").is_none());
        assert!(json.get("active_view").is_none());
    }
}

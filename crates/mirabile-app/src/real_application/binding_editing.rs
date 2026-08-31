use super::{
    AnalysisProfile, AppError, AppErrorKind, AppResult, AspectSet, BoundPayload,
    CalculationRuntime, CanonicalResource, Catalog, ConfigurationLayer, DomainValidate, PointSet,
    RealApplication, ResourceBinding, ResourceRepository, Theme, ViewDocument, ViewInstance,
    ViewInstanceId, WheelTemplate, info, resolve_typed_binding,
};
use crate::{
    ViewDisplayMutation, WorkspaceBindingSelection, WorkspaceBindingSlot,
    WorkspaceCompositionMutation, real_application::validation::validate_session_references,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    #[allow(clippy::too_many_lines)]
    pub(super) fn create_wheel_view(
        &self,
        title: &str,
        radix: mirabile_core::InstanceId,
        comparison: Option<mirabile_core::InstanceId>,
    ) -> AppResult<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "A view title must not be empty",
            ));
        }
        if comparison == Some(radix) {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "Radix and comparison must be distinct charts",
            ));
        }
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        for chart in std::iter::once(radix).chain(comparison) {
            if !session
                .document
                .chart_instances
                .iter()
                .any(|candidate| candidate.instance_id == chart)
            {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    format!("Chart {chart} must be saved and open before creating a view"),
                ));
            }
        }
        let radix_slot =
            mirabile_core::ChartSlotId::new("radix").expect("built-in chart slot is valid");
        let comparison_slot =
            mirabile_core::ChartSlotId::new("comparison").expect("built-in chart slot is valid");
        let mut chart_slots = vec![mirabile_core::ChartSlot {
            id: radix_slot.clone(),
            label: "Radix".into(),
            required: true,
        }];
        let mut charts = std::collections::BTreeMap::from([(radix_slot.clone(), radix)]);
        let mut rings = vec![mirabile_core::RingSpec {
            chart_slot: radix_slot.clone(),
            point_role: mirabile_core::PointRole::Primary,
            geometry: mirabile_core::RingGeometry {
                inner_radius: 108.0,
                outer_radius: 124.0,
            },
        }];
        if let Some(comparison) = comparison {
            chart_slots.push(mirabile_core::ChartSlot {
                id: comparison_slot.clone(),
                label: "Comparison".into(),
                required: true,
            });
            charts.insert(comparison_slot.clone(), comparison);
            rings.push(mirabile_core::RingSpec {
                chart_slot: comparison_slot.clone(),
                point_role: mirabile_core::PointRole::Comparison,
                geometry: mirabile_core::RingGeometry {
                    inner_radius: 134.0,
                    outer_radius: 150.0,
                },
            });
        }
        let document = mirabile_core::ViewDocument {
            chart_slots,
            objects: vec![mirabile_core::ViewObject::Wheel(
                mirabile_core::WheelObject {
                    slot: radix_slot,
                    frame: mirabile_core::ObjectFrame {
                        x: 0.0,
                        y: 0.0,
                        width: 520.0,
                        height: 520.0,
                    },
                },
            )],
            layout: mirabile_core::PageLayout {
                width: 520.0,
                height: 520.0,
            },
        };
        let mut wheel = resolve_typed_binding(
            &session.document.profile.wheel,
            &state.catalog,
            ConfigurationLayer::Workspace,
        )
        .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?
        .value;
        wheel.rings = rings;
        let view_id = ViewInstanceId::new();
        let mut overrides = mirabile_core::ViewOverrides::default();
        if comparison.is_some() {
            overrides.aspect_layers = mirabile_core::AspectLayerVisibility {
                radix_intra: false,
                comparison_intra: false,
                cross_chart: true,
            };
        }
        session.document.views.push(ViewInstance {
            id: view_id,
            title: title.into(),
            document: ResourceBinding::Inline { value: document },
            charts,
            points: None,
            aspects: None,
            analysis: None,
            wheel: Some(ResourceBinding::Inline { value: wheel }),
            theme: None,
            overrides,
        });
        session.active_view = Some(view_id);
        session.document_dirty = true;
        state.session = Some(session);
        state.ensure_view_runtimes();
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info("Wheel view created; save the workspace to persist it"));
        state.advance()
    }

    pub(super) fn apply_view_display(
        &self,
        view_id: ViewInstanceId,
        mutation: ViewDisplayMutation,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let view = session
            .document
            .views
            .iter_mut()
            .find(|view| view.id == view_id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("View {view_id} was not found"),
                )
            })?;
        match mutation {
            ViewDisplayMutation::SetPointHidden {
                slot,
                point_id,
                hidden,
            } => {
                let points = view
                    .overrides
                    .hidden_points_by_slot
                    .entry(slot)
                    .or_default();
                if hidden && !points.contains(&point_id) {
                    points.push(point_id);
                    points.sort();
                } else if !hidden {
                    points.retain(|point| point != &point_id);
                }
            }
            ViewDisplayMutation::SetRingHidden { slot, hidden } => {
                if hidden && !view.overrides.hidden_rings.contains(&slot) {
                    view.overrides.hidden_rings.push(slot);
                    view.overrides.hidden_rings.sort();
                } else if !hidden {
                    view.overrides
                        .hidden_rings
                        .retain(|candidate| candidate != &slot);
                }
            }
            ViewDisplayMutation::SetAspectLayer { layer, visible } => match layer {
                mirabile_core::AspectLayerKind::RadixIntra => {
                    view.overrides.aspect_layers.radix_intra = visible;
                }
                mirabile_core::AspectLayerKind::ComparisonIntra => {
                    view.overrides.aspect_layers.comparison_intra = visible;
                }
                mirabile_core::AspectLayerKind::CrossChart => {
                    view.overrides.aspect_layers.cross_chart = visible;
                }
            },
            ViewDisplayMutation::SetRotation(rotation) => view.overrides.rotation = rotation,
            ViewDisplayMutation::SetWheel(wheel) => {
                wheel.domain_validate().map_err(|error| {
                    AppError::new(AppErrorKind::InvalidIntent, error.to_string())
                })?;
                view.wheel = Some(ResourceBinding::Inline { value: wheel });
            }
            ViewDisplayMutation::SetAspectSet(aspects) => {
                aspects.domain_validate().map_err(|error| {
                    AppError::new(AppErrorKind::InvalidIntent, error.to_string())
                })?;
                view.aspects = Some(ResourceBinding::Inline { value: aspects });
            }
            ViewDisplayMutation::SetTheme(theme) => {
                theme.domain_validate().map_err(|error| {
                    AppError::new(AppErrorKind::InvalidIntent, error.to_string())
                })?;
                view.theme = Some(ResourceBinding::Inline { value: theme });
            }
        }
        session.document_dirty = true;
        state.session = Some(session);
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "View display changed; save the workspace to persist it",
        ));
        state.advance()
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn apply_view_display_patch(
        &self,
        view_id: ViewInstanceId,
        patch: crate::ViewDisplayPatchV1,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let view_index = session
            .document
            .views
            .iter()
            .position(|view| view.id == view_id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("View {view_id} was not found"),
                )
            })?;
        let mut wheel = resolve_typed_binding(
            session.document.views[view_index]
                .wheel
                .as_ref()
                .unwrap_or(&session.document.profile.wheel),
            &state.catalog,
            ConfigurationLayer::View,
        )
        .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?
        .value;
        let mut aspects = resolve_typed_binding(
            session.document.views[view_index]
                .aspects
                .as_ref()
                .unwrap_or(&session.document.profile.aspects),
            &state.catalog,
            ConfigurationLayer::View,
        )
        .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?
        .value;
        let view = &mut session.document.views[view_index];
        for (slot, visibility) in patch.point_visibility {
            let slot = mirabile_core::ChartSlotId::new(&slot)
                .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
            let hidden_points = view
                .overrides
                .hidden_points_by_slot
                .entry(slot)
                .or_default();
            for (point, hidden) in visibility.hidden {
                if hidden && !hidden_points.contains(&point) {
                    hidden_points.push(point);
                } else if !hidden {
                    hidden_points.retain(|candidate| candidate != &point);
                }
            }
            hidden_points.sort();
        }
        for (slot, visible) in patch.ring_visibility {
            let slot = mirabile_core::ChartSlotId::new(&slot)
                .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
            if visible {
                view.overrides
                    .hidden_rings
                    .retain(|candidate| candidate != &slot);
            } else if !view.overrides.hidden_rings.contains(&slot) {
                view.overrides.hidden_rings.push(slot);
            }
        }
        view.overrides.hidden_rings.sort();
        if let Some(rotation) = patch.rotation_degrees {
            view.overrides.rotation = Some(
                mirabile_core::Angle::from_degrees(rotation).map_err(|error| {
                    AppError::new(AppErrorKind::InvalidIntent, error.to_string())
                })?,
            );
        }
        if let Some(value) = patch.zodiac_boundaries {
            wheel.zodiac.show_boundaries = value;
        }
        if let Some(value) = patch.zodiac_labels {
            wheel.zodiac.show_labels = value;
        }
        if let Some(value) = patch.house_cusps {
            wheel.houses.show_cusps = value;
        }
        if let Some(value) = patch.house_numbers {
            wheel.houses.show_numbers = value;
        }
        if let Some(value) = patch.degree_labels {
            wheel.labels.show_degrees = value;
        }
        if let Some(value) = patch.retrograde_markers {
            wheel.labels.show_retrograde = value;
        }
        if let Some(value) = patch.radix_intra_aspects {
            view.overrides.aspect_layers.radix_intra = value;
        }
        if let Some(value) = patch.comparison_intra_aspects {
            view.overrides.aspect_layers.comparison_intra = value;
        }
        if let Some(value) = patch.cross_chart_aspects {
            view.overrides.aspect_layers.cross_chart = value;
        }
        for (aspect_id, enabled) in patch.aspect_enabled {
            let aspect = aspects
                .aspects
                .iter_mut()
                .find(|aspect| aspect.id == aspect_id)
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!("Aspect {aspect_id} was not found"),
                    )
                })?;
            aspect.enabled = enabled;
        }
        for (aspect_id, degrees) in patch.aspect_orbs_degrees {
            let aspect = aspects
                .aspects
                .iter_mut()
                .find(|aspect| aspect.id == aspect_id)
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!("Aspect {aspect_id} was not found"),
                    )
                })?;
            aspect.orbs.maximum = mirabile_core::Angle::from_degrees(degrees)
                .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
        }
        wheel
            .domain_validate()
            .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
        aspects
            .domain_validate()
            .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
        view.wheel = Some(ResourceBinding::Inline { value: wheel });
        view.aspects = Some(ResourceBinding::Inline { value: aspects });
        if let Some(theme) = patch.theme {
            view.theme = Some(match theme {
                crate::ThemeSelectionV1::MirabileDark => ResourceBinding::Inline {
                    value: Theme::mirabile_dark(),
                },
                crate::ThemeSelectionV1::HighContrastLight => ResourceBinding::Inline {
                    value: Theme::high_contrast_light(),
                },
                crate::ThemeSelectionV1::Saved(resource_id) => {
                    resolve_typed_binding::<Theme>(
                        &ResourceBinding::Follow { id: resource_id },
                        &state.catalog,
                        ConfigurationLayer::View,
                    )
                    .map_err(|error| {
                        AppError::new(AppErrorKind::InvalidIntent, error.to_string())
                    })?;
                    ResourceBinding::Follow { id: resource_id }
                }
            });
        }
        session.document_dirty = true;
        state.session = Some(session);
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "View display patch applied atomically; save the workspace to persist it",
        ));
        state.advance()
    }

    pub(super) fn set_workspace_binding(
        &self,
        slot: WorkspaceBindingSlot,
        selection: WorkspaceBindingSelection,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        match slot {
            WorkspaceBindingSlot::DisplayedPoints => {
                session.document.profile.displayed_points =
                    selected_binding::<PointSet>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::AspectedPoints => {
                session.document.profile.aspected_points =
                    selected_binding::<PointSet>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::TransitPoints => {
                session.document.profile.transit_points =
                    selected_binding::<PointSet>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::Aspects => {
                session.document.profile.aspects =
                    selected_binding::<AspectSet>(&state.catalog, selection)?;
                state.editor = None;
            }
            WorkspaceBindingSlot::Analysis => {
                session.document.profile.analysis =
                    selected_binding::<AnalysisProfile>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::Theme => {
                session.document.profile.theme =
                    selected_binding::<Theme>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::Wheel => {
                session.document.profile.wheel =
                    selected_binding::<WheelTemplate>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::ViewDocument { view_id } => {
                let view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                view.document = selected_binding::<ViewDocument>(&state.catalog, selection)?;
            }
        }
        session.document_dirty = true;
        state.session = Some(session);
        state.ensure_view_runtimes();
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Workspace binding mode changed; save the workspace to persist it",
        ));
        state.advance()
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn apply_workspace_composition(
        &self,
        mutation: WorkspaceCompositionMutation,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let mut refresh = false;

        match mutation {
            WorkspaceCompositionMutation::MoveChart {
                instance_id,
                before,
            } => {
                move_item(
                    &mut session.document.chart_instances,
                    |chart| chart.instance_id == instance_id,
                    |chart| before.is_some_and(|before| chart.instance_id == before),
                    "chart instance",
                    instance_id,
                )?;
            }
            WorkspaceCompositionMutation::AddView { document } => {
                let document = selected_binding::<ViewDocument>(&state.catalog, document)?;
                let resolved =
                    resolve_typed_binding(&document, &state.catalog, ConfigurationLayer::View)
                        .map_err(|error| {
                            AppError::new(
                                AppErrorKind::InvalidIntent,
                                format!("The selected ViewDocument could not be resolved: {error}"),
                            )
                        })?;
                let view_id = ViewInstanceId::new();
                let saved_chart = session
                    .document
                    .chart_instances
                    .first()
                    .map(|chart| chart.instance_id);
                let draft_chart = session.draft_charts.first().map(|chart| chart.instance_id);
                let mut charts = std::collections::BTreeMap::new();
                let mut draft_assignments = std::collections::BTreeMap::new();
                for slot in resolved
                    .value
                    .chart_slots
                    .iter()
                    .filter(|slot| slot.required)
                {
                    if let Some(instance_id) = saved_chart {
                        charts.insert(slot.id.clone(), instance_id);
                    } else if let Some(instance_id) = draft_chart {
                        draft_assignments.insert(slot.id.clone(), instance_id);
                    }
                }
                session.document.views.push(ViewInstance {
                    id: view_id,
                    title: "Wheel".into(),
                    document,
                    charts,
                    points: None,
                    aspects: None,
                    analysis: None,
                    wheel: None,
                    theme: None,
                    overrides: mirabile_core::ViewOverrides::default(),
                });
                if !draft_assignments.is_empty() {
                    session
                        .draft_chart_assignments
                        .insert(view_id, draft_assignments);
                }
                session.active_view = Some(view_id);
                refresh = true;
            }
            WorkspaceCompositionMutation::RemoveView { view_id } => {
                let index = session
                    .document
                    .views
                    .iter()
                    .position(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                session.document.views.remove(index);
                session.draft_chart_assignments.remove(&view_id);
                session.temporary_view_overrides.remove(&view_id);
                if session.active_view == Some(view_id) {
                    session.active_view = session
                        .document
                        .views
                        .get(index)
                        .or_else(|| session.document.views.last())
                        .map(|view| view.id);
                    refresh = session.active_view.is_some();
                }
            }
            WorkspaceCompositionMutation::MoveView { view_id, before } => {
                move_item(
                    &mut session.document.views,
                    |view| view.id == view_id,
                    |view| before.is_some_and(|before| view.id == before),
                    "view",
                    view_id,
                )?;
            }
            WorkspaceCompositionMutation::SetRotation { view_id, rotation } => {
                let view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                view.overrides.rotation = rotation;
                refresh = session.active_view == Some(view_id);
            }
            WorkspaceCompositionMutation::SetPointHidden {
                view_id,
                point_id,
                hidden,
            } => {
                let view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                if hidden && !view.overrides.hidden_points.contains(&point_id) {
                    view.overrides.hidden_points.push(point_id);
                    view.overrides.hidden_points.sort();
                } else if !hidden {
                    view.overrides
                        .hidden_points
                        .retain(|point| point != &point_id);
                }
                refresh = session.active_view == Some(view_id);
            }
        }

        session.document_dirty = true;
        validate_session_references(&session, &state.catalog).map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("Workspace composition failed referential validation: {error}"),
            )
        })?;
        state.session = Some(session);
        state.ensure_view_runtimes();
        if refresh {
            self.submit_active_view_refresh(&mut state)?;
        }
        state.notice = Some(info(
            "Workspace composition changed; save the workspace to persist it",
        ));
        state.advance()
    }
}

fn move_item<T, F, B, I>(
    items: &mut Vec<T>,
    is_item: F,
    is_before: B,
    label: &str,
    identity: I,
) -> AppResult<()>
where
    F: Fn(&T) -> bool,
    B: Fn(&T) -> bool,
    I: std::fmt::Display,
{
    let source = items.iter().position(is_item).ok_or_else(|| {
        AppError::new(
            AppErrorKind::NotFound,
            format!("{label} {identity} was not found"),
        )
    })?;
    let item = items.remove(source);
    let destination = items.iter().position(is_before).unwrap_or(items.len());
    items.insert(destination, item);
    Ok(())
}

fn selected_binding<T: BoundPayload>(
    catalog: &Catalog,
    selection: WorkspaceBindingSelection,
) -> AppResult<ResourceBinding<T>> {
    match selection {
        WorkspaceBindingSelection::Follow { resource_id } => {
            require_payload::<T>(catalog.current.get(&resource_id), resource_id, None)?;
            Ok(ResourceBinding::Follow { id: resource_id })
        }
        WorkspaceBindingSelection::Pinned {
            resource_id,
            revision,
        } => {
            require_payload::<T>(
                catalog.history.get(&(resource_id, revision)),
                resource_id,
                Some(revision),
            )?;
            Ok(ResourceBinding::Pinned {
                id: resource_id,
                revision,
            })
        }
        WorkspaceBindingSelection::Inline { resource_id } => {
            let envelope =
                require_payload::<T>(catalog.current.get(&resource_id), resource_id, None)?;
            Ok(ResourceBinding::Inline {
                value: envelope.payload.clone(),
            })
        }
    }
}

fn require_payload<T: BoundPayload>(
    resource: Option<&CanonicalResource>,
    resource_id: crate::ResourceId,
    revision: Option<crate::Revision>,
) -> AppResult<&mirabile_core::ResourceEnvelope<T>> {
    resource.and_then(T::envelope).ok_or_else(|| {
        AppError::new(
            AppErrorKind::NotFound,
            revision.map_or_else(
                || format!("Compatible resource {resource_id} was not found"),
                |revision| {
                    format!("Compatible resource {resource_id} revision {revision} was not found")
                },
            ),
        )
    })
}

use super::{
    AppError, AppErrorKind, AppResult, CalculationSpec, ConfigurationLayer, ConfigurationStack,
    EffectiveConfiguration, RealState, Resolved, ValueSource, ViewInstance, resolve_typed_binding,
    view_resolution_error,
};

impl RealState {
    #[allow(clippy::too_many_lines)]
    pub(super) fn effective_configuration(
        &self,
        calculation_spec: &CalculationSpec,
        view: &ViewInstance,
    ) -> AppResult<EffectiveConfiguration> {
        let workspace = self.workspace().ok_or_else(|| {
            AppError::new(AppErrorKind::ViewComputation, "No workspace is hydrated")
        })?;
        let calculation = ConfigurationStack {
            built_in: CalculationSpec::default(),
            user_default: None,
            workspace: None,
            chart_definition: Some(calculation_spec.clone()),
            view_override: None,
            editor_preview: None,
        }
        .resolve();
        let points_binding = view
            .points
            .as_ref()
            .unwrap_or(&workspace.profile.displayed_points);
        let mut displayed_points = resolve_typed_binding(
            points_binding,
            &self.catalog,
            if view.points.is_some() {
                ConfigurationLayer::View
            } else {
                ConfigurationLayer::Workspace
            },
        )
        .map_err(view_resolution_error)?;
        let effective_overrides = self
            .session
            .as_ref()
            .and_then(|session| session.temporary_view_overrides.get(&view.id))
            .unwrap_or(&view.overrides);
        if !effective_overrides.hidden_points.is_empty() {
            displayed_points
                .value
                .points
                .retain(|selector| match selector {
                    mirabile_core::PointSelector::Point(point) => {
                        !effective_overrides.hidden_points.contains(point)
                    }
                    mirabile_core::PointSelector::Category(_) => true,
                });
            displayed_points.layer = ConfigurationLayer::View;
            displayed_points.source = ValueSource::Inline;
        }
        let aspected_points = resolve_typed_binding(
            &workspace.profile.aspected_points,
            &self.catalog,
            ConfigurationLayer::Workspace,
        )
        .map_err(view_resolution_error)?;
        let aspect_binding = view.aspects.as_ref().unwrap_or(&workspace.profile.aspects);
        let mut aspect_set = resolve_typed_binding(
            aspect_binding,
            &self.catalog,
            if view.aspects.is_some() {
                ConfigurationLayer::View
            } else {
                ConfigurationLayer::Workspace
            },
        )
        .map_err(view_resolution_error)?;
        if let Some(editor) = &self.editor
            && workspace.profile.aspects.id() == editor.base.as_ref().map(|base| base.id)
        {
            aspect_set = Resolved {
                value: editor.draft.clone(),
                layer: ConfigurationLayer::Preview,
                source: ValueSource::Inline,
            };
        }
        let analysis_binding = view
            .analysis
            .as_ref()
            .unwrap_or(&workspace.profile.analysis);
        let analysis = resolve_typed_binding(
            analysis_binding,
            &self.catalog,
            if view.analysis.is_some() {
                ConfigurationLayer::View
            } else {
                ConfigurationLayer::Workspace
            },
        )
        .map_err(view_resolution_error)?;
        let wheel_binding = view.wheel.as_ref().unwrap_or(&workspace.profile.wheel);
        let wheel = resolve_typed_binding(
            wheel_binding,
            &self.catalog,
            if view.wheel.is_some() {
                ConfigurationLayer::View
            } else {
                ConfigurationLayer::Workspace
            },
        )
        .map_err(view_resolution_error)?;
        let theme_binding = view.theme.as_ref().unwrap_or(&workspace.profile.theme);
        let theme = resolve_typed_binding(
            theme_binding,
            &self.catalog,
            if view.theme.is_some() {
                ConfigurationLayer::View
            } else {
                ConfigurationLayer::Workspace
            },
        )
        .map_err(view_resolution_error)?;
        Ok(EffectiveConfiguration {
            calculation,
            displayed_points,
            aspected_points,
            aspect_set,
            analysis,
            wheel,
            theme,
        })
    }
}

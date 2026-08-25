use super::{
    AppError, AppErrorKind, AppResult, CalculationSpec, ConfigurationLayer, ConfigurationStack,
    EffectiveConfiguration, RealState, Resolved, ValueSource, ViewInstance, resolve_typed_binding,
    view_resolution_error,
};

impl RealState {
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
        let mut displayed_points = resolve_typed_binding(
            &workspace.profile.displayed_points,
            &self.catalog,
            ConfigurationLayer::Workspace,
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
        let mut aspect_set = resolve_typed_binding(
            &workspace.profile.aspects,
            &self.catalog,
            ConfigurationLayer::Workspace,
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
        let analysis = resolve_typed_binding(
            &workspace.profile.analysis,
            &self.catalog,
            ConfigurationLayer::Workspace,
        )
        .map_err(view_resolution_error)?;
        let wheel = resolve_typed_binding(
            &workspace.profile.wheel,
            &self.catalog,
            ConfigurationLayer::Workspace,
        )
        .map_err(view_resolution_error)?;
        let theme = resolve_typed_binding(
            &workspace.profile.theme,
            &self.catalog,
            ConfigurationLayer::Workspace,
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

use std::rc::Rc;

use futures::executor::block_on;
use mirabile_app::{
    Angle, AppIntent, AppReadModel, Application, ApplicationStatus, AspectSetDraftMutation,
    ChartMutation, ChartPersistence, ChartPersistence::Saved, DraftState, PointId, RealApplication,
    StartupPolicy, ViewComputationState, demo_ids, demo_resources,
};
use mirabile_store::{MemoryRepository, ResourceRepository};

use crate::mock_application::MockApplication;

fn mock_application() -> Rc<dyn Application> {
    Rc::new(MockApplication::new())
}

fn real_application() -> Rc<dyn Application> {
    let repository = MemoryRepository::default();
    for resource in demo_resources() {
        block_on(repository.create(resource)).expect("demo resource is seeded");
    }
    Rc::new(RealApplication::with_repository_and_policy(
        repository,
        StartupPolicy::OpenWorkspace(demo_ids().workspace),
    ))
}

async fn settle(application: &dyn Application, mut model: AppReadModel) -> AppReadModel {
    while !model.is_settled() {
        let after = model.version;
        model = application
            .wait_for_update(after)
            .await
            .expect("pending application work settles");
        assert!(model.version > after);
    }
    model
}

async fn dispatch_settled(
    application: &dyn Application,
    current: &AppReadModel,
    intent: AppIntent,
) -> AppReadModel {
    let accepted = application
        .dispatch(intent)
        .await
        .expect("intent is accepted");
    assert!(accepted.version > current.version);
    settle(application, accepted).await
}

#[allow(clippy::too_many_lines)]
async fn application_scenario(application: Rc<dyn Application>) {
    let initial = application
        .snapshot()
        .await
        .expect("initial snapshot is available");
    let loading = application
        .initialize()
        .await
        .expect("application initializes");
    assert!(loading.version > initial.version);
    assert_eq!(loading.status, ApplicationStatus::Ready);
    let mut model = settle(application.as_ref(), loading).await;
    assert!(
        model
            .active_view
            .as_ref()
            .and_then(|view| view.scene.as_ref())
            .is_some(),
        "initial calculation produces a Scene"
    );
    assert!(matches!(
        model.active_view.as_ref().map(|view| &view.computation),
        Some(ViewComputationState::Fresh)
    ));

    let aspect_set = model
        .library
        .aspect_sets
        .first()
        .expect("fixture has an Aspect Set")
        .clone();
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::BeginAspectSetEdit {
            resource_id: aspect_set.resource_id,
        },
    )
    .await;
    let initial_aspect = model
        .resource_editor
        .aspect_set
        .as_ref()
        .and_then(|draft| draft.aspects.first())
        .expect("Aspect Set editor projects a row")
        .clone();
    let changed_orb = Angle::from_degrees(initial_aspect.maximum_orb.degrees() + 0.25)
        .expect("changed orb is valid");
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetOrb {
            aspect_id: initial_aspect.aspect_id.clone(),
            maximum: changed_orb,
        }),
    )
    .await;
    assert_eq!(
        model
            .resource_editor
            .aspect_set
            .as_ref()
            .map(|draft| &draft.state),
        Some(&DraftState::Dirty {
            base_revision: aspect_set.revision,
        })
    );
    model = dispatch_settled(application.as_ref(), &model, AppIntent::CancelDraft).await;
    assert_eq!(
        model
            .resource_editor
            .aspect_set
            .as_ref()
            .and_then(|draft| draft.aspects.first())
            .map(|row| row.maximum_orb),
        Some(initial_aspect.maximum_orb)
    );
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
            aspect_id: initial_aspect.aspect_id,
            enabled: !initial_aspect.enabled,
        }),
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveDraft).await;
    assert!(matches!(
        model.resource_editor.aspect_set.as_ref().map(|draft| &draft.state),
        Some(DraftState::Clean { revision }) if *revision > aspect_set.revision
    ));

    let first_chart = model.workspace.active_chart.expect("active chart");
    for selected in model.workspace.selected_charts.clone() {
        model = dispatch_settled(
            application.as_ref(),
            &model,
            AppIntent::SetChartSelection {
                instance_id: selected,
                selected: false,
            },
        )
        .await;
    }
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::SetChartSelection {
            instance_id: first_chart,
            selected: true,
        },
    )
    .await;
    assert_eq!(model.workspace.active_chart, Some(first_chart));
    assert_eq!(model.workspace.selected_charts, vec![first_chart]);

    let unopened = model
        .library
        .charts
        .iter()
        .find(|chart| {
            !model.workspace.charts.iter().any(|open| {
                matches!(
                    open.persistence,
                    ChartPersistence::Saved { definition_id }
                        if definition_id == chart.definition_id
                )
            })
        })
        .expect("fixture has an unopened library chart")
        .definition_id;
    let previous_count = model.workspace.charts.len();
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::OpenChart {
            definition_id: unopened,
        },
    )
    .await;
    let opened = model
        .workspace
        .active_chart
        .expect("opened chart is active");
    assert_ne!(opened, first_chart);
    assert_eq!(model.workspace.charts.len(), previous_count + 1);
    assert_eq!(model.workspace.selected_charts, vec![first_chart]);
    assert!(model.workspace.document_dirty);

    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveWorkspace).await;
    assert!(!model.workspace.document_dirty);

    let opened_index = model
        .workspace
        .charts
        .iter()
        .position(|chart| chart.instance_id == opened)
        .expect("opened chart remains in the rail");
    let expected_after_close = model
        .workspace
        .charts
        .get(opened_index + 1)
        .or_else(|| {
            opened_index
                .checked_sub(1)
                .and_then(|index| model.workspace.charts.get(index))
        })
        .map(|chart| chart.instance_id);
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::SetTemporaryPointHidden {
            point_id: PointId::new("sun").expect("built-in point ID"),
            hidden: true,
        },
    )
    .await;
    assert!(!model.workspace.document_dirty);
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::PromoteTemporaryDisplay,
    )
    .await;
    assert!(model.workspace.document_dirty);
    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveWorkspace).await;
    assert!(!model.workspace.document_dirty);

    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::CloseChart {
            instance_id: opened,
        },
    )
    .await;
    assert_eq!(model.workspace.active_chart, expected_after_close);
    assert_eq!(model.workspace.selected_charts, vec![first_chart]);
    assert!(
        !model
            .workspace
            .charts
            .iter()
            .any(|chart| chart.instance_id == opened)
    );
    assert!(model.workspace.document_dirty);

    let last_good = model
        .active_view
        .as_ref()
        .and_then(|view| view.scene.clone())
        .expect("a last-good Scene exists before refresh");
    let refreshing = application
        .dispatch(AppIntent::RefreshActiveView)
        .await
        .expect("refresh is accepted");
    assert!(refreshing.version > model.version);
    assert_eq!(
        refreshing
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone()),
        Some(last_good.clone())
    );
    assert!(matches!(
        refreshing
            .active_view
            .as_ref()
            .map(|view| &view.computation),
        Some(ViewComputationState::Refreshing)
    ));
    let refreshed = settle(application.as_ref(), refreshing).await;
    let refreshed_view = refreshed.active_view.as_ref().expect("active view remains");
    assert!(refreshed_view.scene.is_some());
    if matches!(refreshed_view.computation, ViewComputationState::Failed(_)) {
        assert_eq!(refreshed_view.scene, Some(last_good));
    } else {
        assert!(matches!(
            refreshed_view.computation,
            ViewComputationState::Fresh
        ));
    }
}

#[test]
fn mock_application_conforms_to_shared_scenarios() {
    block_on(application_scenario(mock_application()));
}

#[test]
fn real_application_conforms_to_shared_scenarios() {
    block_on(application_scenario(real_application()));
}

#[test]
fn real_application_conforms_to_level_a_authoring_scenarios() {
    block_on(real_authoring_scenario(real_application()));
}

#[allow(clippy::too_many_lines)]
async fn real_authoring_scenario(application: Rc<dyn Application>) {
    let loading = application.initialize().await.expect("initialization");
    let mut model = settle(application.as_ref(), loading).await;
    let initial_library_count = model.library.charts.len();

    model = dispatch_settled(application.as_ref(), &model, AppIntent::BeginNewChart).await;
    let first_draft = model.chart_editor.as_ref().expect("new editor");
    assert_eq!(first_draft.fields.title, "Untitled Chart");
    assert!(first_draft.validation.is_empty());
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::ApplyChartMutation(ChartMutation::SetTitle("Canceled Level A".into())),
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::CancelChartEditor).await;
    assert_eq!(model.library.charts.len(), initial_library_count);
    assert!(model.chart_editor.is_none());

    model = dispatch_settled(application.as_ref(), &model, AppIntent::BeginNewChart).await;
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::ApplyChartMutation(ChartMutation::SetTitle("Level A Natal".into())),
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveChartEditor).await;
    let saved_instance = model.workspace.active_chart.expect("saved chart active");
    assert!(matches!(
        model
            .inspector
            .active_chart
            .as_ref()
            .map(|chart| &chart.persistence),
        Some(Saved { .. })
    ));
    assert_eq!(model.library.charts.len(), initial_library_count + 1);
    assert!(
        model
            .active_view
            .as_ref()
            .expect("view")
            .slots
            .iter()
            .any(|slot| slot.chart == Some(saved_instance) && slot.draft_chart.is_none())
    );

    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::BeginSavedChartEdit {
            instance_id: saved_instance,
        },
    )
    .await;
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::ApplyChartMutation(ChartMutation::SetTitle("Canceled saved title".into())),
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::CancelChartEditor).await;
    assert_eq!(
        model
            .inspector
            .active_chart
            .as_ref()
            .map(|chart| chart.title.as_str()),
        Some("Level A Natal")
    );

    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::BeginSavedChartEdit {
            instance_id: saved_instance,
        },
    )
    .await;
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::ApplyChartMutation(ChartMutation::SetTitle("Level A Natal Revised".into())),
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveChartEditor).await;
    assert_eq!(
        model
            .inspector
            .active_chart
            .as_ref()
            .map(|chart| chart.title.as_str()),
        Some("Level A Natal Revised")
    );

    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveWorkspace).await;
    let workspace_revision = model.workspace.document_revision.expect("saved workspace");
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::RenameWorkspace {
            title: "Level A Workspace".into(),
        },
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveWorkspace).await;
    assert!(model.workspace.document_revision.expect("revision") > workspace_revision);
    assert_eq!(model.workspace.title, "Level A Workspace");

    model = dispatch_settled(application.as_ref(), &model, AppIntent::BeginNewAspectSet).await;
    let rows = model
        .resource_editor
        .aspect_set
        .as_ref()
        .expect("new Aspect Set")
        .aspects
        .clone();
    assert!(
        rows.len() >= 2,
        "new sets expose the supported authoring vocabulary"
    );
    for row in rows {
        model = dispatch_settled(
            application.as_ref(),
            &model,
            AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                aspect_id: row.aspect_id,
                enabled: !row.enabled,
            }),
        )
        .await;
    }
    model = dispatch_settled(
        application.as_ref(),
        &model,
        AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetTitle("Level A Aspects".into())),
    )
    .await;
    model = dispatch_settled(application.as_ref(), &model, AppIntent::SaveDraft).await;
    assert!(matches!(
        model
            .resource_editor
            .aspect_set
            .as_ref()
            .map(|draft| &draft.state),
        Some(DraftState::Clean { .. })
    ));
    assert!(
        model
            .library
            .aspect_sets
            .iter()
            .any(|summary| summary.title == "Level A Aspects")
    );
}

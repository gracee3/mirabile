#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
pub(super) const BRIDGE_NAME: &str = "__mirabileWorkbenchV1";

#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AutomationConfiguration {
    pub database_name: String,
}

#[cfg(any(test, target_arch = "wasm32"))]
pub(super) fn parse_configuration(
    enabled: Option<&str>,
    database_name: Option<&str>,
) -> Result<Option<AutomationConfiguration>, String> {
    if enabled != Some("1") {
        return Ok(None);
    }
    let database_name = database_name
        .ok_or_else(|| "automation mode requires an isolated database name".to_owned())?;
    if valid_database_name(database_name) {
        Ok(Some(AutomationConfiguration {
            database_name: database_name.to_owned(),
        }))
    } else {
        Err("automation database names must start with mirabile-workbench-e2e- or mirabile-workbench-dev- and contain only lowercase ASCII letters, digits, and hyphens".into())
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
fn valid_database_name(value: &str) -> bool {
    let prefix = ["mirabile-workbench-e2e-", "mirabile-workbench-dev-"]
        .into_iter()
        .find(|prefix| value.starts_with(prefix));
    prefix.is_some_and(|prefix| {
        value.len() > prefix.len()
            && value.len() <= 96
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn configuration_from_window() -> Option<AutomationConfiguration> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let parameters = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    match parse_configuration(
        parameters.get("mirabileAutomation").as_deref(),
        parameters.get("database").as_deref(),
    ) {
        Ok(configuration) => configuration,
        Err(message) => {
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));
            None
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn install(
    model: leptos::prelude::RwSignal<mirabile_app::AppReadModel>,
    coordinator: crate::dispatcher::WorkbenchCoordinator,
) {
    use std::{cell::Cell, rc::Rc};

    use js_sys::{Object, Reflect};
    use leptos::prelude::GetUntracked;
    use mirabile_app::{ActionSource, AutomationSnapshotV1};
    use wasm_bindgen::{JsValue, closure::Closure};

    let Some(window) = web_sys::window() else {
        return;
    };
    let bridge = Object::new();
    let _ = Reflect::set(
        &bridge,
        &JsValue::from_str("version"),
        &JsValue::from_f64(1.0),
    );

    let snapshot = Closure::<dyn Fn() -> String>::new(move || {
        let controls = crate::control_manifest::capture()
            .map(|manifest| manifest.controls)
            .unwrap_or_default();
        json_envelope(
            "snapshot",
            AutomationSnapshotV1::capture(
                &model.get_untracked(),
                controls,
                coordinator.read_model(),
                coordinator.trace(),
            ),
        )
    });
    set_function(&bridge, "snapshot", &snapshot);
    snapshot.forget();

    let controls =
        Closure::<dyn Fn() -> String>::new(move || match crate::control_manifest::capture() {
            Ok(manifest) => json_envelope("controls", manifest),
            Err(error) => json_error("controls", &error),
        });
    set_function(&bridge, "controls", &controls);
    controls.forget();

    let trace =
        Closure::<dyn Fn() -> String>::new(move || json_envelope("trace", coordinator.trace()));
    set_function(&bridge, "trace", &trace);
    trace.forget();

    let settled = Closure::<dyn Fn() -> bool>::new(move || model.get_untracked().is_settled());
    set_function(&bridge, "waitSettled", &settled);
    settled.forget();

    let next_source = Rc::new(Cell::new(ActionSource::Agent));
    let source_cell = Rc::clone(&next_source);
    let set_source = Closure::<dyn Fn(String) -> String>::new(move |source: String| {
        let source = match source.as_str() {
            "agent" => ActionSource::Agent,
            "human" => ActionSource::Human,
            "macro" => ActionSource::Macro,
            "system" => ActionSource::System,
            "test" => ActionSource::Test,
            _ => return json_error("set_action_source", "unknown action source"),
        };
        source_cell.set(source);
        json_ok("set_action_source")
    });
    set_function(&bridge, "setActionSource", &set_source);
    set_source.forget();

    let execute_source = Rc::clone(&next_source);
    let execute = Closure::<dyn Fn(String) -> String>::new(move |request: String| {
        let request = match serde_json::from_str::<ExecuteRequest>(&request) {
            Ok(request) => request,
            Err(error) => return json_error("execute", &format!("invalid request: {error}")),
        };
        let intent = match request.action.into_intent() {
            Ok(intent) => intent,
            Err(error) => return json_error("execute", &error),
        };
        let source = execute_source.replace(ActionSource::Agent);
        coordinator.dispatch_from(intent, source, request.origin_control);
        json_ok("execute")
    });
    set_function(&bridge, "execute", &execute);
    execute.forget();

    let replay = Closure::<dyn Fn(String) -> String>::new(|_: String| {
        json_error(
            "macro_replay",
            "macro replay is not enabled until the macro schema phase",
        )
    });
    set_function(&bridge, "replayMacro", &replay);
    replay.forget();

    let _ = Reflect::set(&window, &JsValue::from_str(BRIDGE_NAME), &bridge);

    fn set_function<T>(object: &Object, name: &str, closure: &Closure<T>)
    where
        T: ?Sized + wasm_bindgen::closure::WasmClosure,
    {
        let _ = Reflect::set(object, &JsValue::from_str(name), closure.as_ref());
    }

    fn json_envelope<T: Serialize>(kind: &str, value: T) -> String {
        serde_json::to_string(&BridgeEnvelope {
            ok: true,
            kind,
            value: Some(value),
            error: None,
        })
        .unwrap_or_else(|error| json_error(kind, &error.to_string()))
    }

    fn json_ok(kind: &str) -> String {
        json_envelope(kind, serde_json::Value::Null)
    }

    fn json_error(kind: &str, error: &str) -> String {
        serde_json::to_string(&BridgeEnvelope::<serde_json::Value> {
            ok: false,
            kind,
            value: None,
            error: Some(error),
        })
        .unwrap_or_else(|_| {
            format!(r#"{{"ok":false,"kind":"{kind}","error":"serialization failure"}}"#)
        })
    }

    #[derive(Serialize)]
    struct BridgeEnvelope<'a, T> {
        ok: bool,
        kind: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<T>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
struct ExecuteRequest {
    action: WhitelistedAction,
    #[serde(default)]
    origin_control: Option<mirabile_app::ControlAddress>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum WhitelistedAction {
    BeginNewChart,
    BeginSavedChartEdit {
        instance_id: mirabile_app::InstanceId,
    },
    NewWorkspace,
    OpenWorkspace {
        resource_id: mirabile_app::ResourceId,
    },
    RenameWorkspace {
        title: String,
    },
    DiscardWorkspaceChanges,
    ResolveWorkspaceSwitch {
        resolution: mirabile_app::WorkspaceSwitchAction,
    },
    LoadDemoBundle,
    SaveChartEditor,
    CancelChartEditor,
    SetChartTitle {
        title: String,
    },
    SetChartEventKind {
        event_kind: mirabile_app::EventKind,
    },
    SetChartSubjectName {
        subject_name: Option<String>,
    },
    SetChartCivilDate {
        date: mirabile_app::CivilDate,
    },
    SetChartCivilTime {
        time: mirabile_app::CivilTime,
    },
    SetChartTimezone {
        timezone: mirabile_app::ChartTimezone,
    },
    SetChartLocationEnabled {
        enabled: bool,
    },
    SetChartLocationName {
        name: String,
    },
    SetChartLatitude {
        latitude: Option<mirabile_app::Latitude>,
    },
    SetChartLongitude {
        longitude: Option<mirabile_app::Longitude>,
    },
    SetChartZodiac {
        zodiac: mirabile_app::ZodiacSpec,
    },
    SetChartHouses {
        houses: mirabile_app::HouseSystem,
    },
    SetChartCoordinates {
        coordinates: mirabile_app::CoordinateSystem,
    },
    ActivateChart {
        instance_id: mirabile_app::InstanceId,
    },
    BeginAspectSetEdit {
        resource_id: mirabile_app::ResourceId,
    },
    BeginNewAspectSet,
    DuplicateAspectSet {
        resource_id: mirabile_app::ResourceId,
    },
    CancelDraft,
    CloseChart {
        instance_id: mirabile_app::InstanceId,
    },
    OpenChart {
        definition_id: mirabile_app::ResourceId,
    },
    PromoteTemporaryDisplay,
    RefreshActiveView,
    SaveDraft,
    SaveWorkspace,
    SetActiveView {
        view_id: mirabile_app::ViewInstanceId,
    },
    SetChartSelection {
        instance_id: mirabile_app::InstanceId,
        selected: bool,
    },
    SetTemporaryPointHidden {
        point_id: mirabile_app::PointId,
        hidden: bool,
    },
    SetWorkspaceAspectSet {
        resource_id: mirabile_app::ResourceId,
    },
    UpdateAspectEnabled {
        aspect_id: mirabile_app::AspectId,
        enabled: bool,
    },
    UpdateAspectOrb {
        aspect_id: mirabile_app::AspectId,
        degrees: f64,
    },
    SetAspectTitle {
        title: String,
    },
}

#[cfg(target_arch = "wasm32")]
impl WhitelistedAction {
    fn into_intent(self) -> Result<mirabile_app::AppIntent, String> {
        use mirabile_app::{AppIntent, AspectSetDraftMutation};
        Ok(match self {
            Self::BeginNewChart => AppIntent::BeginNewChart,
            Self::BeginSavedChartEdit { instance_id } => {
                AppIntent::BeginSavedChartEdit { instance_id }
            }
            Self::NewWorkspace => AppIntent::NewWorkspace,
            Self::OpenWorkspace { resource_id } => AppIntent::OpenWorkspace { resource_id },
            Self::RenameWorkspace { title } => AppIntent::RenameWorkspace { title },
            Self::DiscardWorkspaceChanges => AppIntent::DiscardWorkspaceChanges,
            Self::ResolveWorkspaceSwitch { resolution } => {
                AppIntent::ResolveWorkspaceSwitch { action: resolution }
            }
            Self::LoadDemoBundle => AppIntent::LoadDemoBundle,
            Self::SaveChartEditor => AppIntent::SaveChartEditor,
            Self::CancelChartEditor => AppIntent::CancelChartEditor,
            Self::SetChartTitle { title } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetTitle(title))
            }
            Self::SetChartEventKind { event_kind } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetEventKind(event_kind))
            }
            Self::SetChartSubjectName { subject_name } => AppIntent::ApplyChartMutation(
                mirabile_app::ChartMutation::SetSubjectName(subject_name),
            ),
            Self::SetChartCivilDate { date } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetCivilDate(date))
            }
            Self::SetChartCivilTime { time } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetCivilTime(time))
            }
            Self::SetChartTimezone { timezone } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetTimezone(timezone))
            }
            Self::SetChartLocationEnabled { enabled } => AppIntent::ApplyChartMutation(
                mirabile_app::ChartMutation::SetLocationEnabled(enabled),
            ),
            Self::SetChartLocationName { name } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetLocationName(name))
            }
            Self::SetChartLatitude { latitude } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetLatitude(latitude))
            }
            Self::SetChartLongitude { longitude } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetLongitude(longitude))
            }
            Self::SetChartZodiac { zodiac } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetZodiac(zodiac))
            }
            Self::SetChartHouses { houses } => {
                AppIntent::ApplyChartMutation(mirabile_app::ChartMutation::SetHouseSystem(houses))
            }
            Self::SetChartCoordinates { coordinates } => AppIntent::ApplyChartMutation(
                mirabile_app::ChartMutation::SetCoordinateSystem(coordinates),
            ),
            Self::ActivateChart { instance_id } => AppIntent::ActivateChart { instance_id },
            Self::BeginAspectSetEdit { resource_id } => {
                AppIntent::BeginAspectSetEdit { resource_id }
            }
            Self::BeginNewAspectSet => AppIntent::BeginNewAspectSet,
            Self::DuplicateAspectSet { resource_id } => {
                AppIntent::DuplicateAspectSet { resource_id }
            }
            Self::CancelDraft => AppIntent::CancelDraft,
            Self::CloseChart { instance_id } => AppIntent::CloseChart { instance_id },
            Self::OpenChart { definition_id } => AppIntent::OpenChart { definition_id },
            Self::PromoteTemporaryDisplay => AppIntent::PromoteTemporaryDisplay,
            Self::RefreshActiveView => AppIntent::RefreshActiveView,
            Self::SaveDraft => AppIntent::SaveDraft,
            Self::SaveWorkspace => AppIntent::SaveWorkspace,
            Self::SetActiveView { view_id } => AppIntent::SetActiveView { view_id },
            Self::SetChartSelection {
                instance_id,
                selected,
            } => AppIntent::SetChartSelection {
                instance_id,
                selected,
            },
            Self::SetTemporaryPointHidden { point_id, hidden } => {
                AppIntent::SetTemporaryPointHidden { point_id, hidden }
            }
            Self::SetWorkspaceAspectSet { resource_id } => {
                AppIntent::SetWorkspaceAspectSet { resource_id }
            }
            Self::UpdateAspectEnabled { aspect_id, enabled } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                    aspect_id,
                    enabled,
                })
            }
            Self::UpdateAspectOrb { aspect_id, degrees } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetOrb {
                    aspect_id,
                    maximum: mirabile_app::Angle::from_degrees(degrees)
                        .map_err(|error| error.to_string())?,
                })
            }
            Self::SetAspectTitle { title } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetTitle(title))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_requires_explicit_enablement_and_isolated_database() {
        assert_eq!(parse_configuration(None, None), Ok(None));
        assert_eq!(parse_configuration(Some("0"), Some("mirabile")), Ok(None));
        assert!(parse_configuration(Some("1"), Some("mirabile")).is_err());
        assert!(parse_configuration(Some("1"), Some("mirabile-workbench-e2e-")).is_err());
        assert_eq!(
            parse_configuration(Some("1"), Some("mirabile-workbench-e2e-42")),
            Ok(Some(AutomationConfiguration {
                database_name: "mirabile-workbench-e2e-42".into(),
            }))
        );
    }
}

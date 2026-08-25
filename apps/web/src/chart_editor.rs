use leptos::prelude::*;
use mirabile_app::{
    ActionSource, AppAction, AppIntent, AppReadModel, ChartEditorState, ChartMutation,
    ChartTimezone, CivilDate, CivilTime, ControlAddress, ControlId, ControlOptionDescriptor,
    CoordinateSystem, EventKind, HouseSystem, Latitude, Longitude, Offset, TimezoneAuthoringMode,
    ZodiacMode, ZodiacSpec,
};

use crate::{
    dispatcher::WorkbenchCoordinator,
    workbench_controls::{
        ActionControl, BufferedDateField, BufferedNumberField, BufferedTextField,
        BufferedTimeField, EnumSelect, Toggle, chart_save_pending,
    },
};

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn ChartAuthoring(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let title_buffer = RwSignal::new(String::new());
    let title_error = RwSignal::new(None::<String>);
    let subject_buffer = RwSignal::new(String::new());
    let subject_error = RwSignal::new(None::<String>);
    let date_buffer = RwSignal::new(String::new());
    let date_error = RwSignal::new(None::<String>);
    let time_buffer = RwSignal::new(String::new());
    let time_error = RwSignal::new(None::<String>);
    let offset_buffer = RwSignal::new(String::new());
    let offset_error = RwSignal::new(None::<String>);
    let location_buffer = RwSignal::new(String::new());
    let location_error = RwSignal::new(None::<String>);
    let latitude_buffer = RwSignal::new(String::new());
    let latitude_error = RwSignal::new(None::<String>);
    let longitude_buffer = RwSignal::new(String::new());
    let longitude_error = RwSignal::new(None::<String>);
    let disabled = Signal::derive(move || {
        model.get().chart_editor.is_some_and(|editor| {
            matches!(
                editor.state,
                ChartEditorState::Saving | ChartEditorState::Conflict
            )
        })
    });
    let factual_disabled = Signal::derive(move || {
        model.get().chart_editor.is_some_and(|editor| {
            matches!(
                editor.state,
                ChartEditorState::Saving | ChartEditorState::Conflict
            ) || !editor.factual_mutations_enabled
        })
    });
    let editor_disabled_reason = Signal::derive(move || {
        model
            .get()
            .chart_editor
            .and_then(|editor| match editor.state {
                ChartEditorState::Saving => Some("The chart editor is already saving".to_owned()),
                ChartEditorState::Conflict => Some(
                    "Cancel and reopen the chart to adopt the refreshed component heads".to_owned(),
                ),
                ChartEditorState::Clean | ChartEditorState::Dirty => None,
            })
    });
    let factual_disabled_reason = Signal::derive(move || {
        model
            .get()
            .chart_editor
            .and_then(|editor| match editor.state {
                ChartEditorState::Saving => Some("The chart editor is already saving".to_owned()),
                ChartEditorState::Conflict => Some(
                    "Cancel and reopen the chart to adopt the refreshed component heads".to_owned(),
                ),
                ChartEditorState::Clean | ChartEditorState::Dirty => {
                    editor.factual_mutations_disabled_reason
                }
            })
    });

    view! {
        <section class="inspector-section chart-authoring" aria-labelledby="chart-authoring-title">
            <div class="draft-heading">
                <div>
                    <p class="section-kicker">"CHART AUTHORING"</p>
                    <h3 id="chart-authoring-title">"Chart editor"</h3>
                </div>
                <ActionControl
                    address=ControlAddress::new(ControlId::CHART_NEW).to_string()
                    label="New chart".into()
                    disabled=Signal::derive(move || !model.get().availability(AppAction::BeginNewChart).is_enabled())
                    disabled_reason=Signal::derive(move || model.get().availability(AppAction::BeginNewChart)
                        .disabled_reason().map(str::to_owned))
                    on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                        AppIntent::BeginNewChart,
                        ActionSource::Human,
                        Some(ControlAddress::new(ControlId::CHART_NEW)),
                    ))
                />
                {move || {
                    let snapshot = model.get();
                    if snapshot.chart_editor.is_some() {
                        return None;
                    }
                    let instance_id = snapshot.inspector.active_chart.and_then(|chart| {
                        matches!(chart.persistence, mirabile_app::ChartPersistence::Saved { .. })
                            .then_some(chart.instance_id)
                    })?;
                    let address = ControlAddress::qualified(
                        ControlId::CHART_EDIT_SAVED,
                        [("instance", instance_id.to_string())],
                    )
                    .expect("saved chart edit address");
                    let origin = address.clone();
                    Some(view! {
                        <ActionControl
                            address=address.to_string()
                            label="Edit active chart".into()
                            disabled=Signal::derive(|| false)
                            on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                AppIntent::BeginSavedChartEdit { instance_id },
                                ActionSource::Human,
                                Some(origin.clone()),
                            ))
                        />
                    })
                }}
            </div>

            {move || model.get().chart_editor.map(|editor| {
                let validation = editor.validation.clone();
                view! {
                    <div class="chart-editor-fields">
                        <p class="revision-line">{format!("{:?} · {} validation issue(s)", editor.state, validation.len())}</p>
                        {editor.factual_mutations_disabled_reason.map(|reason| view! {
                            <p class="notice warning" role="status">{reason}</p>
                        })}
                        {(!editor.conflicts.is_empty()).then(|| view! {
                            <ul class="validation-list" role="status">
                                {editor.conflicts.into_iter().map(|conflict| view! {
                                    <li>{format!(
                                        "{:?} conflict: expected {}, current {}",
                                        conflict.component,
                                        conflict.expected_revision,
                                        conflict.actual_revision,
                                    )}</li>
                                }).collect_view()}
                            </ul>
                        })}
                        <BufferedTextField
                            address=ControlAddress::new(ControlId::CHART_TITLE).to_string()
                            label="Title".into()
                            authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| editor.fields.title))
                            disabled
                            disabled_reason=editor_disabled_reason
                            buffer=title_buffer
                            error=title_error
                            parser=Callback::new(Ok::<String, String>)
                            on_commit=Callback::new(move |value: String| dispatch_mutation(
                                dispatcher,
                                ControlId::CHART_TITLE,
                                ChartMutation::SetTitle(value),
                            ))
                        />
                        <BufferedTextField
                            address=ControlAddress::new(ControlId::CHART_SUBJECT_NAME).to_string()
                            label="Subject name (optional)".into()
                            authoritative=Signal::derive(move || model.get().chart_editor.and_then(|editor| editor.fields.subject_name).unwrap_or_default())
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            buffer=subject_buffer
                            error=subject_error
                            parser=Callback::new(Ok::<String, String>)
                            on_commit=Callback::new(move |value: String| dispatch_mutation(
                                dispatcher,
                                ControlId::CHART_SUBJECT_NAME,
                                ChartMutation::SetSubjectName((!value.trim().is_empty()).then_some(value)),
                            ))
                        />
                        <EnumSelect
                            address=ControlAddress::new(ControlId::CHART_EVENT_KIND).to_string()
                            label="Event kind".into()
                            value=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| event_kind_value(&editor.fields.event_kind).into()))
                            options=Signal::derive(event_kind_options)
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            on_change=Callback::new(move |value: String| {
                                if let Some(kind) = parse_event_kind(&value) {
                                    dispatch_mutation(dispatcher, ControlId::CHART_EVENT_KIND, ChartMutation::SetEventKind(kind));
                                }
                            })
                        />
                        <BufferedDateField
                            address=ControlAddress::new(ControlId::CHART_CIVIL_DATE).to_string()
                            label="Civil date".into()
                            authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| format_date(editor.fields.civil_date)))
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            buffer=date_buffer
                            error=date_error
                            parser=Callback::new(|value: String| parse_date(&value).map(format_date))
                            on_commit=Callback::new(move |value: String| {
                                if let Ok(date) = parse_date(&value) {
                                    dispatch_mutation(dispatcher, ControlId::CHART_CIVIL_DATE, ChartMutation::SetCivilDate(date));
                                }
                            })
                        />
                        <BufferedTimeField
                            address=ControlAddress::new(ControlId::CHART_CIVIL_TIME).to_string()
                            label="Civil time".into()
                            authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| format_time(editor.fields.civil_time)))
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            buffer=time_buffer
                            error=time_error
                            parser=Callback::new(|value: String| parse_time(&value).map(format_time))
                            on_commit=Callback::new(move |value: String| {
                                if let Ok(time) = parse_time(&value) {
                                    dispatch_mutation(dispatcher, ControlId::CHART_CIVIL_TIME, ChartMutation::SetCivilTime(time));
                                }
                            })
                        />
                        <EnumSelect
                            address=ControlAddress::new(ControlId::CHART_TIMEZONE).to_string()
                            label="Timezone mode".into()
                            value=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| match editor.fields.timezone {
                                ChartTimezone::UniversalTime => "universal_time".into(),
                                ChartTimezone::FixedOffset(_) => "fixed_offset".into(),
                            }))
                            options=Signal::derive(move || timezone_options(&model.get()))
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            on_change=Callback::new(move |value: String| match value.as_str() {
                                "universal_time" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::UniversalTime)),
                                "fixed_offset" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::FixedOffset(Offset::UTC))),
                                _ => {}
                            })
                        />
                        <Show when=move || model.get().chart_editor.is_some_and(|editor| matches!(editor.fields.timezone, ChartTimezone::FixedOffset(_)))>
                            <BufferedNumberField
                                address=ControlAddress::new(ControlId::CHART_FIXED_OFFSET).to_string()
                                label="UTC offset minutes".into()
                                authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| match editor.fields.timezone {
                                    ChartTimezone::FixedOffset(offset) => (offset.seconds() / 60).to_string(),
                                    ChartTimezone::UniversalTime => "0".into(),
                                }))
                                disabled=factual_disabled
                                disabled_reason=factual_disabled_reason
                                buffer=offset_buffer
                                error=offset_error
                                parser=Callback::new(|value: String| parse_offset(&value).map(|offset| (offset.seconds() / 60).to_string()))
                                on_commit=Callback::new(move |value: String| {
                                    if let Ok(offset) = parse_offset(&value) {
                                        dispatch_mutation(dispatcher, ControlId::CHART_FIXED_OFFSET, ChartMutation::SetTimezone(ChartTimezone::FixedOffset(offset)));
                                    }
                                })
                            />
                        </Show>
                        <Toggle
                            address=ControlAddress::new(ControlId::CHART_LOCATION_ENABLED).to_string()
                            label="Use manual location".into()
                            checked=Signal::derive(move || model.get().chart_editor.is_some_and(|editor| editor.fields.location.enabled))
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            on_change=Callback::new(move |enabled| dispatch_mutation(
                                dispatcher,
                                ControlId::CHART_LOCATION_ENABLED,
                                ChartMutation::SetLocationEnabled(enabled),
                            ))
                        />
                        <Show when=move || model.get().chart_editor.is_some_and(|editor| editor.fields.location.enabled)>
                            <BufferedTextField
                                address=ControlAddress::new(ControlId::CHART_LOCATION_NAME).to_string()
                                label="Location name".into()
                                authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| editor.fields.location.display_name))
                                disabled=factual_disabled
                                disabled_reason=factual_disabled_reason
                                buffer=location_buffer
                                error=location_error
                                parser=Callback::new(Ok::<String, String>)
                                on_commit=Callback::new(move |value: String| dispatch_mutation(dispatcher, ControlId::CHART_LOCATION_NAME, ChartMutation::SetLocationName(value)))
                            />
                            <BufferedNumberField
                                address=ControlAddress::new(ControlId::CHART_LATITUDE).to_string()
                                label="Latitude".into()
                                authoritative=Signal::derive(move || model.get().chart_editor.and_then(|editor| editor.fields.location.latitude).map_or_else(String::new, |value| value.degrees().to_string()))
                                disabled=factual_disabled
                                disabled_reason=factual_disabled_reason
                                buffer=latitude_buffer
                                error=latitude_error
                                parser=Callback::new(|value: String| parse_latitude(&value).map(|value| value.degrees().to_string()))
                                on_commit=Callback::new(move |value: String| {
                                    if let Ok(latitude) = parse_latitude(&value) {
                                        dispatch_mutation(dispatcher, ControlId::CHART_LATITUDE, ChartMutation::SetLatitude(Some(latitude)));
                                    }
                                })
                            />
                            <BufferedNumberField
                                address=ControlAddress::new(ControlId::CHART_LONGITUDE).to_string()
                                label="Longitude".into()
                                authoritative=Signal::derive(move || model.get().chart_editor.and_then(|editor| editor.fields.location.longitude).map_or_else(String::new, |value| value.degrees().to_string()))
                                disabled=factual_disabled
                                disabled_reason=factual_disabled_reason
                                buffer=longitude_buffer
                                error=longitude_error
                                parser=Callback::new(|value: String| parse_longitude(&value).map(|value| value.degrees().to_string()))
                                on_commit=Callback::new(move |value: String| {
                                    if let Ok(longitude) = parse_longitude(&value) {
                                        dispatch_mutation(dispatcher, ControlId::CHART_LONGITUDE, ChartMutation::SetLongitude(Some(longitude)));
                                    }
                                })
                            />
                        </Show>
                        <EnumSelect
                            address=ControlAddress::new(ControlId::CHART_ZODIAC).to_string()
                            label="Zodiac".into()
                            value=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| match editor.fields.zodiac { ZodiacSpec::Tropical => "tropical".into(), ZodiacSpec::Sidereal { .. } => "sidereal".into() }))
                            options=Signal::derive(move || zodiac_options(&model.get()))
                            disabled
                            disabled_reason=editor_disabled_reason
                            on_change=Callback::new(move |value: String| {
                                if value == "tropical" {
                                    dispatch_mutation(dispatcher, ControlId::CHART_ZODIAC, ChartMutation::SetZodiac(ZodiacSpec::Tropical));
                                }
                            })
                        />
                        <EnumSelect
                            address=ControlAddress::new(ControlId::CHART_HOUSES).to_string()
                            label="House system".into()
                            value=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| house_value(editor.fields.houses).into()))
                            options=Signal::derive(move || house_options(&model.get()))
                            disabled
                            disabled_reason=editor_disabled_reason
                            on_change=Callback::new(move |value: String| {
                                if let Some(houses) = parse_houses(&value) {
                                    dispatch_mutation(dispatcher, ControlId::CHART_HOUSES, ChartMutation::SetHouseSystem(houses));
                                }
                            })
                        />
                        <EnumSelect
                            address=ControlAddress::new(ControlId::CHART_COORDINATES).to_string()
                            label="Coordinate system".into()
                            value=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| coordinate_value(editor.fields.coordinates).into()))
                            options=Signal::derive(move || coordinate_options(&model.get()))
                            disabled
                            disabled_reason=editor_disabled_reason
                            on_change=Callback::new(move |value: String| {
                                if let Some(coordinates) = parse_coordinates(&value) {
                                    dispatch_mutation(dispatcher, ControlId::CHART_COORDINATES, ChartMutation::SetCoordinateSystem(coordinates));
                                }
                            })
                        />
                        {(!validation.is_empty()).then(|| view! {
                            <ul class="validation-list" role="status">
                                {validation.into_iter().map(|issue| view! { <li>{format!("{}: {}", issue.field, issue.message)}</li> }).collect_view()}
                            </ul>
                        })}
                        <div class="draft-actions">
                            <ActionControl
                                address=ControlAddress::new(ControlId::CHART_EDITOR_SAVE).to_string()
                                label="Save chart".into()
                                disabled=Signal::derive(move || !model.get().availability(AppAction::SaveChartEditor).is_enabled())
                                disabled_reason=Signal::derive(move || model.get().availability(AppAction::SaveChartEditor)
                                    .disabled_reason().map(str::to_owned))
                                pending=Signal::derive(move || chart_save_pending(&model.get()))
                                on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                    AppIntent::SaveChartEditor,
                                    ActionSource::Human,
                                    Some(ControlAddress::new(ControlId::CHART_EDITOR_SAVE)),
                                ))
                            />
                            <ActionControl
                                address=ControlAddress::new(ControlId::CHART_EDITOR_CANCEL).to_string()
                                label="Cancel".into()
                                disabled=Signal::derive(move || !model.get().availability(AppAction::CancelChartEditor).is_enabled())
                                disabled_reason=Signal::derive(move || model.get().availability(AppAction::CancelChartEditor)
                                    .disabled_reason().map(str::to_owned))
                                on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                    AppIntent::CancelChartEditor,
                                    ActionSource::Human,
                                    Some(ControlAddress::new(ControlId::CHART_EDITOR_CANCEL)),
                                ))
                            />
                        </div>
                    </div>
                }
            })}
        </section>
    }
}

fn dispatch_mutation(
    dispatcher: WorkbenchCoordinator,
    control: ControlId,
    mutation: ChartMutation,
) {
    dispatcher.dispatch_from(
        AppIntent::ApplyChartMutation(mutation),
        ActionSource::Human,
        Some(ControlAddress::new(control)),
    );
}

fn option(
    value: &str,
    label: &str,
    enabled: bool,
    reason: Option<String>,
) -> ControlOptionDescriptor {
    ControlOptionDescriptor {
        value: value.into(),
        label: label.into(),
        enabled,
        disabled_reason: reason,
    }
}

fn event_kind_options() -> Vec<ControlOptionDescriptor> {
    [
        ("birth", "Birth"),
        ("event", "Event"),
        ("ingress", "Ingress"),
        ("question", "Question"),
    ]
    .into_iter()
    .map(|(value, label)| option(value, label, true, None))
    .collect()
}

fn event_kind_value(value: &EventKind) -> &'static str {
    match value {
        EventKind::Birth => "birth",
        EventKind::Event => "event",
        EventKind::Ingress => "ingress",
        EventKind::Question => "question",
        EventKind::Other(_) => "other",
    }
}

fn parse_event_kind(value: &str) -> Option<EventKind> {
    match value {
        "birth" => Some(EventKind::Birth),
        "event" => Some(EventKind::Event),
        "ingress" => Some(EventKind::Ingress),
        "question" => Some(EventKind::Question),
        _ => None,
    }
}

fn zodiac_options(model: &AppReadModel) -> Vec<ControlOptionDescriptor> {
    model
        .authoring
        .zodiac_modes
        .iter()
        .map(|choice| {
            option(
                match choice.value {
                    ZodiacMode::Tropical => "tropical",
                    ZodiacMode::Sidereal => "sidereal",
                },
                &choice.label,
                choice.enabled,
                choice.disabled_reason.clone(),
            )
        })
        .collect()
}

fn house_options(model: &AppReadModel) -> Vec<ControlOptionDescriptor> {
    model
        .authoring
        .house_systems
        .iter()
        .map(|choice| {
            option(
                house_value(choice.value),
                &choice.label,
                choice.enabled,
                choice.disabled_reason.clone(),
            )
        })
        .collect()
}

const fn house_value(value: HouseSystem) -> &'static str {
    match value {
        HouseSystem::NoHouses => "no_houses",
        HouseSystem::Equal => "equal",
        HouseSystem::Placidus => "placidus",
        HouseSystem::WholeSign => "whole_sign",
    }
}

fn parse_houses(value: &str) -> Option<HouseSystem> {
    match value {
        "no_houses" => Some(HouseSystem::NoHouses),
        "equal" => Some(HouseSystem::Equal),
        "placidus" => Some(HouseSystem::Placidus),
        "whole_sign" => Some(HouseSystem::WholeSign),
        _ => None,
    }
}

fn coordinate_options(model: &AppReadModel) -> Vec<ControlOptionDescriptor> {
    model
        .authoring
        .coordinate_systems
        .iter()
        .map(|choice| {
            option(
                coordinate_value(choice.value),
                &choice.label,
                choice.enabled,
                choice.disabled_reason.clone(),
            )
        })
        .collect()
}

const fn coordinate_value(value: CoordinateSystem) -> &'static str {
    match value {
        CoordinateSystem::Geocentric => "geocentric",
        CoordinateSystem::Topocentric => "topocentric",
        CoordinateSystem::Heliocentric => "heliocentric",
    }
}

fn parse_coordinates(value: &str) -> Option<CoordinateSystem> {
    match value {
        "geocentric" => Some(CoordinateSystem::Geocentric),
        "topocentric" => Some(CoordinateSystem::Topocentric),
        "heliocentric" => Some(CoordinateSystem::Heliocentric),
        _ => None,
    }
}

fn timezone_options(model: &AppReadModel) -> Vec<ControlOptionDescriptor> {
    model
        .authoring
        .timezone_modes
        .iter()
        .map(|choice| {
            option(
                match choice.value {
                    TimezoneAuthoringMode::UniversalTime => "universal_time",
                    TimezoneAuthoringMode::FixedOffset => "fixed_offset",
                    TimezoneAuthoringMode::NamedZone => "named_zone",
                    TimezoneAuthoringMode::LocalMeanTime => "local_mean_time",
                    TimezoneAuthoringMode::LocalApparentTime => "local_apparent_time",
                },
                &choice.label,
                choice.enabled,
                choice.disabled_reason.clone(),
            )
        })
        .collect()
}

fn format_date(value: CivilDate) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year(),
        value.month(),
        value.day()
    )
}

fn parse_date(value: &str) -> Result<CivilDate, String> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .ok_or("Enter YYYY-MM-DD")?
        .parse()
        .map_err(|_| "Enter a valid year")?;
    let month = parts
        .next()
        .ok_or("Enter YYYY-MM-DD")?
        .parse()
        .map_err(|_| "Enter a valid month")?;
    let day = parts
        .next()
        .ok_or("Enter YYYY-MM-DD")?
        .parse()
        .map_err(|_| "Enter a valid day")?;
    if parts.next().is_some() {
        return Err("Enter YYYY-MM-DD".into());
    }
    CivilDate::new(year, month, day).map_err(|error| error.to_string())
}

fn format_time(value: CivilTime) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        value.hour(),
        value.minute(),
        value.second()
    )
}

fn parse_time(value: &str) -> Result<CivilTime, String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return Err("Enter HH:MM or HH:MM:SS".into());
    }
    let hour = parts[0].parse().map_err(|_| "Enter a valid hour")?;
    let minute = parts[1].parse().map_err(|_| "Enter a valid minute")?;
    let second = parts.get(2).map_or(Ok(0), |value| {
        value.parse().map_err(|_| "Enter a valid second")
    })?;
    CivilTime::new(hour, minute, second).map_err(|error| error.to_string())
}

fn parse_offset(value: &str) -> Result<Offset, String> {
    let minutes = value
        .parse::<i32>()
        .map_err(|_| "Enter whole offset minutes")?;
    let seconds = minutes.checked_mul(60).ok_or("Offset is too large")?;
    Offset::from_seconds(seconds).map_err(|error| error.to_string())
}

fn parse_latitude(value: &str) -> Result<Latitude, String> {
    Latitude::from_degrees(
        value
            .parse()
            .map_err(|_| "Enter a latitude from -90 through 90")?,
    )
    .map_err(|error| error.to_string())
}

fn parse_longitude(value: &str) -> Result<Longitude, String> {
    Longitude::from_degrees(
        value
            .parse()
            .map_err(|_| "Enter a longitude from -180 through 180")?,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_and_location_parsers_reject_invalid_buffers() {
        assert_eq!(parse_date("2026-08-24").expect("date").day(), 24);
        assert!(parse_date("2026-02-30").is_err());
        assert_eq!(parse_time("23:59").expect("time").second(), 0);
        assert!(parse_latitude("90.1").is_err());
        assert!(parse_longitude("-181").is_err());
        assert_eq!(parse_offset("-300").expect("offset").seconds(), -18_000);
    }
}

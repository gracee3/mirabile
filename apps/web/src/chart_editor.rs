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
        ActionControl, BufferedDateField, BufferedInputKind, BufferedNumberField,
        BufferedTextField, BufferedTimeField, EnumSelect, Toggle, ValidatedField,
        chart_save_pending, invalid_buffer_registry,
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
    let record_title_buffer = RwSignal::new(String::new());
    let record_title_error = RwSignal::new(None::<String>);
    let subject_buffer = RwSignal::new(String::new());
    let subject_error = RwSignal::new(None::<String>);
    let date_buffer = RwSignal::new(String::new());
    let date_error = RwSignal::new(None::<String>);
    let time_buffer = RwSignal::new(String::new());
    let time_error = RwSignal::new(None::<String>);
    let offset_buffer = RwSignal::new(String::new());
    let offset_error = RwSignal::new(None::<String>);
    let named_zone_buffer = RwSignal::new(String::new());
    let named_zone_error = RwSignal::new(None::<String>);
    let location_buffer = RwSignal::new(String::new());
    let location_error = RwSignal::new(None::<String>);
    let latitude_buffer = RwSignal::new(String::new());
    let latitude_error = RwSignal::new(None::<String>);
    let longitude_buffer = RwSignal::new(String::new());
    let longitude_error = RwSignal::new(None::<String>);
    let invalid_buffers = invalid_buffer_registry();
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
                            authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| editor.fields.definition_metadata.title))
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
                        <label>"ChartDefinition description"<textarea
                            prop:value=editor.fields.definition_metadata.description.clone().unwrap_or_default()
                            data-mirabile-control=ControlId::CHART_DEFINITION_DESCRIPTION.to_string()
                            data-mirabile-address=ControlAddress::new(ControlId::CHART_DEFINITION_DESCRIPTION).to_string()
                            data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string()
                            disabled=disabled
                            on:change=move |event| { let value=event_target_value(&event); dispatch_mutation(dispatcher, ControlId::CHART_DEFINITION_DESCRIPTION, ChartMutation::SetDefinitionDescription((!value.trim().is_empty()).then_some(value))); }
                        /></label>
                        <label>"ChartDefinition tags"<input type="text"
                            prop:value=editor.fields.definition_metadata.tags.join(", ")
                            data-mirabile-control=ControlId::CHART_DEFINITION_TAGS.to_string()
                            data-mirabile-address=ControlAddress::new(ControlId::CHART_DEFINITION_TAGS).to_string()
                            data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string()
                            disabled=disabled
                            on:change=move |event| dispatch_mutation(dispatcher, ControlId::CHART_DEFINITION_TAGS, ChartMutation::SetDefinitionTags(parse_tags(&event_target_value(&event))))
                        /></label>
                        <BufferedTextField
                            address=ControlAddress::new(ControlId::CHART_RECORD_TITLE).to_string()
                            label="ChartRecord title".into()
                            authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| editor.fields.record_metadata.title))
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            buffer=record_title_buffer
                            error=record_title_error
                            parser=Callback::new(Ok::<String, String>)
                            on_commit=Callback::new(move |value: String| dispatch_mutation(
                                dispatcher,
                                ControlId::CHART_RECORD_TITLE,
                                ChartMutation::SetRecordTitle(value),
                            ))
                        />
                        <label>"ChartRecord description"<textarea
                            prop:value=editor.fields.record_metadata.description.clone().unwrap_or_default()
                            data-mirabile-control=ControlId::CHART_RECORD_DESCRIPTION.to_string()
                            data-mirabile-address=ControlAddress::new(ControlId::CHART_RECORD_DESCRIPTION).to_string()
                            data-mirabile-kind="text" data-mirabile-enabled=move || (!factual_disabled.get()).to_string()
                            data-mirabile-disabled-reason=move || factual_disabled_reason.get()
                            disabled=factual_disabled
                            on:change=move |event| { let value=event_target_value(&event); dispatch_mutation(dispatcher, ControlId::CHART_RECORD_DESCRIPTION, ChartMutation::SetRecordDescription((!value.trim().is_empty()).then_some(value))); }
                        /></label>
                        <label>"ChartRecord tags"<input type="text"
                            prop:value=editor.fields.record_metadata.tags.join(", ")
                            data-mirabile-control=ControlId::CHART_RECORD_TAGS.to_string()
                            data-mirabile-address=ControlAddress::new(ControlId::CHART_RECORD_TAGS).to_string()
                            data-mirabile-kind="text" data-mirabile-enabled=move || (!factual_disabled.get()).to_string()
                            data-mirabile-disabled-reason=move || factual_disabled_reason.get()
                            disabled=factual_disabled
                            on:change=move |event| dispatch_mutation(dispatcher, ControlId::CHART_RECORD_TAGS, ChartMutation::SetRecordTags(parse_tags(&event_target_value(&event))))
                        /></label>
                        <small class="revision-line">{component_identity("ChartDefinition", &editor.fields.definition_metadata)}</small>
                        <small class="revision-line">{component_identity("ChartRecord", &editor.fields.record_metadata)}</small>
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
                        <ChartRecordDetails record=editor.fields.record.clone() disabled=factual_disabled disabled_reason=factual_disabled_reason dispatcher />
                        <ChartNestedFacts record=editor.fields.record.clone() notes=editor.notes.clone() life_events=editor.life_events.clone() disabled=factual_disabled dispatcher />
                        <fieldset class="payload-fields"><legend>"Complete calculation semantics"</legend>
                            <label>"Lunar node"<select prop:value=move || model.get().chart_editor.map_or_else(String::new, |editor| format!("{:?}", editor.fields.calculation.lunar_node).to_lowercase()) data-mirabile-control=ControlId::CHART_LUNAR_NODE.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_LUNAR_NODE).to_string() data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| if let Some(editor)=model.get().chart_editor { let mut calculation=editor.fields.calculation; calculation.lunar_node=if event_target_value(&event) == "mean" { mirabile_app::LunarNodeType::Mean } else { mirabile_app::LunarNodeType::True }; dispatch_mutation(dispatcher, ControlId::CHART_LUNAR_NODE, ChartMutation::SetCalculation(calculation)); }><option value="mean">"Mean"</option><option value="true">"True"</option></select></label>
                            <label>"Black Moon"<select prop:value=move || model.get().chart_editor.map_or_else(String::new, |editor| format!("{:?}", editor.fields.calculation.black_moon).to_lowercase()) data-mirabile-control=ControlId::CHART_BLACK_MOON.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_BLACK_MOON).to_string() data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| if let Some(editor)=model.get().chart_editor { let mut calculation=editor.fields.calculation; calculation.black_moon=if event_target_value(&event) == "osculating" { mirabile_app::BlackMoonType::Osculating } else { mirabile_app::BlackMoonType::Mean }; dispatch_mutation(dispatcher, ControlId::CHART_BLACK_MOON, ChartMutation::SetCalculation(calculation)); }><option value="mean">"Mean"</option><option value="osculating">"Osculating"</option></select></label>
                            <label>"Fortune formula"<select prop:value=move || model.get().chart_editor.map_or_else(String::new, |editor| match editor.fields.calculation.fortune_formula { mirabile_app::FortuneFormula::DayNight => "day-night".into(), mirabile_app::FortuneFormula::AlwaysAscendantPlusMoonMinusSun => "always".into() }) data-mirabile-control=ControlId::CHART_FORTUNE_FORMULA.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_FORTUNE_FORMULA).to_string() data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| if let Some(editor)=model.get().chart_editor { let mut calculation=editor.fields.calculation; calculation.fortune_formula=if event_target_value(&event) == "always" { mirabile_app::FortuneFormula::AlwaysAscendantPlusMoonMinusSun } else { mirabile_app::FortuneFormula::DayNight }; dispatch_mutation(dispatcher, ControlId::CHART_FORTUNE_FORMULA, ChartMutation::SetCalculation(calculation)); }><option value="day-night">"Day/night"</option><option value="always">"Always Asc + Moon - Sun"</option></select></label>
                            {[("aberration", "Aberration"), ("light-time", "Light time"), ("nutation", "Nutation")].into_iter().map(|(field, label)| view! { <label class="check-field"><input type="checkbox" prop:checked=move || model.get().chart_editor.is_some_and(|editor| match field { "aberration" => editor.fields.calculation.corrections.aberration, "light-time" => editor.fields.calculation.corrections.light_time, _ => editor.fields.calculation.corrections.nutation }) data-mirabile-control=ControlId::CHART_CORRECTION.to_string() data-mirabile-address=ControlAddress::qualified(ControlId::CHART_CORRECTION, [("field", field)]).expect("correction address").to_string() data-mirabile-kind="checkbox" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| if let Some(editor)=model.get().chart_editor { let mut calculation=editor.fields.calculation; match field { "aberration" => calculation.corrections.aberration=event_target_checked(&event), "light-time" => calculation.corrections.light_time=event_target_checked(&event), _ => calculation.corrections.nutation=event_target_checked(&event) } dispatch_mutation(dispatcher, ControlId::CHART_CORRECTION, ChartMutation::SetCalculation(calculation)); } /><span>{label}</span></label> }).collect_view()}
                        </fieldset>
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
                                ChartTimezone::NamedZone(_) => "named_zone".into(),
                                ChartTimezone::LocalMeanTime => "local_mean_time".into(),
                                ChartTimezone::LocalApparentTime => "local_apparent_time".into(),
                                ChartTimezone::Unknown => "unknown".into(),
                            }))
                            options=Signal::derive(move || timezone_options(&model.get()))
                            disabled=factual_disabled
                            disabled_reason=factual_disabled_reason
                            on_change=Callback::new(move |value: String| match value.as_str() {
                                "universal_time" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::UniversalTime)),
                                "fixed_offset" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::FixedOffset(Offset::UTC))),
                                "named_zone" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::NamedZone("Etc/UTC".into()))),
                                "local_mean_time" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::LocalMeanTime)),
                                "local_apparent_time" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::LocalApparentTime)),
                                "unknown" => dispatch_mutation(dispatcher, ControlId::CHART_TIMEZONE, ChartMutation::SetTimezone(ChartTimezone::Unknown)),
                                _ => {}
                            })
                        />
                        <Show when=move || model.get().chart_editor.is_some_and(|editor| matches!(editor.fields.timezone, ChartTimezone::FixedOffset(_)))>
                            <BufferedNumberField
                                address=ControlAddress::new(ControlId::CHART_FIXED_OFFSET).to_string()
                                label="UTC offset minutes".into()
                                authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| match editor.fields.timezone {
                                    ChartTimezone::FixedOffset(offset) => (offset.seconds() / 60).to_string(),
                                    _ => "0".into(),
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
                        <Show when=move || model.get().chart_editor.is_some_and(|editor| matches!(editor.fields.timezone, ChartTimezone::NamedZone(_)))>
                            <BufferedTextField
                                address=ControlAddress::new(ControlId::CHART_NAMED_ZONE).to_string()
                                label="IANA timezone name".into()
                                authoritative=Signal::derive(move || model.get().chart_editor.map_or_else(String::new, |editor| match editor.fields.timezone {
                                    ChartTimezone::NamedZone(name) => name,
                                    _ => String::new(),
                                }))
                                disabled=factual_disabled
                                disabled_reason=factual_disabled_reason
                                buffer=named_zone_buffer
                                error=named_zone_error
                                parser=Callback::new(|value: String| if value.trim().is_empty() { Err("Timezone name is required".into()) } else { Ok(value) })
                                on_commit=Callback::new(move |value: String| dispatch_mutation(dispatcher, ControlId::CHART_NAMED_ZONE, ChartMutation::SetTimezone(ChartTimezone::NamedZone(value))))
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
                                disabled=Signal::derive(move || !model.get().availability(AppAction::SaveChartEditor).is_enabled()
                                    || invalid_buffers.has_prefix("chart."))
                                disabled_reason=Signal::derive(move || if invalid_buffers.has_prefix("chart.") {
                                    Some("Correct invalid local chart values before saving".to_owned())
                                } else {
                                    model.get().availability(AppAction::SaveChartEditor)
                                        .disabled_reason().map(str::to_owned)
                                })
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

#[component]
fn ChartRecordDetails(
    record: mirabile_app::ChartRecord,
    disabled: Signal<bool>,
    disabled_reason: Signal<Option<String>>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let custom_base = record.clone();
    let pronouns_base = record.clone();
    let calendar_base = record.clone();
    let calendar_identifier_base = record.clone();
    let disambiguation_base = record.clone();
    let country_base = record.clone();
    let atlas_provider_base = record.clone();
    let atlas_record_base = record.clone();
    let atlas_version_base = record.clone();
    let source_description_base = record.clone();
    let source_type_base = record.clone();
    let recorded_by_base = record.clone();
    let custom = match &record.event_kind {
        EventKind::Other(value) => value.clone(),
        _ => String::new(),
    };
    let pronouns = record
        .subject
        .as_ref()
        .and_then(|value| value.pronouns.clone())
        .unwrap_or_default();
    let country = record
        .location
        .as_ref()
        .and_then(|value| value.country_region.clone())
        .unwrap_or_default();
    let atlas = record
        .location
        .as_ref()
        .and_then(|value| value.atlas_provenance.clone());
    view! { <fieldset class="payload-fields chart-record-details"><legend>"Complete factual provenance"</legend>
        <label>"Custom event label (sets Other when nonempty)"<input type="text" prop:value=custom data-mirabile-control=ControlId::CHART_CUSTOM_EVENT_KIND.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_CUSTOM_EVENT_KIND).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() data-mirabile-disabled-reason=move || disabled_reason.get() disabled=disabled on:change=move |event| { let value=event_target_value(&event); if !value.trim().is_empty() { let mut next=custom_base.clone(); next.event_kind=EventKind::Other(value); dispatch_record(dispatcher, ControlId::CHART_CUSTOM_EVENT_KIND, next); } } /></label>
        <label>"Pronouns"<input type="text" prop:value=pronouns data-mirabile-control=ControlId::CHART_SUBJECT_PRONOUNS.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_SUBJECT_PRONOUNS).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let value=event_target_value(&event); let mut next=pronouns_base.clone(); if let Some(subject)=&mut next.subject { subject.pronouns=(!value.trim().is_empty()).then_some(value); dispatch_record(dispatcher, ControlId::CHART_SUBJECT_PRONOUNS, next); } } /></label>
        <label>"Calendar"<select prop:value=calendar_key(&record.time.calendar) data-mirabile-control=ControlId::CHART_CALENDAR.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_CALENDAR).to_string() data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut next=calendar_base.clone(); next.time.calendar=match event_target_value(&event).as_str() { "julian" => mirabile_app::CalendarSpec::Julian, "historical" => mirabile_app::CalendarSpec::HistoricalTransition { identifier:"custom-transition".into() }, _ => mirabile_app::CalendarSpec::ProlepticGregorian }; dispatch_record(dispatcher, ControlId::CHART_CALENDAR, next); }><option value="gregorian">"Proleptic Gregorian"</option><option value="julian">"Julian"</option><option value="historical">"Historical transition"</option></select></label>
        {if let mirabile_app::CalendarSpec::HistoricalTransition { identifier }=&record.time.calendar { let identifier=identifier.clone(); view! {
            <ValidatedField address=ControlAddress::qualified(ControlId::CHART_CALENDAR, [("field", "identifier")]).expect("calendar identifier address").to_string() label="Calendar transition identifier".to_owned() kind=BufferedInputKind::Text
                authoritative=Signal::derive(move || identifier.clone()) disabled disabled_reason=disabled_reason
                parser=Callback::new(|text: String| (!text.trim().is_empty()).then(|| text.trim().to_owned()).ok_or_else(|| "Transition identifier is required".to_owned()))
                on_commit=Callback::new(move |value: String| { let mut next=calendar_identifier_base.clone(); next.time.calendar=mirabile_app::CalendarSpec::HistoricalTransition { identifier:value }; dispatch_record(dispatcher, ControlId::CHART_CALENDAR, next); }) />
        }.into_any() } else { ().into_any() }}
        <label>"Ambiguous local time"<select prop:value=disambiguation_key(record.time.disambiguation) data-mirabile-control=ControlId::CHART_DISAMBIGUATION.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_DISAMBIGUATION).to_string() data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut next=disambiguation_base.clone(); next.time.disambiguation=match event_target_value(&event).as_str() { "earlier"=>Some(mirabile_app::TimeChoice::Earlier), "later"=>Some(mirabile_app::TimeChoice::Later), _=>None }; dispatch_record(dispatcher, ControlId::CHART_DISAMBIGUATION, next); }><option value="none">"Not specified"</option><option value="earlier">"Earlier occurrence"</option><option value="later">"Later occurrence"</option></select></label>
        <label>"Country / region"<input type="text" prop:value=country data-mirabile-control=ControlId::CHART_COUNTRY_REGION.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_COUNTRY_REGION).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let value=event_target_value(&event); let mut next=country_base.clone(); if let Some(location)=&mut next.location { location.country_region=(!value.trim().is_empty()).then_some(value); dispatch_record(dispatcher, ControlId::CHART_COUNTRY_REGION, next); } } /></label>
        <label>"Atlas provider"<input type="text" prop:value=atlas.as_ref().map(|value| value.provider.clone()).unwrap_or_default() data-mirabile-control=ControlId::CHART_ATLAS_PROVIDER.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_ATLAS_PROVIDER).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut next=atlas_provider_base.clone(); update_atlas(&mut next, |atlas| atlas.provider=event_target_value(&event)); dispatch_record(dispatcher, ControlId::CHART_ATLAS_PROVIDER, next); } /></label>
        <label>"Atlas record ID"<input type="text" prop:value=atlas.as_ref().and_then(|value| value.record_id.clone()).unwrap_or_default() data-mirabile-control=ControlId::CHART_ATLAS_RECORD.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_ATLAS_RECORD).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let value=event_target_value(&event); let mut next=atlas_record_base.clone(); update_atlas(&mut next, |atlas| atlas.record_id=(!value.trim().is_empty()).then_some(value)); dispatch_record(dispatcher, ControlId::CHART_ATLAS_RECORD, next); } /></label>
        <label>"Atlas data version"<input type="text" prop:value=atlas.and_then(|value| value.data_version).unwrap_or_default() data-mirabile-control=ControlId::CHART_ATLAS_VERSION.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_ATLAS_VERSION).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let value=event_target_value(&event); let mut next=atlas_version_base.clone(); update_atlas(&mut next, |atlas| atlas.data_version=(!value.trim().is_empty()).then_some(value)); dispatch_record(dispatcher, ControlId::CHART_ATLAS_VERSION, next); } /></label>
        <label>"Source description"<input type="text" prop:value=record.source.description data-mirabile-control=ControlId::CHART_SOURCE_DESCRIPTION.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_SOURCE_DESCRIPTION).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut next=source_description_base.clone(); next.source.description=event_target_value(&event); dispatch_record(dispatcher, ControlId::CHART_SOURCE_DESCRIPTION, next); } /></label>
        <label>"Source type"<select prop:value=source_type_key(record.source.source_type) data-mirabile-control=ControlId::CHART_SOURCE_TYPE.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_SOURCE_TYPE).to_string() data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut next=source_type_base.clone(); next.source.source_type=parse_source_type(&event_target_value(&event)); dispatch_record(dispatcher, ControlId::CHART_SOURCE_TYPE, next); }>{source_type_options().into_iter().map(|(value,label)| view! { <option value=value>{label}</option> }).collect_view()}</select></label>
        <label>"Recorded by"<input type="text" prop:value=record.source.recorded_by.unwrap_or_default() data-mirabile-control=ControlId::CHART_SOURCE_RECORDED_BY.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_SOURCE_RECORDED_BY).to_string() data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let value=event_target_value(&event); let mut next=recorded_by_base.clone(); next.source.recorded_by=(!value.trim().is_empty()).then_some(value); dispatch_record(dispatcher, ControlId::CHART_SOURCE_RECORDED_BY, next); } /></label>
    </fieldset> }
}

fn dispatch_record(
    dispatcher: WorkbenchCoordinator,
    control: ControlId,
    record: mirabile_app::ChartRecord,
) {
    dispatch_mutation(
        dispatcher,
        control,
        ChartMutation::SetRecordDetails(Box::new(record)),
    );
}
fn update_atlas(
    record: &mut mirabile_app::ChartRecord,
    update: impl FnOnce(&mut mirabile_app::AtlasRef),
) {
    if let Some(location) = &mut record.location {
        let atlas = location
            .atlas_provenance
            .get_or_insert_with(|| mirabile_app::AtlasRef {
                provider: "Manual".into(),
                record_id: None,
                data_version: None,
            });
        update(atlas);
    }
}
fn calendar_key(value: &mirabile_app::CalendarSpec) -> &'static str {
    match value {
        mirabile_app::CalendarSpec::Julian => "julian",
        mirabile_app::CalendarSpec::HistoricalTransition { .. } => "historical",
        mirabile_app::CalendarSpec::ProlepticGregorian => "gregorian",
    }
}
fn disambiguation_key(value: Option<mirabile_app::TimeChoice>) -> &'static str {
    match value {
        Some(mirabile_app::TimeChoice::Earlier) => "earlier",
        Some(mirabile_app::TimeChoice::Later) => "later",
        None => "none",
    }
}
fn time_zone_key(value: &mirabile_app::TimeZoneAssertion) -> &'static str {
    match value {
        mirabile_app::TimeZoneAssertion::UniversalTime => "universal_time",
        mirabile_app::TimeZoneAssertion::FixedOffset(_) => "fixed_offset",
        mirabile_app::TimeZoneAssertion::NamedZone(_) => "named_zone",
        mirabile_app::TimeZoneAssertion::LocalMeanTime => "local_mean_time",
        mirabile_app::TimeZoneAssertion::LocalApparentTime => "local_apparent_time",
        mirabile_app::TimeZoneAssertion::Unknown => "unknown",
    }
}
fn time_zone_options() -> impl IntoView {
    view! { <>
        <option value="universal_time">"Universal Time"</option>
        <option value="fixed_offset">"Fixed offset"</option>
        <option value="named_zone">"Named zone"</option>
        <option value="local_mean_time">"Local Mean Time"</option>
        <option value="local_apparent_time">"Local Apparent Time"</option>
        <option value="unknown">"Unknown"</option>
    </> }
}
fn source_type_options() -> [(&'static str, &'static str); 7] {
    [
        ("birth-certificate", "Birth certificate"),
        ("memory", "Memory"),
        ("published", "Published"),
        ("research", "Research"),
        ("user-assertion", "User assertion"),
        ("system-clock", "System clock"),
        ("unknown", "Unknown"),
    ]
}
fn source_type_key(value: mirabile_app::SourceType) -> &'static str {
    match value {
        mirabile_app::SourceType::BirthCertificate => "birth-certificate",
        mirabile_app::SourceType::Memory => "memory",
        mirabile_app::SourceType::Published => "published",
        mirabile_app::SourceType::Research => "research",
        mirabile_app::SourceType::UserAssertion => "user-assertion",
        mirabile_app::SourceType::SystemClock => "system-clock",
        mirabile_app::SourceType::Unknown => "unknown",
    }
}
fn parse_source_type(value: &str) -> mirabile_app::SourceType {
    match value {
        "birth-certificate" => mirabile_app::SourceType::BirthCertificate,
        "memory" => mirabile_app::SourceType::Memory,
        "published" => mirabile_app::SourceType::Published,
        "research" => mirabile_app::SourceType::Research,
        "system-clock" => mirabile_app::SourceType::SystemClock,
        "unknown" => mirabile_app::SourceType::Unknown,
        _ => mirabile_app::SourceType::UserAssertion,
    }
}

#[component]
fn ChartNestedFacts(
    record: mirabile_app::ChartRecord,
    notes: Vec<mirabile_app::StableDraftItemReadModel<mirabile_app::Note>>,
    life_events: Vec<mirabile_app::LifeEventDraftReadModel>,
    disabled: Signal<bool>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let last_note = notes.last().map(|row| row.item_id);
    let last_event = life_events.last().map(|row| row.item_id);
    let event_time = record.time.clone();
    view! { <fieldset class="payload-fields nested-chart-facts"><legend>"Notes and life events"</legend>
        <h4>"Chart notes"</h4>
        {notes.into_iter().map(|row| { let item_id=row.item_id; let update=row.value.clone(); view! { <div class="builder-row"><label>"Note"<textarea prop:value=row.value.text data-mirabile-control=ControlId::CHART_NOTE_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_NOTE_FIELD, "note", item_id, Some("text")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=update.clone(); value.text=event_target_value(&event); dispatch_mutation(dispatcher, ControlId::CHART_NOTE_FIELD, ChartMutation::Notes(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label><small>{format!("Timestamp {}", row.value.created_at.unix_millis())}</small><button type="button" class="button secondary" data-mirabile-control=ControlId::CHART_NOTE_MOVE.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_NOTE_MOVE, "note", item_id, Some("end")) data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_NOTE_MOVE, ChartMutation::Notes(mirabile_app::DraftListMutation::Move { item_id, before:None }))>"Move to end"</button><button type="button" class="button danger" data-mirabile-control=ControlId::CHART_NOTE_REMOVE.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_NOTE_REMOVE, "note", item_id, None) data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_NOTE_REMOVE, ChartMutation::Notes(mirabile_app::DraftListMutation::Remove { item_id }))>"Remove note"</button></div> } }).collect_view()}
        <button type="button" class="button secondary" data-mirabile-control=ControlId::CHART_NOTE_INSERT.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_NOTE_INSERT).to_string() data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_NOTE_INSERT, ChartMutation::Notes(mirabile_app::DraftListMutation::Insert { after:last_note, value:mirabile_app::Note { text:"New note".into(), created_at:mirabile_app::Timestamp::from_unix_millis(0) } }))>"Add chart note"</button>
        <h4>"Life events"</h4>
        {life_events.into_iter().map(|row| view! { <ChartLifeEventRow row disabled dispatcher /> }).collect_view()}
        <button type="button" class="button secondary" data-mirabile-control=ControlId::CHART_LIFE_EVENT_INSERT.to_string() data-mirabile-address=ControlAddress::new(ControlId::CHART_LIFE_EVENT_INSERT).to_string() data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_INSERT, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Insert { after:last_event, value:mirabile_app::LifeEvent { title:"New life event".into(), time:event_time.clone(), location:None, notes:Vec::new() } }))>"Add life event"</button>
    </fieldset> }
}

#[component]
fn ChartLifeEventRow(
    row: mirabile_app::LifeEventDraftReadModel,
    disabled: Signal<bool>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let item_id = row.item_id;
    let title_base = row.value.clone();
    let date_base = row.value.clone();
    let time_base = row.value.clone();
    let calendar_base = row.value.clone();
    let calendar_identifier_base = row.value.clone();
    let timezone_base = row.value.clone();
    let timezone_value_base = row.value.clone();
    let disambiguation_base = row.value.clone();
    let location_toggle_base = row.value.clone();
    let last_note = row.notes.last().map(|note| note.item_id);
    let date_authoritative = format_date(row.value.time.civil_datetime.date);
    let time_authoritative = format_time(row.value.time.civil_datetime.time);
    view! { <div class="builder-row life-event-row"><label>"Title"<input type="text" prop:value=row.value.title.clone() data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("title")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=title_base.clone(); value.title=event_target_value(&event); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label>
        <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("date")) label="Date".to_owned() kind=BufferedInputKind::Date
            authoritative=Signal::derive(move || date_authoritative.clone()) disabled
            parser=Callback::new(|text: String| parse_date(&text).map(format_date).map_err(|_| "Enter a valid civil date".to_owned()))
            on_commit=Callback::new(move |text: String| if let Ok(date)=parse_date(&text) { let mut value=date_base.clone(); value.time.civil_datetime.date=date; dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
        <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("time")) label="Time".to_owned() kind=BufferedInputKind::Time
            authoritative=Signal::derive(move || time_authoritative.clone()) disabled
            parser=Callback::new(|text: String| parse_time(&text).map(format_time).map_err(|_| "Enter a valid civil time".to_owned()))
            on_commit=Callback::new(move |text: String| if let Ok(time)=parse_time(&text) { let mut value=time_base.clone(); value.time.civil_datetime.time=time; dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
        <label>"Calendar"<select prop:value=calendar_key(&row.value.time.calendar) data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("calendar")) data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=calendar_base.clone(); value.time.calendar=match event_target_value(&event).as_str() { "julian"=>mirabile_app::CalendarSpec::Julian, "historical"=>mirabile_app::CalendarSpec::HistoricalTransition { identifier:"custom-transition".into() }, _=>mirabile_app::CalendarSpec::ProlepticGregorian }; dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }><option value="gregorian">"Proleptic Gregorian"</option><option value="julian">"Julian"</option><option value="historical">"Historical transition"</option></select></label>
        {if let mirabile_app::CalendarSpec::HistoricalTransition { identifier }=&row.value.time.calendar { let identifier=identifier.clone(); view! {
            <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("calendar-identifier")) label="Transition identifier".to_owned() kind=BufferedInputKind::Text
                authoritative=Signal::derive(move || identifier.clone()) disabled
                parser=Callback::new(|text: String| (!text.trim().is_empty()).then(|| text.trim().to_owned()).ok_or_else(|| "Transition identifier is required".to_owned()))
                on_commit=Callback::new(move |next: String| { let mut value=calendar_identifier_base.clone(); value.time.calendar=mirabile_app::CalendarSpec::HistoricalTransition { identifier:next }; dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
        }.into_any() } else { ().into_any() }}
        <label>"Timezone"<select prop:value=time_zone_key(&row.value.time.zone) data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("timezone")) data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=timezone_base.clone(); value.time.zone=match event_target_value(&event).as_str() { "fixed_offset"=>mirabile_app::TimeZoneAssertion::FixedOffset(mirabile_app::Offset::UTC), "named_zone"=>mirabile_app::TimeZoneAssertion::NamedZone("Etc/UTC".into()), "local_mean_time"=>mirabile_app::TimeZoneAssertion::LocalMeanTime, "local_apparent_time"=>mirabile_app::TimeZoneAssertion::LocalApparentTime, "unknown"=>mirabile_app::TimeZoneAssertion::Unknown, _=>mirabile_app::TimeZoneAssertion::UniversalTime }; dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }>{time_zone_options()}</select></label>
        {match &row.value.time.zone {
            mirabile_app::TimeZoneAssertion::FixedOffset(offset) => { let minutes=(offset.seconds()/60).to_string(); let base=timezone_value_base.clone(); view! {
                <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("timezone-value")) label="UTC offset minutes".to_owned() kind=BufferedInputKind::Number
                    authoritative=Signal::derive(move || minutes.clone()) disabled
                    parser=Callback::new(|text: String| parse_offset(&text).map(|offset| (offset.seconds()/60).to_string()).map_err(|_| "Enter an offset from -1439 through 1439 minutes".to_owned()))
                    on_commit=Callback::new(move |text: String| if let Ok(offset)=parse_offset(&text) { let mut value=base.clone(); value.time.zone=mirabile_app::TimeZoneAssertion::FixedOffset(offset); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
            }.into_any() },
            mirabile_app::TimeZoneAssertion::NamedZone(name) => { let name=name.clone(); let base=timezone_value_base.clone(); view! {
                <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("timezone-value")) label="IANA timezone name".to_owned() kind=BufferedInputKind::Text
                    authoritative=Signal::derive(move || name.clone()) disabled
                    parser=Callback::new(|text: String| (!text.trim().is_empty()).then(|| text.trim().to_owned()).ok_or_else(|| "Timezone name is required".to_owned()))
                    on_commit=Callback::new(move |name: String| { let mut value=base.clone(); value.time.zone=mirabile_app::TimeZoneAssertion::NamedZone(name); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
            }.into_any() },
            _ => ().into_any()
        }}
        <label>"Ambiguous local time"<select prop:value=disambiguation_key(row.value.time.disambiguation) data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("disambiguation")) data-mirabile-kind="select" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=disambiguation_base.clone(); value.time.disambiguation=match event_target_value(&event).as_str() { "earlier"=>Some(mirabile_app::TimeChoice::Earlier), "later"=>Some(mirabile_app::TimeChoice::Later), _=>None }; dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }><option value="none">"Not specified"</option><option value="earlier">"Earlier occurrence"</option><option value="later">"Later occurrence"</option></select></label>
        <label class="checkbox-field"><input type="checkbox" prop:checked=row.value.location.is_some() data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("location-enabled")) data-mirabile-kind="toggle" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=location_toggle_base.clone(); value.location=event_target_checked(&event).then(|| mirabile_app::LocationAssertion { display_name:"Location".into(), country_region:None, latitude:mirabile_app::Latitude::from_degrees(0.0).expect("latitude"), longitude:mirabile_app::Longitude::from_degrees(0.0).expect("longitude"), atlas_provenance:None }); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } />"Use location"</label>
        {row.value.location.clone().map(|location| view! { <LifeEventLocation item_id event=row.value.clone() location disabled dispatcher /> })}
        {row.notes.into_iter().map(|note| { let note_id=note.item_id; let update=note.value.clone(); view! { <div class="nested-note"><label>"Life-event note"<textarea prop:value=note.value.text data-mirabile-control=ControlId::CHART_LIFE_EVENT_NOTE_FIELD.to_string() data-mirabile-address=chart_nested_item_address(ControlId::CHART_LIFE_EVENT_NOTE_FIELD, item_id, note_id, Some("text")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |event| { let mut value=update.clone(); value.text=event_target_value(&event); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_NOTE_FIELD, ChartMutation::LifeEventNotes { life_event_id:item_id, mutation:mirabile_app::DraftListMutation::Update { item_id:note_id, value } }); } /></label><button type="button" class="button danger" data-mirabile-control=ControlId::CHART_LIFE_EVENT_NOTE_REMOVE.to_string() data-mirabile-address=chart_nested_item_address(ControlId::CHART_LIFE_EVENT_NOTE_REMOVE, item_id, note_id, None) data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_NOTE_REMOVE, ChartMutation::LifeEventNotes { life_event_id:item_id, mutation:mirabile_app::DraftListMutation::Remove { item_id:note_id } })>"Remove note"</button></div> } }).collect_view()}
        <button type="button" class="button secondary" data-mirabile-control=ControlId::CHART_LIFE_EVENT_NOTE_INSERT.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_NOTE_INSERT, "life-event", item_id, None) data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_NOTE_INSERT, ChartMutation::LifeEventNotes { life_event_id:item_id, mutation:mirabile_app::DraftListMutation::Insert { after:last_note, value:mirabile_app::Note { text:"New note".into(), created_at:mirabile_app::Timestamp::from_unix_millis(0) } } })>"Add nested note"</button>
        <button type="button" class="button secondary" data-mirabile-control=ControlId::CHART_LIFE_EVENT_MOVE.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_MOVE, "life-event", item_id, Some("end")) data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_MOVE, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Move { item_id, before:None }))>"Move event to end"</button><button type="button" class="button danger" data-mirabile-control=ControlId::CHART_LIFE_EVENT_REMOVE.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_REMOVE, "life-event", item_id, None) data-mirabile-kind="action" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:click=move |_| dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_REMOVE, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Remove { item_id }))>"Remove life event"</button>
    </div> }
}

#[component]
fn LifeEventLocation(
    item_id: mirabile_app::DraftItemId,
    event: mirabile_app::LifeEvent,
    location: mirabile_app::LocationAssertion,
    disabled: Signal<bool>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let name_base = event.clone();
    let latitude_base = event.clone();
    let longitude_base = event.clone();
    let country_base = event.clone();
    let atlas_provider_base = event.clone();
    let atlas_record_base = event.clone();
    let atlas_version_base = event;
    let atlas = location.atlas_provenance.clone();
    let latitude_authoritative = location.latitude.degrees().to_string();
    let longitude_authoritative = location.longitude.degrees().to_string();
    view! { <div class="location-fields"><label>"Event location"<input type="text" prop:value=location.display_name data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("location-name")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |change| { let mut value=name_base.clone(); if let Some(location)=&mut value.location { location.display_name=event_target_value(&change); } dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label>
        <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("latitude")) label="Latitude".to_owned() kind=BufferedInputKind::Number
            authoritative=Signal::derive(move || latitude_authoritative.clone()) disabled
            parser=Callback::new(|text: String| text.parse::<f64>().ok().and_then(|degrees| mirabile_app::Latitude::from_degrees(degrees).ok()).map(|value| value.degrees().to_string()).ok_or_else(|| "Enter a latitude from -90 through 90 degrees".to_owned()))
            on_commit=Callback::new(move |text: String| if let Ok(degrees)=text.parse() && let Ok(latitude)=mirabile_app::Latitude::from_degrees(degrees) { let mut value=latitude_base.clone(); if let Some(location)=&mut value.location { location.latitude=latitude; } dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
        <ValidatedField address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("longitude")) label="Longitude".to_owned() kind=BufferedInputKind::Number
            authoritative=Signal::derive(move || longitude_authoritative.clone()) disabled
            parser=Callback::new(|text: String| text.parse::<f64>().ok().and_then(|degrees| mirabile_app::Longitude::from_degrees(degrees).ok()).map(|value| value.degrees().to_string()).ok_or_else(|| "Enter a longitude from -180 through 180 degrees".to_owned()))
            on_commit=Callback::new(move |text: String| if let Ok(degrees)=text.parse() && let Ok(longitude)=mirabile_app::Longitude::from_degrees(degrees) { let mut value=longitude_base.clone(); if let Some(location)=&mut value.location { location.longitude=longitude; } dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); }) />
        <label>"Country / region"<input type="text" prop:value=location.country_region.unwrap_or_default() data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("country-region")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |change| { let raw=event_target_value(&change); let mut value=country_base.clone(); if let Some(location)=&mut value.location { location.country_region=(!raw.trim().is_empty()).then_some(raw); } dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label>
        <label>"Atlas provider"<input type="text" prop:value=atlas.as_ref().map(|value| value.provider.clone()).unwrap_or_default() data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("atlas-provider")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |change| { let mut value=atlas_provider_base.clone(); update_event_atlas(&mut value, |atlas| atlas.provider=event_target_value(&change)); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label>
        <label>"Atlas record ID"<input type="text" prop:value=atlas.as_ref().and_then(|value| value.record_id.clone()).unwrap_or_default() data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("atlas-record")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |change| { let raw=event_target_value(&change); let mut value=atlas_record_base.clone(); update_event_atlas(&mut value, |atlas| atlas.record_id=(!raw.trim().is_empty()).then_some(raw)); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label>
        <label>"Atlas data version"<input type="text" prop:value=atlas.and_then(|value| value.data_version).unwrap_or_default() data-mirabile-control=ControlId::CHART_LIFE_EVENT_FIELD.to_string() data-mirabile-address=chart_item_address(ControlId::CHART_LIFE_EVENT_FIELD, "life-event", item_id, Some("atlas-version")) data-mirabile-kind="text" data-mirabile-enabled=move || (!disabled.get()).to_string() disabled=disabled on:change=move |change| { let raw=event_target_value(&change); let mut value=atlas_version_base.clone(); update_event_atlas(&mut value, |atlas| atlas.data_version=(!raw.trim().is_empty()).then_some(raw)); dispatch_mutation(dispatcher, ControlId::CHART_LIFE_EVENT_FIELD, ChartMutation::LifeEvents(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label>
    </div> }
}

fn update_event_atlas(
    event: &mut mirabile_app::LifeEvent,
    update: impl FnOnce(&mut mirabile_app::AtlasRef),
) {
    if let Some(location) = &mut event.location {
        let atlas = location
            .atlas_provenance
            .get_or_insert_with(|| mirabile_app::AtlasRef {
                provider: "Manual".into(),
                record_id: None,
                data_version: None,
            });
        update(atlas);
    }
}

fn chart_item_address(
    control: ControlId,
    collection: &'static str,
    item_id: mirabile_app::DraftItemId,
    field: Option<&'static str>,
) -> String {
    let mut qualifiers = vec![
        ("collection", collection.to_owned()),
        ("draft-item", item_id.to_string()),
    ];
    if let Some(field) = field {
        qualifiers.push(("field", field.to_owned()));
    }
    ControlAddress::qualified(control, qualifiers)
        .expect("chart item address")
        .to_string()
}
fn chart_nested_item_address(
    control: ControlId,
    parent: mirabile_app::DraftItemId,
    item_id: mirabile_app::DraftItemId,
    field: Option<&'static str>,
) -> String {
    let mut qualifiers = vec![
        ("collection", "life-event-notes".to_owned()),
        ("parent", parent.to_string()),
        ("draft-item", item_id.to_string()),
    ];
    if let Some(field) = field {
        qualifiers.push(("field", field.to_owned()));
    }
    ControlAddress::qualified(control, qualifiers)
        .expect("nested chart item address")
        .to_string()
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
                    TimezoneAuthoringMode::Unknown => "unknown",
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

fn parse_tags(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn component_identity(
    kind: &str,
    metadata: &mirabile_app::ChartComponentMetadataReadModel,
) -> String {
    metadata.resource_id.map_or_else(
        || format!("{kind}: identity and revisions are allocated by the atomic save"),
        |resource_id| {
            format!(
                "{kind}: {resource_id} · schema {} · r{} · created {} · modified {}",
                metadata
                    .schema_version
                    .expect("saved component has a schema version"),
                metadata.revision.expect("saved component has a revision"),
                metadata
                    .created_at
                    .expect("saved component has a creation timestamp")
                    .unix_millis(),
                metadata
                    .modified_at
                    .expect("saved component has a modification timestamp")
                    .unix_millis(),
            )
        },
    )
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

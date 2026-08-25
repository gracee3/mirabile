use std::fmt::Display;

use leptos::{ev, html, prelude::*};
use mirabile_app::ControlOptionDescriptor;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BufferedInputKind {
    Date,
    Number,
    Text,
    Time,
}

impl BufferedInputKind {
    const fn html_type(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Number | Self::Text => "text",
            Self::Time => "time",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EditBuffer<T> {
    authoritative: T,
    buffer: String,
    editing: bool,
    error: Option<String>,
}

impl<T> EditBuffer<T>
where
    T: Clone + Display,
{
    pub(super) fn new(authoritative: T) -> Self {
        Self {
            buffer: authoritative.to_string(),
            authoritative,
            editing: false,
            error: None,
        }
    }

    pub(super) fn begin(&mut self) {
        self.buffer = self.authoritative.to_string();
        self.error = None;
        self.editing = true;
    }

    pub(super) fn input(&mut self, value: impl Into<String>) {
        self.buffer = value.into();
    }

    pub(super) fn validate(&mut self, parser: impl FnOnce(&str) -> Result<T, String>) -> Option<T> {
        match parser(&self.buffer) {
            Ok(value) => {
                self.error = None;
                Some(value)
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    pub(super) fn commit(&mut self, value: T) {
        self.authoritative = value;
        self.buffer = self.authoritative.to_string();
        self.error = None;
        self.editing = false;
    }

    pub(super) fn cancel(&mut self) {
        self.buffer = self.authoritative.to_string();
        self.error = None;
        self.editing = false;
    }

    pub(super) fn synchronize(&mut self, authoritative: T) {
        self.authoritative = authoritative;
        if !self.editing {
            self.buffer = self.authoritative.to_string();
            self.error = None;
        }
    }

    pub(super) fn buffer(&self) -> &str {
        &self.buffer
    }

    pub(super) const fn editing(&self) -> bool {
        self.editing
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[component]
pub(super) fn Panel(
    title: String,
    #[prop(optional)] labelled_by: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <section class="workbench-panel" aria-labelledby=labelled_by>
            <h2>{title}</h2>
            {children()}
        </section>
    }
}

#[component]
pub(super) fn DisclosureSection(
    title: String,
    #[prop(default = true)] initially_open: bool,
    children: Children,
) -> impl IntoView {
    let open = RwSignal::new(initially_open);
    view! {
        <section class="disclosure-section">
            <button
                type="button"
                class="disclosure-trigger"
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|open| *open = !*open)
            >
                {title}
            </button>
            <div hidden=move || !open.get()>{children()}</div>
        </section>
    }
}

#[component]
pub(super) fn FieldRow(
    label: String,
    #[prop(optional)] help: Option<String>,
    #[prop(optional)] error: Option<Signal<Option<String>>>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="field-row">
            <span class="field-row-label">{label}</span>
            <div class="field-row-control">{children()}</div>
            {help.map(|help| view! { <small class="field-help">{help}</small> })}
            {error.map(|error| view! {
                <small class="field-error" role="status">{move || error.get().unwrap_or_default()}</small>
            })}
        </div>
    }
}

#[component]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn BufferedField(
    address: String,
    label: String,
    kind: BufferedInputKind,
    authoritative: Signal<String>,
    disabled: Signal<bool>,
    buffer: RwSignal<String>,
    error: RwSignal<Option<String>>,
    parser: Callback<String, Result<String, String>>,
    on_commit: Callback<String>,
    #[prop(optional)] help: Option<String>,
    #[prop(optional)] qualifier_name: Option<String>,
    #[prop(optional)] qualifier_value: Option<String>,
) -> impl IntoView {
    let control = address
        .split_once('[')
        .map_or_else(|| address.clone(), |(control, _)| control.to_owned());
    let editing = RwSignal::new(false);
    let trigger_ref = NodeRef::<html::Button>::new();
    let input_ref = NodeRef::<html::Input>::new();

    Effect::new(move || {
        let value = authoritative.get();
        if !editing.get_untracked() {
            buffer.set(value);
            error.set(None);
        }
    });
    Effect::new(move || {
        if editing.get()
            && let Some(input) = input_ref.get()
        {
            let _ = input.focus();
            input.select();
        }
    });

    let commit = move || match parser.run(buffer.get_untracked()) {
        Ok(value) => {
            error.set(None);
            on_commit.run(value);
            editing.set(false);
            if let Some(trigger) = trigger_ref.get_untracked() {
                let _ = trigger.focus();
            }
        }
        Err(message) => error.set(Some(message)),
    };
    let cancel = move || {
        buffer.set(authoritative.get_untracked());
        error.set(None);
        editing.set(false);
        if let Some(trigger) = trigger_ref.get_untracked() {
            let _ = trigger.focus();
        }
    };

    view! {
        <div
            class="buffered-field"
            data-mirabile-control=control
            data-mirabile-address=address
            data-mirabile-qualifier-name=qualifier_name
            data-mirabile-qualifier-value=qualifier_value
            data-mirabile-kind=kind.html_type()
            data-mirabile-editing=move || editing.get().to_string()
            data-mirabile-invalid=move || error.get().is_some().to_string()
        >
            <span class="field-label-text">{label}</span>
            <Show
                when=move || editing.get()
                fallback=move || view! {
                    <button
                        node_ref=trigger_ref
                        type="button"
                        class="buffered-value"
                        data-mirabile-native="value"
                        disabled=move || disabled.get()
                        aria-label="Edit value"
                        on:click=move |_| {
                            buffer.set(authoritative.get_untracked());
                            error.set(None);
                            editing.set(true);
                        }
                    >
                        {move || authoritative.get()}
                    </button>
                }
            >
                <div class="buffered-edit">
                    <input
                        node_ref=input_ref
                        type=kind.html_type()
                        data-mirabile-native="value"
                        inputmode=(kind == BufferedInputKind::Number).then_some("decimal")
                        prop:value=move || buffer.get()
                        aria-invalid=move || error.get().is_some().to_string()
                        disabled=move || disabled.get()
                        on:input=move |event| {
                            let value = event_target_value(&event);
                            buffer.set(value.clone());
                            error.set(parser.run(value).err());
                        }
                        on:keydown=move |event: ev::KeyboardEvent| match event.key().as_str() {
                            "Enter" => {
                                event.prevent_default();
                                commit();
                            }
                            "Escape" => {
                                event.prevent_default();
                                cancel();
                            }
                            _ => {}
                        }
                    />
                    <button type="button" class="button primary" on:click=move |_| commit()>
                        "Apply"
                    </button>
                    <button type="button" class="button secondary" on:click=move |_| cancel()>
                        "Cancel"
                    </button>
                </div>
            </Show>
            {help.map(|help| view! { <small class="field-help">{help}</small> })}
            <small class="field-error" role="status">{move || error.get().unwrap_or_default()}</small>
        </div>
    }
}

#[component]
pub(super) fn BufferedTextField(
    address: String,
    label: String,
    authoritative: Signal<String>,
    disabled: Signal<bool>,
    buffer: RwSignal<String>,
    error: RwSignal<Option<String>>,
    parser: Callback<String, Result<String, String>>,
    on_commit: Callback<String>,
) -> impl IntoView {
    view! {
        <BufferedField
            address
            label
            kind=BufferedInputKind::Text
            authoritative
            disabled
            buffer
            error
            parser
            on_commit
        />
    }
}

#[component]
pub(super) fn BufferedNumberField(
    address: String,
    label: String,
    authoritative: Signal<String>,
    disabled: Signal<bool>,
    buffer: RwSignal<String>,
    error: RwSignal<Option<String>>,
    parser: Callback<String, Result<String, String>>,
    on_commit: Callback<String>,
    #[prop(optional)] help: Option<String>,
    #[prop(optional)] qualifier_name: Option<String>,
    #[prop(optional)] qualifier_value: Option<String>,
) -> impl IntoView {
    view! {
        <BufferedField
            address
            label
            kind=BufferedInputKind::Number
            authoritative
            disabled
            buffer
            error
            parser
            on_commit
            help=help.unwrap_or_default()
            qualifier_name=qualifier_name.unwrap_or_default()
            qualifier_value=qualifier_value.unwrap_or_default()
        />
    }
}

#[component]
pub(super) fn BufferedDateField(
    address: String,
    label: String,
    authoritative: Signal<String>,
    disabled: Signal<bool>,
    buffer: RwSignal<String>,
    error: RwSignal<Option<String>>,
    parser: Callback<String, Result<String, String>>,
    on_commit: Callback<String>,
) -> impl IntoView {
    view! {
        <BufferedField
            address
            label
            kind=BufferedInputKind::Date
            authoritative
            disabled
            buffer
            error
            parser
            on_commit
        />
    }
}

#[component]
pub(super) fn BufferedTimeField(
    address: String,
    label: String,
    authoritative: Signal<String>,
    disabled: Signal<bool>,
    buffer: RwSignal<String>,
    error: RwSignal<Option<String>>,
    parser: Callback<String, Result<String, String>>,
    on_commit: Callback<String>,
) -> impl IntoView {
    view! {
        <BufferedField
            address
            label
            kind=BufferedInputKind::Time
            authoritative
            disabled
            buffer
            error
            parser
            on_commit
        />
    }
}

#[component]
pub(super) fn EnumSelect(
    address: String,
    label: String,
    value: Signal<String>,
    options: Signal<Vec<ControlOptionDescriptor>>,
    disabled: Signal<bool>,
    on_change: Callback<String>,
) -> impl IntoView {
    view! {
        <label class="field-label" data-mirabile-control=address>
            <span>{label}</span>
            <select
                prop:value=move || value.get()
                disabled=move || disabled.get()
                on:change=move |event| on_change.run(event_target_value(&event))
            >
                {move || options.get().into_iter().map(|option| view! {
                    <option value=option.value disabled=!option.enabled title=option.disabled_reason>
                        {option.label}
                    </option>
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
pub(super) fn Toggle(
    address: String,
    label: String,
    checked: Signal<bool>,
    disabled: Signal<bool>,
    on_change: Callback<bool>,
) -> impl IntoView {
    view! {
        <label class="check-field" data-mirabile-control=address>
            <input
                type="checkbox"
                prop:checked=move || checked.get()
                disabled=move || disabled.get()
                on:change=move |event| on_change.run(event_target_checked(&event))
            />
            <span>{label}</span>
        </label>
    }
}

#[component]
pub(super) fn Picker(
    address: String,
    label: String,
    value: Signal<String>,
    options: Signal<Vec<ControlOptionDescriptor>>,
    disabled: Signal<bool>,
    on_change: Callback<String>,
) -> impl IntoView {
    view! { <EnumSelect address label value options disabled on_change /> }
}

#[component]
pub(super) fn ActionControl(
    address: String,
    label: String,
    disabled: Signal<bool>,
    on_activate: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class="button"
            data-mirabile-control=address
            disabled=move || disabled.get()
            on:click=move |_| on_activate.run(())
        >
            {label}
        </button>
    }
}

#[component]
pub(super) fn StatusControl(
    address: String,
    label: String,
    value: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="status-control" data-mirabile-control=address role="status">
            <span>{label}</span>
            <strong>{move || value.get()}</strong>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_local_buffer_never_replaces_authoritative_value() {
        let mut field = EditBuffer::new(6_u32);
        field.begin();
        field.input("6.");
        assert_eq!(
            field.validate(|text| text.parse::<u32>().map_err(|error| error.to_string())),
            None
        );
        assert!(field.error().is_some());
        field.cancel();
        assert_eq!(field.buffer(), "6");
        assert!(!field.editing());
    }

    #[test]
    fn valid_commit_and_external_synchronization_are_explicit() {
        let mut field = EditBuffer::new("old".to_owned());
        field.begin();
        field.input("new");
        let parsed = field
            .validate(|text| Ok(text.trim().to_owned()))
            .expect("valid");
        field.commit(parsed);
        assert_eq!(field.buffer(), "new");
        field.synchronize("remote".to_owned());
        assert_eq!(field.buffer(), "remote");
    }
}

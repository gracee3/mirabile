#[cfg(target_arch = "wasm32")]
pub(super) fn capture() -> Result<mirabile_app::ControlManifest, String> {
    use std::str::FromStr as _;

    use mirabile_app::{
        ControlAddress, ControlDescriptor, ControlEntityIdentity, ControlId, ControlKind,
        ControlManifest, ControlOptionDescriptor,
    };
    use wasm_bindgen::JsCast as _;

    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| "document is unavailable".to_owned())?;
    let nodes = document
        .query_selector_all("[data-mirabile-control]")
        .map_err(|_| "control query failed".to_owned())?;
    let mut descriptors = Vec::with_capacity(nodes.length() as usize);
    for index in 0..nodes.length() {
        let element = nodes
            .item(index)
            .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
            .ok_or_else(|| "instrumented control was not an Element".to_owned())?;
        let control = element
            .get_attribute("data-mirabile-control")
            .ok_or_else(|| "instrumented control omitted its ID".to_owned())?;
        let address = element
            .get_attribute("data-mirabile-address")
            .map_or_else(
                || ControlId::from_str(&control).map(ControlAddress::new),
                |address| ControlAddress::from_str(&address),
            )
            .map_err(|error| error.to_string())?;
        let tag = element.tag_name();
        let input = element.clone().dyn_into::<web_sys::HtmlInputElement>().ok();
        let select = element
            .clone()
            .dyn_into::<web_sys::HtmlSelectElement>()
            .ok();
        let nested_select = element
            .query_selector("select")
            .ok()
            .flatten()
            .and_then(|select| select.dyn_into::<web_sys::HtmlSelectElement>().ok());
        let effective_select = select.as_ref().or(nested_select.as_ref());
        let nested_input = element
            .query_selector("input")
            .ok()
            .flatten()
            .and_then(|input| input.dyn_into::<web_sys::HtmlInputElement>().ok());
        let effective_input = input.as_ref().or(nested_input.as_ref());
        let kind = if tag == "BUTTON" {
            ControlKind::Action
        } else if effective_select.is_some() {
            ControlKind::Select
        } else if effective_input.is_some_and(|input| input.type_() == "checkbox") {
            ControlKind::Checkbox
        } else if effective_input.is_some_and(|input| input.type_() == "date") {
            ControlKind::Date
        } else if effective_input.is_some_and(|input| input.type_() == "time") {
            ControlKind::Time
        } else {
            ControlKind::Text
        };
        let value = if let Some(select) = effective_select {
            serde_json::Value::String(select.value())
        } else if let Some(input) = effective_input {
            if input.type_() == "checkbox" {
                serde_json::Value::Bool(input.checked())
            } else {
                serde_json::Value::String(input.value())
            }
        } else {
            serde_json::Value::String(element.text_content().unwrap_or_default().trim().to_owned())
        };
        let disabled = element.has_attribute("disabled")
            || effective_input.is_some_and(web_sys::HtmlInputElement::disabled)
            || effective_select.is_some_and(web_sys::HtmlSelectElement::disabled);
        let options = effective_select.map_or_else(Vec::new, |select| {
            (0..select.length())
                .filter_map(|index| select.item(index))
                .filter_map(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
                .map(|option| ControlOptionDescriptor {
                    value: option.value(),
                    label: option.label(),
                    enabled: !option.disabled(),
                    disabled_reason: option
                        .disabled()
                        .then(|| option.get_attribute("title"))
                        .flatten(),
                })
                .collect()
        });
        descriptors.push(ControlDescriptor {
            address,
            kind,
            label: element
                .get_attribute("aria-label")
                .or_else(|| element.get_attribute("data-mirabile-label"))
                .unwrap_or_else(|| element.text_content().unwrap_or_default().trim().to_owned()),
            value,
            local_buffer: element
                .get_attribute("data-mirabile-editing")
                .is_some_and(|value| value == "true")
                .then(|| nested_input.as_ref().map(web_sys::HtmlInputElement::value))
                .flatten(),
            locked: element.has_attribute("readonly"),
            editing: element.get_attribute("data-mirabile-editing").as_deref() == Some("true"),
            invalid: element.get_attribute("data-mirabile-invalid").as_deref() == Some("true")
                || element.get_attribute("aria-invalid").as_deref() == Some("true"),
            pending: element.get_attribute("aria-busy").as_deref() == Some("true"),
            enabled: !disabled,
            disabled_reason: disabled.then(|| element.get_attribute("title")).flatten(),
            options,
            entity: ControlEntityIdentity {
                resource_id: element
                    .get_attribute("data-mirabile-resource")
                    .or_else(|| element.get_attribute("data-mirabile-definition"))
                    .and_then(|value| mirabile_app::ResourceId::from_str(&value).ok()),
                instance_id: element
                    .get_attribute("data-mirabile-instance")
                    .and_then(|value| mirabile_app::InstanceId::from_str(&value).ok()),
                view_id: element
                    .get_attribute("data-mirabile-view")
                    .and_then(|value| mirabile_app::ViewInstanceId::from_str(&value).ok()),
            },
        });
    }
    ControlManifest::new(descriptors).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn capture() -> Result<mirabile_app::ControlManifest, String> {
    Err("control manifests require a browser build".to_owned())
}

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
        let textarea = element
            .clone()
            .dyn_into::<web_sys::HtmlTextAreaElement>()
            .ok();
        let nested_textarea = element
            .query_selector("textarea")
            .ok()
            .flatten()
            .and_then(|textarea| textarea.dyn_into::<web_sys::HtmlTextAreaElement>().ok());
        let effective_textarea = textarea.as_ref().or(nested_textarea.as_ref());
        let kind = element
            .get_attribute("data-mirabile-kind")
            .ok_or_else(|| format!("instrumented control {address} omitted its semantic kind"))?
            .parse::<ControlKind>()
            .map_err(|error| format!("instrumented control {address}: {error}"))?;
        let value = if let Some(value) = element.get_attribute("data-mirabile-value") {
            serde_json::Value::String(value)
        } else if let Some(select) = effective_select {
            serde_json::Value::String(select.value())
        } else if let Some(textarea) = effective_textarea {
            serde_json::Value::String(textarea.value())
        } else if let Some(input) = effective_input {
            if input.type_() == "checkbox" {
                serde_json::Value::Bool(input.checked())
            } else {
                serde_json::Value::String(input.value())
            }
        } else {
            serde_json::Value::String(element.text_content().unwrap_or_default().trim().to_owned())
        };
        let enabled = semantic_bool(&element, "data-mirabile-enabled", true, &address)?;
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
        let locked = semantic_bool(&element, "data-mirabile-locked", false, &address)?;
        let editing = semantic_bool(&element, "data-mirabile-editing", false, &address)?;
        let invalid = semantic_bool(&element, "data-mirabile-invalid", false, &address)?;
        let pending = semantic_bool(&element, "data-mirabile-pending", false, &address)?;
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
                .then(|| {
                    nested_input
                        .as_ref()
                        .map(web_sys::HtmlInputElement::value)
                        .or_else(|| {
                            nested_textarea
                                .as_ref()
                                .map(web_sys::HtmlTextAreaElement::value)
                        })
                })
                .flatten(),
            locked,
            editing,
            invalid,
            pending,
            enabled,
            disabled_reason: (!enabled).then(|| {
                element
                    .get_attribute("data-mirabile-disabled-reason")
                    .or_else(|| element.get_attribute("title"))
                    .or_else(|| effective_input.and_then(|input| input.get_attribute("title")))
                    .or_else(|| effective_select.and_then(|select| select.get_attribute("title")))
                    .unwrap_or_else(|| "Control is currently unavailable".to_owned())
            }),
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

#[cfg(target_arch = "wasm32")]
fn semantic_bool(
    element: &web_sys::Element,
    attribute: &str,
    default: bool,
    address: &mirabile_app::ControlAddress,
) -> Result<bool, String> {
    match element.get_attribute(attribute).as_deref() {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(value) => Err(format!(
            "instrumented control {address} published invalid {attribute} value {value}"
        )),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn capture() -> Result<mirabile_app::ControlManifest, String> {
    Err("control manifests require a browser build".to_owned())
}

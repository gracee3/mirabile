use std::{borrow::Cow, collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use crate::{InstanceId, ResourceId, ViewInstanceId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ControlId(Cow<'static, str>);

impl ControlId {
    pub const APPLICATION_REFRESH: Self = Self(Cow::Borrowed("application.refresh"));
    pub const APPLICATION_RETRY: Self = Self(Cow::Borrowed("application.retry"));
    pub const ASPECT_EDIT: Self = Self(Cow::Borrowed("aspect.edit"));
    pub const ASPECT_DUPLICATE: Self = Self(Cow::Borrowed("aspect.duplicate"));
    pub const ASPECT_ENABLED: Self = Self(Cow::Borrowed("aspect.enabled"));
    pub const ASPECT_MAXIMUM_ORB: Self = Self(Cow::Borrowed("aspect.maximum-orb"));
    pub const ASPECT_NEW: Self = Self(Cow::Borrowed("aspect.new"));
    pub const ASPECT_RESOURCE: Self = Self(Cow::Borrowed("aspect.resource"));
    pub const ASPECT_TITLE: Self = Self(Cow::Borrowed("aspect.title"));
    pub const CHART_ACTIVATE: Self = Self(Cow::Borrowed("chart.activate"));
    pub const CHART_CLOSE: Self = Self(Cow::Borrowed("chart.close"));
    pub const CHART_COORDINATES: Self = Self(Cow::Borrowed("chart.coordinates"));
    pub const CHART_CIVIL_DATE: Self = Self(Cow::Borrowed("chart.civil-date"));
    pub const CHART_CIVIL_TIME: Self = Self(Cow::Borrowed("chart.civil-time"));
    pub const CHART_EDITOR_CANCEL: Self = Self(Cow::Borrowed("chart.editor-cancel"));
    pub const CHART_EDIT_SAVED: Self = Self(Cow::Borrowed("chart.edit-saved"));
    pub const CHART_EDITOR_SAVE: Self = Self(Cow::Borrowed("chart.editor-save"));
    pub const CHART_EVENT_KIND: Self = Self(Cow::Borrowed("chart.event-kind"));
    pub const CHART_FIXED_OFFSET: Self = Self(Cow::Borrowed("chart.fixed-offset"));
    pub const CHART_HOUSES: Self = Self(Cow::Borrowed("chart.houses"));
    pub const CHART_LATITUDE: Self = Self(Cow::Borrowed("chart.latitude"));
    pub const CHART_LOCATION_ENABLED: Self = Self(Cow::Borrowed("chart.location-enabled"));
    pub const CHART_LOCATION_NAME: Self = Self(Cow::Borrowed("chart.location-name"));
    pub const CHART_LONGITUDE: Self = Self(Cow::Borrowed("chart.longitude"));
    pub const CHART_NEW: Self = Self(Cow::Borrowed("chart.new"));
    pub const CHART_OPEN: Self = Self(Cow::Borrowed("chart.open"));
    pub const CHART_SELECT: Self = Self(Cow::Borrowed("chart.select"));
    pub const CHART_SUBJECT_NAME: Self = Self(Cow::Borrowed("chart.subject-name"));
    pub const CHART_TIMEZONE: Self = Self(Cow::Borrowed("chart.timezone"));
    pub const CHART_TITLE: Self = Self(Cow::Borrowed("chart.title"));
    pub const CHART_ZODIAC: Self = Self(Cow::Borrowed("chart.zodiac"));
    pub const DISPLAY_POINT: Self = Self(Cow::Borrowed("display.point"));
    pub const DISPLAY_PROMOTE: Self = Self(Cow::Borrowed("display.promote"));
    pub const DIAGNOSTICS_EXPORT_SNAPSHOT: Self =
        Self(Cow::Borrowed("diagnostics.export-snapshot"));
    pub const DIAGNOSTICS_EXPORT_TRACE: Self = Self(Cow::Borrowed("diagnostics.export-trace"));
    pub const DRAFT_CANCEL: Self = Self(Cow::Borrowed("draft.cancel"));
    pub const DRAFT_SAVE: Self = Self(Cow::Borrowed("draft.save"));
    pub const MACRO_CLEAR: Self = Self(Cow::Borrowed("macro.clear"));
    pub const MACRO_EXPORT: Self = Self(Cow::Borrowed("macro.export"));
    pub const MACRO_IMPORT: Self = Self(Cow::Borrowed("macro.import"));
    pub const MACRO_JSON: Self = Self(Cow::Borrowed("macro.json"));
    pub const MACRO_NAME: Self = Self(Cow::Borrowed("macro.name"));
    pub const MACRO_REPLAY: Self = Self(Cow::Borrowed("macro.replay"));
    pub const MACRO_START: Self = Self(Cow::Borrowed("macro.start"));
    pub const MACRO_STOP: Self = Self(Cow::Borrowed("macro.stop"));
    pub const COCKPIT_SEARCH: Self = Self(Cow::Borrowed("cockpit.search"));
    pub const COCKPIT_EXPAND_ALL: Self = Self(Cow::Borrowed("cockpit.expand-all"));
    pub const COCKPIT_COLLAPSE_ALL: Self = Self(Cow::Borrowed("cockpit.collapse-all"));
    pub const RESOURCE_NEW: Self = Self(Cow::Borrowed("resource.new"));
    pub const RESOURCE_EDIT: Self = Self(Cow::Borrowed("resource.edit"));
    pub const RESOURCE_TITLE: Self = Self(Cow::Borrowed("resource.title"));
    pub const RESOURCE_DESCRIPTION: Self = Self(Cow::Borrowed("resource.description"));
    pub const RESOURCE_TAGS: Self = Self(Cow::Borrowed("resource.tags"));
    pub const RESOURCE_SAVE: Self = Self(Cow::Borrowed("resource.save"));
    pub const RESOURCE_CANCEL: Self = Self(Cow::Borrowed("resource.cancel"));
    pub const RESOURCE_ANALYSIS_APPLYING: Self =
        Self(Cow::Borrowed("resource.analysis.applying-state"));
    pub const RESOURCE_ANALYSIS_PATTERNS: Self = Self(Cow::Borrowed("resource.analysis.patterns"));
    pub const RESOURCE_ANALYSIS_MAXIMUM_HITS: Self =
        Self(Cow::Borrowed("resource.analysis.maximum-hits"));
    pub const RESOURCE_THEME_COLOR: Self = Self(Cow::Borrowed("resource.theme.color"));
    pub const RESOURCE_VIEW_WIDTH: Self = Self(Cow::Borrowed("resource.view.width"));
    pub const RESOURCE_VIEW_HEIGHT: Self = Self(Cow::Borrowed("resource.view.height"));
    pub const RESOURCE_QUERY_DESCRIPTION: Self = Self(Cow::Borrowed("resource.query.description"));
    pub const RESOURCE_POINT: Self = Self(Cow::Borrowed("resource.point"));
    pub const RESOURCE_WHEEL_FIELD: Self = Self(Cow::Borrowed("resource.wheel.field"));
    pub const BINDING_MODE: Self = Self(Cow::Borrowed("binding.mode"));
    pub const BINDING_RESOURCE: Self = Self(Cow::Borrowed("binding.resource"));
    pub const BINDING_REVISION: Self = Self(Cow::Borrowed("binding.revision"));
    pub const REPOSITORY_SELECT: Self = Self(Cow::Borrowed("repository.select"));
    pub const REPOSITORY_DELETE: Self = Self(Cow::Borrowed("repository.delete"));
    pub const REPOSITORY_CONFIRM_DELETE: Self = Self(Cow::Borrowed("repository.confirm-delete"));
    pub const VIEW_ACTIVATE: Self = Self(Cow::Borrowed("view.activate"));
    pub const VIEW_SLOT: Self = Self(Cow::Borrowed("view.slot"));
    pub const WORKSPACE_SAVE: Self = Self(Cow::Borrowed("workspace.save"));
    pub const WORKSPACE_DISCARD: Self = Self(Cow::Borrowed("workspace.discard"));
    pub const WORKSPACE_LOAD_DEMO: Self = Self(Cow::Borrowed("workspace.load-demo"));
    pub const WORKSPACE_NEW: Self = Self(Cow::Borrowed("workspace.new"));
    pub const WORKSPACE_OPEN: Self = Self(Cow::Borrowed("workspace.open"));
    pub const WORKSPACE_SWITCH_DISCARD: Self = Self(Cow::Borrowed("workspace.switch-discard"));
    pub const WORKSPACE_SWITCH_SAVE: Self = Self(Cow::Borrowed("workspace.switch-save"));
    pub const WORKSPACE_SWITCH_STAY: Self = Self(Cow::Borrowed("workspace.switch-stay"));
    pub const WORKSPACE_TITLE: Self = Self(Cow::Borrowed("workspace.title"));

    pub fn new(value: impl Into<String>) -> Result<Self, ControlAddressError> {
        let value = value.into();
        validate_control_id(&value)?;
        Ok(Self(Cow::Owned(value)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for ControlId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl FromStr for ControlId {
    type Err = ControlAddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for ControlId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ControlAddress {
    pub control: ControlId,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub qualifiers: BTreeMap<String, String>,
}

impl<'de> Deserialize<'de> for ControlAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawControlAddress {
            control: ControlId,
            #[serde(default)]
            qualifiers: BTreeMap<String, String>,
        }

        let raw = RawControlAddress::deserialize(deserializer)?;
        Self::qualified(raw.control, raw.qualifiers).map_err(D::Error::custom)
    }
}

impl ControlAddress {
    pub fn new(control: ControlId) -> Self {
        Self {
            control,
            qualifiers: BTreeMap::new(),
        }
    }

    pub fn qualified(
        control: ControlId,
        qualifiers: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Result<Self, ControlAddressError> {
        let mut address = Self::new(control);
        for (name, value) in qualifiers {
            address.insert_qualifier(name.into(), value.into())?;
        }
        Ok(address)
    }

    pub fn insert_qualifier(
        &mut self,
        name: String,
        value: String,
    ) -> Result<(), ControlAddressError> {
        validate_qualifier_name(&name)?;
        validate_qualifier_value(&value)?;
        if self.qualifiers.insert(name.clone(), value).is_some() {
            return Err(ControlAddressError::DuplicateQualifier(name));
        }
        Ok(())
    }
}

impl fmt::Display for ControlAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.control.fmt(formatter)?;
        if !self.qualifiers.is_empty() {
            formatter.write_str("[")?;
            for (index, (name, value)) in self.qualifiers.iter().enumerate() {
                if index > 0 {
                    formatter.write_str(",")?;
                }
                write!(formatter, "{name}={value}")?;
            }
            formatter.write_str("]")?;
        }
        Ok(())
    }
}

impl FromStr for ControlAddress {
    type Err = ControlAddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (control, qualifiers) = match value.split_once('[') {
            None => (value, None),
            Some((control, suffix)) => {
                let qualifiers = suffix
                    .strip_suffix(']')
                    .ok_or(ControlAddressError::MalformedAddress)?;
                (control, Some(qualifiers))
            }
        };
        if control.contains(']') {
            return Err(ControlAddressError::MalformedAddress);
        }
        let mut address = Self::new(ControlId::from_str(control)?);
        if let Some(qualifiers) = qualifiers {
            if qualifiers.is_empty() {
                return Err(ControlAddressError::MalformedAddress);
            }
            for qualifier in qualifiers.split(',') {
                let (name, value) = qualifier
                    .split_once('=')
                    .ok_or(ControlAddressError::MalformedAddress)?;
                address.insert_qualifier(name.to_owned(), value.to_owned())?;
            }
        }
        Ok(address)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ControlAddressError {
    #[error("control IDs must contain two or more lowercase dotted segments")]
    InvalidControlId,
    #[error("control qualifier names must be lowercase semantic tokens")]
    InvalidQualifierName,
    #[error("control qualifier values must be nonempty canonical tokens")]
    InvalidQualifierValue,
    #[error("control qualifier {0} was repeated")]
    DuplicateQualifier(String),
    #[error("control address {0} was repeated in the manifest")]
    DuplicateAddress(String),
    #[error("control address syntax is malformed")]
    MalformedAddress,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Action,
    Checkbox,
    Date,
    Number,
    Picker,
    Select,
    Status,
    Text,
    Time,
    Toggle,
}

impl ControlKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Action => "action",
            Self::Checkbox => "checkbox",
            Self::Date => "date",
            Self::Number => "number",
            Self::Picker => "picker",
            Self::Select => "select",
            Self::Status => "status",
            Self::Text => "text",
            Self::Time => "time",
            Self::Toggle => "toggle",
        }
    }
}

impl fmt::Display for ControlKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl FromStr for ControlKind {
    type Err = UnknownControlKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "action" => Ok(Self::Action),
            "checkbox" => Ok(Self::Checkbox),
            "date" => Ok(Self::Date),
            "number" => Ok(Self::Number),
            "picker" => Ok(Self::Picker),
            "select" => Ok(Self::Select),
            "status" => Ok(Self::Status),
            "text" => Ok(Self::Text),
            "time" => Ok(Self::Time),
            "toggle" => Ok(Self::Toggle),
            _ => Err(UnknownControlKind(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown semantic control kind {0}")]
pub struct UnknownControlKind(String);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ControlOptionDescriptor {
    pub value: String,
    pub label: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ControlEntityIdentity {
    pub resource_id: Option<ResourceId>,
    pub instance_id: Option<InstanceId>,
    pub view_id: Option<ViewInstanceId>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ControlDescriptor {
    pub address: ControlAddress,
    pub kind: ControlKind,
    pub label: String,
    pub value: serde_json::Value,
    pub local_buffer: Option<String>,
    pub locked: bool,
    pub editing: bool,
    pub invalid: bool,
    pub pending: bool,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub options: Vec<ControlOptionDescriptor>,
    pub entity: ControlEntityIdentity,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ControlManifest {
    pub controls: Vec<ControlDescriptor>,
}

impl ControlManifest {
    pub fn new(controls: Vec<ControlDescriptor>) -> Result<Self, ControlAddressError> {
        let mut addresses = std::collections::BTreeSet::new();
        for control in &controls {
            if !addresses.insert(control.address.clone()) {
                return Err(ControlAddressError::DuplicateAddress(
                    control.address.to_string(),
                ));
            }
        }
        Ok(Self { controls })
    }

    pub fn get(&self, address: &ControlAddress) -> Option<&ControlDescriptor> {
        self.controls
            .iter()
            .find(|control| &control.address == address)
    }
}

fn validate_control_id(value: &str) -> Result<(), ControlAddressError> {
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() < 2 || segments.iter().any(|segment| !valid_name_segment(segment)) {
        return Err(ControlAddressError::InvalidControlId);
    }
    Ok(())
}

fn validate_qualifier_name(value: &str) -> Result<(), ControlAddressError> {
    if valid_name_segment(value) {
        Ok(())
    } else {
        Err(ControlAddressError::InvalidQualifierName)
    }
}

fn validate_qualifier_value(value: &str) -> Result<(), ControlAddressError> {
    if !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.:".contains(&byte)
        })
    {
        Ok(())
    } else {
        Err(ControlAddressError::InvalidQualifierValue)
    }
}

fn valid_name_segment(value: &str) -> bool {
    value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_ids_reject_noncanonical_dotted_names() {
        assert_eq!(
            ControlId::from_str("aspect.maximum-orb").expect("canonical ID"),
            ControlId::ASPECT_MAXIMUM_ORB
        );
        for invalid in ["aspect", "Aspect.orb", "aspect..orb", "aspect.maximum_orb"] {
            assert!(ControlId::from_str(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn addresses_are_canonical_and_round_trip() {
        let address = ControlAddress::from_str(
            "view.slot[slot=primary,view=30000000-0000-4000-8000-000000000001]",
        )
        .expect("address");
        assert_eq!(
            address.to_string(),
            "view.slot[slot=primary,view=30000000-0000-4000-8000-000000000001]"
        );
        assert_eq!(
            ControlAddress::from_str(&address.to_string()).expect("round trip"),
            address
        );
        assert!(ControlAddress::from_str("view.slot[slot=primary,slot=outer]").is_err());
        assert!(ControlAddress::from_str("view.slot[]").is_err());
    }

    #[test]
    fn serde_cannot_bypass_control_validation() {
        assert!(serde_json::from_str::<ControlId>(r#""Aspect.orb""#).is_err());
        assert!(
            serde_json::from_str::<ControlAddress>(
                r#"{"control":"view.slot","qualifiers":{"slot":""}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn control_kind_strings_are_canonical_and_complete() {
        let kinds = [
            ControlKind::Action,
            ControlKind::Checkbox,
            ControlKind::Date,
            ControlKind::Number,
            ControlKind::Picker,
            ControlKind::Select,
            ControlKind::Status,
            ControlKind::Text,
            ControlKind::Time,
            ControlKind::Toggle,
        ];

        for kind in kinds {
            assert_eq!(ControlKind::from_str(kind.as_str()), Ok(kind));
            assert_eq!(kind.to_string(), kind.as_str());
        }
        assert!(ControlKind::from_str("radio").is_err());
        assert!(ControlKind::from_str("Text").is_err());
    }
}

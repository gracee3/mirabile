use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AnalysisProfile, Angle, AspectId, ChartDefinition, ChartRecord, DomainValidate,
    DomainValidationError, DomainValidationIssue, PointId, QueryDefinition, ResourceId, Revision,
    RevisionError, SchemaVersion, Theme, Timestamp, ViewDocument, WheelTemplate, Workspace,
    validation::{in_range, nonempty, positive},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    ChartRecord,
    ChartDefinition,
    PointSet,
    AspectSet,
    CalculationProfile,
    AnalysisProfile,
    WheelTemplate,
    ViewDocument,
    Theme,
    RulershipScheme,
    DignityScheme,
    ArabicPartsSet,
    FixedStarSet,
    QueryDefinition,
    Workspace,
}

pub trait ResourcePayload: DomainValidate {
    const KIND: ResourceKind;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceEnvelope<T> {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub schema_version: SchemaVersion,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub revision: Revision,
    pub created_at: Timestamp,
    pub modified_at: Timestamp,
    pub payload: T,
}

impl<T: ResourcePayload> ResourceEnvelope<T> {
    pub fn new(title: impl Into<String>, payload: T, now: Timestamp) -> Self {
        Self::with_id(ResourceId::new(), title, payload, now)
    }

    pub fn with_id(id: ResourceId, title: impl Into<String>, payload: T, now: Timestamp) -> Self {
        Self {
            id,
            kind: T::KIND,
            schema_version: SchemaVersion::V1,
            title: title.into(),
            description: None,
            tags: Vec::new(),
            revision: Revision::INITIAL,
            created_at: now,
            modified_at: now,
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), ResourceError> {
        if self.kind != T::KIND {
            return Err(ResourceError::KindMismatch {
                declared: self.kind,
                payload: T::KIND,
            });
        }
        if self.title.trim().is_empty() {
            return Err(ResourceError::EmptyTitle);
        }
        if self.schema_version != SchemaVersion::V1 {
            return Err(ResourceError::UnsupportedSchemaVersion {
                actual: self.schema_version,
            });
        }
        if self.modified_at < self.created_at {
            return Err(ResourceError::Domain(DomainValidationError::new(
                "modified_at",
                DomainValidationIssue::Chronology,
            )));
        }
        let mut normalized_tags = self
            .tags
            .iter()
            .enumerate()
            .map(|(index, tag)| {
                nonempty(tag, &format!("tags[{index}]"))?;
                Ok(tag.trim().to_owned())
            })
            .collect::<Result<Vec<_>, DomainValidationError>>()?;
        normalized_tags.sort();
        if normalized_tags.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ResourceError::Domain(DomainValidationError::new(
                "tags",
                DomainValidationIssue::Duplicate,
            )));
        }
        self.payload
            .domain_validate()
            .map_err(|error| error.prepend("payload"))?;
        Ok(())
    }

    pub fn next_with_payload(
        &self,
        payload: T,
        modified_at: Timestamp,
    ) -> Result<Self, ResourceError>
    where
        T: Clone,
    {
        Ok(Self {
            id: self.id,
            kind: self.kind,
            schema_version: self.schema_version,
            title: self.title.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            revision: self.revision.next()?,
            created_at: self.created_at,
            modified_at,
            payload,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "binding", rename_all = "snake_case")]
pub enum ResourceBinding<T> {
    Follow { id: ResourceId },
    Pinned { id: ResourceId, revision: Revision },
    Inline { value: T },
}

impl<T> ResourceBinding<T> {
    pub const fn id(&self) -> Option<ResourceId> {
        match self {
            Self::Follow { id } | Self::Pinned { id, .. } => Some(*id),
            Self::Inline { .. } => None,
        }
    }

    pub const fn pinned_revision(&self) -> Option<Revision> {
        match self {
            Self::Pinned { revision, .. } => Some(*revision),
            Self::Follow { .. } | Self::Inline { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PointSelector {
    Point(PointId),
    Category(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PointSet {
    pub points: Vec<PointSelector>,
}

impl PointSet {
    pub fn direct_points(&self) -> impl Iterator<Item = &PointId> {
        self.points.iter().filter_map(|selector| match selector {
            PointSelector::Point(point) => Some(point),
            PointSelector::Category(_) => None,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectSet {
    pub aspects: Vec<AspectDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectDefinition {
    pub id: AspectId,
    pub name: String,
    pub angle: Angle,
    pub enabled: bool,
    pub orbs: OrbPolicy,
    pub classification: AspectClass,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct OrbPolicy {
    pub maximum: Angle,
    pub applying_multiplier: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectClass {
    Major,
    Minor,
    Harmonic,
    Custom,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "resource_type", content = "resource", rename_all = "snake_case")]
pub enum CanonicalResource {
    ChartRecord(ResourceEnvelope<ChartRecord>),
    ChartDefinition(ResourceEnvelope<ChartDefinition>),
    PointSet(ResourceEnvelope<PointSet>),
    AspectSet(ResourceEnvelope<AspectSet>),
    AnalysisProfile(ResourceEnvelope<AnalysisProfile>),
    WheelTemplate(ResourceEnvelope<WheelTemplate>),
    ViewDocument(ResourceEnvelope<ViewDocument>),
    Theme(ResourceEnvelope<Theme>),
    QueryDefinition(ResourceEnvelope<QueryDefinition>),
    Workspace(ResourceEnvelope<Workspace>),
}

macro_rules! resource_access {
    ($self:ident, $field:ident) => {
        match $self {
            Self::ChartRecord(value) => value.$field,
            Self::ChartDefinition(value) => value.$field,
            Self::PointSet(value) => value.$field,
            Self::AspectSet(value) => value.$field,
            Self::AnalysisProfile(value) => value.$field,
            Self::WheelTemplate(value) => value.$field,
            Self::ViewDocument(value) => value.$field,
            Self::Theme(value) => value.$field,
            Self::QueryDefinition(value) => value.$field,
            Self::Workspace(value) => value.$field,
        }
    };
}

impl CanonicalResource {
    pub fn id(&self) -> ResourceId {
        resource_access!(self, id)
    }

    pub fn revision(&self) -> Revision {
        resource_access!(self, revision)
    }

    pub fn schema_version(&self) -> SchemaVersion {
        resource_access!(self, schema_version)
    }

    pub fn title(&self) -> &str {
        match self {
            Self::ChartRecord(value) => &value.title,
            Self::ChartDefinition(value) => &value.title,
            Self::PointSet(value) => &value.title,
            Self::AspectSet(value) => &value.title,
            Self::AnalysisProfile(value) => &value.title,
            Self::WheelTemplate(value) => &value.title,
            Self::ViewDocument(value) => &value.title,
            Self::Theme(value) => &value.title,
            Self::QueryDefinition(value) => &value.title,
            Self::Workspace(value) => &value.title,
        }
    }

    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::ChartRecord(_) => ResourceKind::ChartRecord,
            Self::ChartDefinition(_) => ResourceKind::ChartDefinition,
            Self::PointSet(_) => ResourceKind::PointSet,
            Self::AspectSet(_) => ResourceKind::AspectSet,
            Self::AnalysisProfile(_) => ResourceKind::AnalysisProfile,
            Self::WheelTemplate(_) => ResourceKind::WheelTemplate,
            Self::ViewDocument(_) => ResourceKind::ViewDocument,
            Self::Theme(_) => ResourceKind::Theme,
            Self::QueryDefinition(_) => ResourceKind::QueryDefinition,
            Self::Workspace(_) => ResourceKind::Workspace,
        }
    }

    pub fn validate(&self) -> Result<(), ResourceError> {
        match self {
            Self::ChartRecord(value) => value.validate(),
            Self::ChartDefinition(value) => value.validate(),
            Self::PointSet(value) => value.validate(),
            Self::AspectSet(value) => value.validate(),
            Self::AnalysisProfile(value) => value.validate(),
            Self::WheelTemplate(value) => value.validate(),
            Self::ViewDocument(value) => value.validate(),
            Self::Theme(value) => value.validate(),
            Self::QueryDefinition(value) => value.validate(),
            Self::Workspace(value) => value.validate(),
        }
    }
}

impl ResourcePayload for ChartRecord {
    const KIND: ResourceKind = ResourceKind::ChartRecord;
}

impl ResourcePayload for ChartDefinition {
    const KIND: ResourceKind = ResourceKind::ChartDefinition;
}

impl ResourcePayload for PointSet {
    const KIND: ResourceKind = ResourceKind::PointSet;
}

impl ResourcePayload for AspectSet {
    const KIND: ResourceKind = ResourceKind::AspectSet;
}

impl ResourcePayload for AnalysisProfile {
    const KIND: ResourceKind = ResourceKind::AnalysisProfile;
}

impl ResourcePayload for WheelTemplate {
    const KIND: ResourceKind = ResourceKind::WheelTemplate;
}

impl ResourcePayload for ViewDocument {
    const KIND: ResourceKind = ResourceKind::ViewDocument;
}

impl ResourcePayload for Theme {
    const KIND: ResourceKind = ResourceKind::Theme;
}

impl ResourcePayload for QueryDefinition {
    const KIND: ResourceKind = ResourceKind::QueryDefinition;
}

impl ResourcePayload for Workspace {
    const KIND: ResourceKind = ResourceKind::Workspace;
}

impl DomainValidate for PointSet {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        let mut selectors = self.points.clone();
        selectors.sort_by(|lhs, rhs| selector_key(lhs).cmp(&selector_key(rhs)));
        if selectors
            .windows(2)
            .any(|pair| selector_key(&pair[0]) == selector_key(&pair[1]))
        {
            return Err(DomainValidationError::new(
                "points",
                DomainValidationIssue::Duplicate,
            ));
        }
        for (index, selector) in self.points.iter().enumerate() {
            if let PointSelector::Category(category) = selector {
                nonempty(category, &format!("points[{index}].value"))?;
            }
        }
        Ok(())
    }
}

fn selector_key(selector: &PointSelector) -> (u8, &str) {
    match selector {
        PointSelector::Point(point) => (0, point.as_str()),
        PointSelector::Category(category) => (1, category.as_str()),
    }
}

impl DomainValidate for AspectSet {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        let mut ids = self
            .aspects
            .iter()
            .map(|aspect| aspect.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        if ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "aspects.id",
                DomainValidationIssue::Duplicate,
            ));
        }
        for (index, aspect) in self.aspects.iter().enumerate() {
            nonempty(&aspect.name, &format!("aspects[{index}].name"))?;
            in_range(
                aspect.angle.degrees(),
                0.0,
                180.0,
                true,
                &format!("aspects[{index}].angle"),
            )?;
            in_range(
                aspect.orbs.maximum.degrees(),
                0.0,
                180.0,
                true,
                &format!("aspects[{index}].orbs.maximum"),
            )?;
            positive(
                aspect.orbs.applying_multiplier,
                &format!("aspects[{index}].orbs.applying_multiplier"),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ResourceError {
    #[error("resource title must not be empty")]
    EmptyTitle,
    #[error("resource kind {declared:?} does not match payload kind {payload:?}")]
    KindMismatch {
        declared: ResourceKind,
        payload: ResourceKind,
    },
    #[error("portable schema version {actual} is unsupported")]
    UnsupportedSchemaVersion { actual: SchemaVersion },
    #[error(transparent)]
    Domain(#[from] DomainValidationError),
    #[error(transparent)]
    Revision(#[from] RevisionError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_modes_remain_distinct_after_serialization() {
        let id = ResourceId::new();
        let bindings = [
            ResourceBinding::<PointSet>::Follow { id },
            ResourceBinding::Pinned {
                id,
                revision: Revision::INITIAL,
            },
            ResourceBinding::Inline {
                value: PointSet { points: Vec::new() },
            },
        ];

        for binding in bindings {
            let json = serde_json::to_string(&binding).expect("serialize binding");
            let decoded: ResourceBinding<PointSet> =
                serde_json::from_str(&json).expect("deserialize binding");
            assert_eq!(decoded, binding);
        }
    }
}

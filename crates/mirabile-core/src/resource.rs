use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AnalysisProfile, Angle, AspectId, ChartDefinition, ChartRecord, DomainValidate,
    DomainValidationError, DomainValidationIssue, PointId, QueryDefinition, ResourceId, Revision,
    RevisionError, SchemaVersion, Theme, Timestamp, ViewDocument, WheelTemplate, WorkspaceDocument,
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
    WorkspaceDocument,
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
    WorkspaceDocument(ResourceEnvelope<WorkspaceDocument>),
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
            Self::WorkspaceDocument(value) => value.$field,
        }
    };
}

impl CanonicalResource {
    pub const KINDS: [ResourceKind; 10] = [
        ResourceKind::ChartRecord,
        ResourceKind::ChartDefinition,
        ResourceKind::PointSet,
        ResourceKind::AspectSet,
        ResourceKind::AnalysisProfile,
        ResourceKind::WheelTemplate,
        ResourceKind::ViewDocument,
        ResourceKind::Theme,
        ResourceKind::QueryDefinition,
        ResourceKind::WorkspaceDocument,
    ];

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
            Self::WorkspaceDocument(value) => &value.title,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            Self::ChartRecord(value) => value.description.as_deref(),
            Self::ChartDefinition(value) => value.description.as_deref(),
            Self::PointSet(value) => value.description.as_deref(),
            Self::AspectSet(value) => value.description.as_deref(),
            Self::AnalysisProfile(value) => value.description.as_deref(),
            Self::WheelTemplate(value) => value.description.as_deref(),
            Self::ViewDocument(value) => value.description.as_deref(),
            Self::Theme(value) => value.description.as_deref(),
            Self::QueryDefinition(value) => value.description.as_deref(),
            Self::WorkspaceDocument(value) => value.description.as_deref(),
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            Self::ChartRecord(value) => &value.tags,
            Self::ChartDefinition(value) => &value.tags,
            Self::PointSet(value) => &value.tags,
            Self::AspectSet(value) => &value.tags,
            Self::AnalysisProfile(value) => &value.tags,
            Self::WheelTemplate(value) => &value.tags,
            Self::ViewDocument(value) => &value.tags,
            Self::Theme(value) => &value.tags,
            Self::QueryDefinition(value) => &value.tags,
            Self::WorkspaceDocument(value) => &value.tags,
        }
    }

    pub fn created_at(&self) -> Timestamp {
        resource_access!(self, created_at)
    }

    pub fn modified_at(&self) -> Timestamp {
        resource_access!(self, modified_at)
    }

    pub fn set_title(&mut self, title: String) {
        match self {
            Self::ChartRecord(value) => value.title = title,
            Self::ChartDefinition(value) => value.title = title,
            Self::PointSet(value) => value.title = title,
            Self::AspectSet(value) => value.title = title,
            Self::AnalysisProfile(value) => value.title = title,
            Self::WheelTemplate(value) => value.title = title,
            Self::ViewDocument(value) => value.title = title,
            Self::Theme(value) => value.title = title,
            Self::QueryDefinition(value) => value.title = title,
            Self::WorkspaceDocument(value) => value.title = title,
        }
    }

    pub fn set_description(&mut self, description: Option<String>) {
        match self {
            Self::ChartRecord(value) => value.description = description,
            Self::ChartDefinition(value) => value.description = description,
            Self::PointSet(value) => value.description = description,
            Self::AspectSet(value) => value.description = description,
            Self::AnalysisProfile(value) => value.description = description,
            Self::WheelTemplate(value) => value.description = description,
            Self::ViewDocument(value) => value.description = description,
            Self::Theme(value) => value.description = description,
            Self::QueryDefinition(value) => value.description = description,
            Self::WorkspaceDocument(value) => value.description = description,
        }
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        match self {
            Self::ChartRecord(value) => value.tags = tags,
            Self::ChartDefinition(value) => value.tags = tags,
            Self::PointSet(value) => value.tags = tags,
            Self::AspectSet(value) => value.tags = tags,
            Self::AnalysisProfile(value) => value.tags = tags,
            Self::WheelTemplate(value) => value.tags = tags,
            Self::ViewDocument(value) => value.tags = tags,
            Self::Theme(value) => value.tags = tags,
            Self::QueryDefinition(value) => value.tags = tags,
            Self::WorkspaceDocument(value) => value.tags = tags,
        }
    }

    pub fn next_revision(&self, modified_at: Timestamp) -> Result<Self, ResourceError> {
        let next = match self {
            Self::ChartRecord(value) => Self::ChartRecord(next_envelope(value, modified_at)?),
            Self::ChartDefinition(value) => {
                Self::ChartDefinition(next_envelope(value, modified_at)?)
            }
            Self::PointSet(value) => Self::PointSet(next_envelope(value, modified_at)?),
            Self::AspectSet(value) => Self::AspectSet(next_envelope(value, modified_at)?),
            Self::AnalysisProfile(value) => {
                Self::AnalysisProfile(next_envelope(value, modified_at)?)
            }
            Self::WheelTemplate(value) => Self::WheelTemplate(next_envelope(value, modified_at)?),
            Self::ViewDocument(value) => Self::ViewDocument(next_envelope(value, modified_at)?),
            Self::Theme(value) => Self::Theme(next_envelope(value, modified_at)?),
            Self::QueryDefinition(value) => {
                Self::QueryDefinition(next_envelope(value, modified_at)?)
            }
            Self::WorkspaceDocument(value) => {
                Self::WorkspaceDocument(next_envelope(value, modified_at)?)
            }
        };
        next.validate()?;
        Ok(next)
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
            Self::WorkspaceDocument(_) => ResourceKind::WorkspaceDocument,
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
            Self::WorkspaceDocument(value) => value.validate(),
        }
    }
}

fn next_envelope<T: ResourcePayload + Clone>(
    envelope: &ResourceEnvelope<T>,
    modified_at: Timestamp,
) -> Result<ResourceEnvelope<T>, ResourceError> {
    let mut next = envelope.clone();
    next.revision = envelope.revision.next()?;
    next.modified_at = modified_at;
    Ok(next)
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

impl ResourcePayload for WorkspaceDocument {
    const KIND: ResourceKind = ResourceKind::WorkspaceDocument;
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
    fn canonical_resource_kinds_exclude_reserved_payloads() {
        assert_eq!(CanonicalResource::KINDS.len(), 10);
        assert!(CanonicalResource::KINDS.contains(&ResourceKind::ChartRecord));
        assert!(CanonicalResource::KINDS.contains(&ResourceKind::WorkspaceDocument));
        for reserved in [
            ResourceKind::CalculationProfile,
            ResourceKind::RulershipScheme,
            ResourceKind::DignityScheme,
            ResourceKind::ArabicPartsSet,
            ResourceKind::FixedStarSet,
        ] {
            assert!(!CanonicalResource::KINDS.contains(&reserved));
        }
    }

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

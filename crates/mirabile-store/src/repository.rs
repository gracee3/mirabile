use async_trait::async_trait;
use mirabile_core::{CanonicalResource, ResourceId, ResourceKind, Revision, Timestamp};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceTombstone {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub revision: Revision,
    pub deleted_at: Timestamp,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ResourceState {
    Present(CanonicalResource),
    Deleted(ResourceTombstone),
}

impl ResourceState {
    pub fn id(&self) -> ResourceId {
        match self {
            Self::Present(resource) => resource.id(),
            Self::Deleted(tombstone) => tombstone.id,
        }
    }

    pub fn kind(&self) -> ResourceKind {
        match self {
            Self::Present(resource) => resource.kind(),
            Self::Deleted(tombstone) => tombstone.kind,
        }
    }

    pub fn revision(&self) -> Revision {
        match self {
            Self::Present(resource) => resource.revision(),
            Self::Deleted(tombstone) => tombstone.revision,
        }
    }
}

#[async_trait(?Send)]
pub trait ResourceRepository {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError>;

    /// Atomically creates every resource in a non-empty local batch.
    ///
    /// If any resource is invalid, duplicated, or already present, no resource is created.
    async fn create_batch(&self, resources: Vec<CanonicalResource>) -> Result<(), RepositoryError>;

    async fn save(
        &self,
        expected_revision: Revision,
        resource: CanonicalResource,
    ) -> Result<(), RepositoryError>;

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError>;

    async fn get_head(&self, id: ResourceId) -> Result<Option<ResourceState>, RepositoryError>;

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<ResourceState>, RepositoryError>;

    async fn list(
        &self,
        kind: Option<ResourceKind>,
    ) -> Result<Vec<CanonicalResource>, RepositoryError>;

    async fn delete(
        &self,
        id: ResourceId,
        expected_revision: Revision,
        deleted_at: Timestamp,
    ) -> Result<ResourceTombstone, RepositoryError>;
}

pub fn validate_create(resource: &CanonicalResource) -> Result<(), RepositoryError> {
    resource.validate()?;
    if resource.revision() != Revision::INITIAL {
        return Err(RepositoryError::InitialRevisionRequired {
            actual: resource.revision(),
        });
    }
    Ok(())
}

pub fn validate_create_batch(resources: &[CanonicalResource]) -> Result<(), RepositoryError> {
    if resources.is_empty() {
        return Err(RepositoryError::EmptyCreateBatch);
    }
    let mut ids = std::collections::BTreeSet::new();
    for resource in resources {
        validate_create(resource)?;
        if !ids.insert(resource.id()) {
            return Err(RepositoryError::DuplicateBatchIdentity(resource.id()));
        }
    }
    Ok(())
}

pub fn validate_save(
    current: &ResourceState,
    expected_revision: Revision,
    incoming: &CanonicalResource,
) -> Result<(), RepositoryError> {
    incoming.validate()?;
    let ResourceState::Present(current) = current else {
        return Err(RepositoryError::ResourceDeleted(current.id()));
    };
    if current.id() != incoming.id() {
        return Err(RepositoryError::IdentityChanged {
            expected: current.id(),
            actual: incoming.id(),
        });
    }
    if current.kind() != incoming.kind() {
        return Err(RepositoryError::KindChanged {
            expected: current.kind(),
            actual: incoming.kind(),
        });
    }
    if current.revision() != expected_revision {
        return Err(RepositoryError::Conflict {
            expected: expected_revision,
            actual: current.revision(),
        });
    }
    let required = expected_revision
        .next()
        .map_err(mirabile_core::ResourceError::from)?;
    if incoming.revision() != required {
        return Err(RepositoryError::NonSequentialRevision {
            expected: required,
            actual: incoming.revision(),
        });
    }
    Ok(())
}

pub fn validate_delete(
    current: &ResourceState,
    expected_revision: Revision,
    deleted_at: Timestamp,
) -> Result<ResourceTombstone, RepositoryError> {
    if current.revision() != expected_revision {
        return Err(RepositoryError::Conflict {
            expected: expected_revision,
            actual: current.revision(),
        });
    }
    let ResourceState::Present(resource) = current else {
        return Err(RepositoryError::ResourceDeleted(current.id()));
    };
    Ok(ResourceTombstone {
        id: resource.id(),
        kind: resource.kind(),
        revision: resource
            .revision()
            .next()
            .map_err(mirabile_core::ResourceError::from)?,
        deleted_at,
    })
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("an atomic create batch must contain at least one resource")]
    EmptyCreateBatch,
    #[error("resource {0} occurs more than once in an atomic create batch")]
    DuplicateBatchIdentity(ResourceId),
    #[error("resource {0} already exists")]
    AlreadyExists(ResourceId),
    #[error("resource {0} was not found")]
    NotFound(ResourceId),
    #[error("resource {0} has been deleted and its stable ID cannot be reused")]
    ResourceDeleted(ResourceId),
    #[error("new resources must start at revision 1, got {actual}")]
    InitialRevisionRequired { actual: Revision },
    #[error("revision conflict: expected {expected}, current revision is {actual}")]
    Conflict {
        expected: Revision,
        actual: Revision,
    },
    #[error("next revision must be {expected}, got {actual}")]
    NonSequentialRevision {
        expected: Revision,
        actual: Revision,
    },
    #[error("resource identity changed from {expected} to {actual}")]
    IdentityChanged {
        expected: ResourceId,
        actual: ResourceId,
    },
    #[error("resource kind changed from {expected:?} to {actual:?}")]
    KindChanged {
        expected: ResourceKind,
        actual: ResourceKind,
    },
    #[error(transparent)]
    InvalidResource(#[from] mirabile_core::ResourceError),
    #[error("portable schema version {actual} is unsupported")]
    UnsupportedSchemaVersion { actual: u64 },
    #[error("portable resource serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("storage adapter failed: {0}")]
    Adapter(String),
}

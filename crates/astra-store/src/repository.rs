use astra_core::{CanonicalResource, ResourceId, ResourceKind, Revision};
use async_trait::async_trait;
use thiserror::Error;

#[async_trait(?Send)]
pub trait ResourceRepository {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError>;

    async fn save(
        &self,
        expected_revision: Revision,
        resource: CanonicalResource,
    ) -> Result<(), RepositoryError>;

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError>;

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<CanonicalResource>, RepositoryError>;

    async fn list(
        &self,
        kind: Option<ResourceKind>,
    ) -> Result<Vec<CanonicalResource>, RepositoryError>;

    async fn delete(&self, id: ResourceId) -> Result<(), RepositoryError>;
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

pub fn validate_save(
    current: &CanonicalResource,
    expected_revision: Revision,
    incoming: &CanonicalResource,
) -> Result<(), RepositoryError> {
    incoming.validate()?;
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
        .map_err(astra_core::ResourceError::from)?;
    if incoming.revision() != required {
        return Err(RepositoryError::NonSequentialRevision {
            expected: required,
            actual: incoming.revision(),
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("resource {0} already exists")]
    AlreadyExists(ResourceId),
    #[error("resource {0} was not found")]
    NotFound(ResourceId),
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
    InvalidResource(#[from] astra_core::ResourceError),
    #[error("portable resource serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("storage adapter failed: {0}")]
    Adapter(String),
}

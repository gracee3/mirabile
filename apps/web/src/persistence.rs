#[cfg(target_arch = "wasm32")]
use astra_core::{AspectSet, CanonicalResource, Command, ResourceEnvelope, Revision};
#[cfg(target_arch = "wasm32")]
use astra_store::{
    IndexedDbRepository, RepositoryError, ResourceRepository, execute_resource_command,
};

#[cfg(target_arch = "wasm32")]
const DATABASE_NAME: &str = "astra-foundation";

#[cfg(target_arch = "wasm32")]
pub async fn load_or_seed(
    seed: ResourceEnvelope<AspectSet>,
) -> Result<ResourceEnvelope<AspectSet>, RepositoryError> {
    let repository = IndexedDbRepository::open(DATABASE_NAME).await?;
    match repository.get(seed.id).await? {
        Some(CanonicalResource::AspectSet(resource)) => Ok(resource),
        Some(other) => Err(RepositoryError::KindChanged {
            expected: astra_core::ResourceKind::AspectSet,
            actual: other.kind(),
        }),
        None => {
            repository
                .create(CanonicalResource::AspectSet(seed.clone()))
                .await?;
            Ok(seed)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn save_command(
    base: ResourceEnvelope<AspectSet>,
    command: Command,
) -> Result<(), RepositoryError> {
    let repository = IndexedDbRepository::open(DATABASE_NAME).await?;
    if repository.get(base.id).await?.is_none() {
        repository
            .create(CanonicalResource::AspectSet(base.clone()))
            .await?;
    }
    execute_resource_command(&repository, command).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub const fn conflict_revision(error: &RepositoryError) -> Option<Revision> {
    match error {
        RepositoryError::Conflict { actual, .. } => Some(*actual),
        _ => None,
    }
}

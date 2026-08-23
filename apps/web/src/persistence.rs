#[cfg(target_arch = "wasm32")]
use astra_core::{AspectSet, CanonicalResource, Command, ResourceEnvelope, Revision};
#[cfg(target_arch = "wasm32")]
use astra_store::{
    IndexedDbRepository, RepositoryError, ResourceRepository, execute_resource_command,
};

#[cfg(target_arch = "wasm32")]
const DATABASE_NAME: &str = "astra-foundation";

#[cfg(target_arch = "wasm32")]
pub type BrowserRepository = IndexedDbRepository;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct BrowserRepository;

#[cfg(target_arch = "wasm32")]
pub async fn open_and_load(
    seed: ResourceEnvelope<AspectSet>,
) -> Result<(BrowserRepository, ResourceEnvelope<AspectSet>), RepositoryError> {
    let repository = IndexedDbRepository::open(DATABASE_NAME).await?;
    let canonical = match repository.get(seed.id).await? {
        Some(CanonicalResource::AspectSet(resource)) => resource,
        Some(other) => {
            return Err(RepositoryError::KindChanged {
                expected: astra_core::ResourceKind::AspectSet,
                actual: other.kind(),
            });
        }
        None => {
            repository
                .create(CanonicalResource::AspectSet(seed.clone()))
                .await?;
            seed
        }
    };
    Ok((repository, canonical))
}

#[cfg(target_arch = "wasm32")]
pub async fn save_command(
    repository: &BrowserRepository,
    command: Command,
) -> Result<(), RepositoryError> {
    execute_resource_command(repository, command).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub const fn conflict_revision(error: &RepositoryError) -> Option<Revision> {
    match error {
        RepositoryError::Conflict { actual, .. } => Some(*actual),
        _ => None,
    }
}

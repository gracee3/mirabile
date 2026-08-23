use mirabile_core::{Command, ResourceId, Revision};

use crate::{RepositoryError, ResourceRepository};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandOutcome {
    ResourceCreated { id: ResourceId, revision: Revision },
    ResourceSaved { id: ResourceId, revision: Revision },
}

pub async fn execute_resource_command<R: ResourceRepository>(
    repository: &R,
    command: Command,
) -> Result<CommandOutcome, RepositoryError> {
    match command {
        Command::CreateResource { resource } => {
            let outcome = CommandOutcome::ResourceCreated {
                id: resource.id(),
                revision: resource.revision(),
            };
            repository.create(resource).await?;
            Ok(outcome)
        }
        Command::SaveResourceDraft {
            expected_revision,
            resource,
        } => {
            let outcome = CommandOutcome::ResourceSaved {
                id: resource.id(),
                revision: resource.revision(),
            };
            repository.save(expected_revision, resource).await?;
            Ok(outcome)
        }
        _ => Err(RepositoryError::Adapter(
            "command requires a workspace application handler".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use mirabile_core::{CanonicalResource, PointSet, ResourceEnvelope, Timestamp};

    use super::*;
    use crate::MemoryRepository;

    #[test]
    fn resource_commands_use_repository_revision_rules() {
        block_on(async {
            let repository = MemoryRepository::default();
            let envelope = ResourceEnvelope::new(
                "Empty point set",
                PointSet { points: Vec::new() },
                Timestamp::from_unix_millis(0),
            );
            let id = envelope.id;
            let outcome = execute_resource_command(
                &repository,
                Command::CreateResource {
                    resource: CanonicalResource::PointSet(envelope),
                },
            )
            .await
            .expect("execute create");

            assert_eq!(
                outcome,
                CommandOutcome::ResourceCreated {
                    id,
                    revision: Revision::INITIAL,
                }
            );
        });
    }
}

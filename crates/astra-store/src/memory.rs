use std::{cell::RefCell, collections::BTreeMap};

use astra_core::{CanonicalResource, ResourceId, ResourceKind, Revision};
use async_trait::async_trait;

use crate::{RepositoryError, ResourceRepository, validate_create, validate_save};

#[derive(Clone, Debug, Default)]
pub struct MemoryRepository {
    current: RefCell<BTreeMap<ResourceId, CanonicalResource>>,
    history: RefCell<BTreeMap<(ResourceId, Revision), CanonicalResource>>,
}

impl MemoryRepository {
    pub fn current_count(&self) -> usize {
        self.current.borrow().len()
    }

    pub fn revision_count(&self) -> usize {
        self.history.borrow().len()
    }
}

#[async_trait(?Send)]
impl ResourceRepository for MemoryRepository {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError> {
        validate_create(&resource)?;
        let id = resource.id();
        if self.current.borrow().contains_key(&id) {
            return Err(RepositoryError::AlreadyExists(id));
        }
        self.history
            .borrow_mut()
            .insert((id, resource.revision()), resource.clone());
        self.current.borrow_mut().insert(id, resource);
        Ok(())
    }

    async fn save(
        &self,
        expected_revision: Revision,
        resource: CanonicalResource,
    ) -> Result<(), RepositoryError> {
        let id = resource.id();
        {
            let current = self.current.borrow();
            let existing = current.get(&id).ok_or(RepositoryError::NotFound(id))?;
            validate_save(existing, expected_revision, &resource)?;
        }
        self.history
            .borrow_mut()
            .insert((id, resource.revision()), resource.clone());
        self.current.borrow_mut().insert(id, resource);
        Ok(())
    }

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError> {
        Ok(self.current.borrow().get(&id).cloned())
    }

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<CanonicalResource>, RepositoryError> {
        Ok(self.history.borrow().get(&(id, revision)).cloned())
    }

    async fn list(
        &self,
        kind: Option<ResourceKind>,
    ) -> Result<Vec<CanonicalResource>, RepositoryError> {
        let mut resources: Vec<_> = self
            .current
            .borrow()
            .values()
            .filter(|resource| kind.is_none_or(|expected| resource.kind() == expected))
            .cloned()
            .collect();
        resources.sort_by_key(CanonicalResource::id);
        Ok(resources)
    }

    async fn delete(&self, id: ResourceId) -> Result<(), RepositoryError> {
        if self.current.borrow_mut().remove(&id).is_none() {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astra_core::{PointId, PointSelector, PointSet, ResourceEnvelope, Timestamp};
    use futures::executor::block_on;

    use super::*;

    fn point_resource() -> CanonicalResource {
        CanonicalResource::PointSet(ResourceEnvelope::new(
            "Visible points",
            PointSet {
                points: vec![PointSelector::Point(
                    PointId::new("sun").expect("valid point ID"),
                )],
            },
            Timestamp::from_unix_millis(0),
        ))
    }

    #[test]
    fn preserves_history_and_rejects_stale_writes() {
        block_on(async {
            let repository = MemoryRepository::default();
            let first = point_resource();
            let id = first.id();
            repository.create(first.clone()).await.expect("create");
            let CanonicalResource::PointSet(first_envelope) = first else {
                panic!("point set")
            };
            let mut next_envelope = first_envelope
                .next_with_payload(
                    PointSet { points: Vec::new() },
                    Timestamp::from_unix_millis(1),
                )
                .expect("next revision");
            next_envelope.title = "No visible points".into();
            let next = CanonicalResource::PointSet(next_envelope);
            repository
                .save(Revision::INITIAL, next.clone())
                .await
                .expect("save");

            assert_eq!(repository.current_count(), 1);
            assert_eq!(repository.revision_count(), 2);
            assert_eq!(
                repository
                    .get_revision(id, Revision::INITIAL)
                    .await
                    .expect("read")
                    .expect("revision exists")
                    .title(),
                "Visible points"
            );
            let stale = repository.save(Revision::INITIAL, next).await;
            assert!(matches!(stale, Err(RepositoryError::Conflict { .. })));
        });
    }
}

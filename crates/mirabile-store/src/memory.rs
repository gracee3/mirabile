use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use async_trait::async_trait;
use mirabile_core::{CanonicalResource, ResourceId, ResourceKind, Revision, Timestamp};

use crate::{
    AtomicSaveBatch, RepositoryError, ResourceRepository, ResourceState, ResourceTombstone,
    RevisionConflict, validate_create, validate_create_batch, validate_delete, validate_save,
    validate_save_batch,
};

#[derive(Clone, Debug, Default)]
pub struct MemoryRepository {
    state: Rc<RefCell<MemoryState>>,
}

#[derive(Clone, Debug, Default)]
struct MemoryState {
    current: BTreeMap<ResourceId, ResourceState>,
    history: BTreeMap<(ResourceId, Revision), ResourceState>,
}

impl MemoryRepository {
    pub fn current_count(&self) -> usize {
        self.state.borrow().current.len()
    }

    pub fn revision_count(&self) -> usize {
        self.state.borrow().history.len()
    }
}

#[async_trait(?Send)]
impl ResourceRepository for MemoryRepository {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError> {
        validate_create(&resource)?;
        let id = resource.id();
        let mut state = self.state.borrow_mut();
        if state.current.contains_key(&id) {
            return Err(RepositoryError::AlreadyExists(id));
        }
        let resource = ResourceState::Present(resource);
        state
            .history
            .insert((id, resource.revision()), resource.clone());
        state.current.insert(id, resource);
        Ok(())
    }

    async fn create_batch(&self, resources: Vec<CanonicalResource>) -> Result<(), RepositoryError> {
        validate_create_batch(&resources)?;
        let mut state = self.state.borrow_mut();
        for resource in &resources {
            if state.current.contains_key(&resource.id()) {
                return Err(RepositoryError::AlreadyExists(resource.id()));
            }
        }
        for resource in resources {
            let id = resource.id();
            let resource = ResourceState::Present(resource);
            state
                .history
                .insert((id, resource.revision()), resource.clone());
            state.current.insert(id, resource);
        }
        Ok(())
    }

    async fn save(
        &self,
        expected_revision: Revision,
        resource: CanonicalResource,
    ) -> Result<(), RepositoryError> {
        let id = resource.id();
        let mut state = self.state.borrow_mut();
        let existing = state
            .current
            .get(&id)
            .ok_or(RepositoryError::NotFound(id))?;
        validate_save(existing, expected_revision, &resource)?;
        let resource = ResourceState::Present(resource);
        state
            .history
            .insert((id, resource.revision()), resource.clone());
        state.current.insert(id, resource);
        Ok(())
    }

    async fn save_batch(&self, batch: AtomicSaveBatch) -> Result<(), RepositoryError> {
        validate_save_batch(&batch)?;
        let mut state = self.state.borrow_mut();
        let mut conflicts = Vec::new();
        for expectation in &batch.expectations {
            let current = state
                .current
                .get(&expectation.id)
                .ok_or(RepositoryError::NotFound(expectation.id))?;
            if current.revision() != expectation.expected_revision {
                conflicts.push(RevisionConflict {
                    id: expectation.id,
                    expected: expectation.expected_revision,
                    actual: current.revision(),
                });
            } else if matches!(current, ResourceState::Deleted(_)) {
                return Err(RepositoryError::ResourceDeleted(expectation.id));
            }
        }
        if !conflicts.is_empty() {
            return Err(RepositoryError::BatchConflict { conflicts });
        }
        for resource in &batch.changes {
            let expectation = batch
                .expectations
                .iter()
                .find(|expectation| expectation.id == resource.id())
                .expect("batch structure was validated");
            let current = state
                .current
                .get(&resource.id())
                .expect("expected head was preflighted");
            validate_save(current, expectation.expected_revision, resource)?;
        }
        for resource in batch.changes {
            let id = resource.id();
            let resource = ResourceState::Present(resource);
            state
                .history
                .insert((id, resource.revision()), resource.clone());
            state.current.insert(id, resource);
        }
        Ok(())
    }

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError> {
        Ok(match self.state.borrow().current.get(&id) {
            Some(ResourceState::Present(resource)) => Some(resource.clone()),
            Some(ResourceState::Deleted(_)) | None => None,
        })
    }

    async fn get_head(&self, id: ResourceId) -> Result<Option<ResourceState>, RepositoryError> {
        Ok(self.state.borrow().current.get(&id).cloned())
    }

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<ResourceState>, RepositoryError> {
        Ok(self.state.borrow().history.get(&(id, revision)).cloned())
    }

    async fn list(
        &self,
        kind: Option<ResourceKind>,
    ) -> Result<Vec<CanonicalResource>, RepositoryError> {
        let mut resources: Vec<_> = self
            .state
            .borrow()
            .current
            .values()
            .filter_map(|state| match state {
                ResourceState::Present(resource)
                    if kind.is_none_or(|expected| resource.kind() == expected) =>
                {
                    Some(resource.clone())
                }
                ResourceState::Present(_) | ResourceState::Deleted(_) => None,
            })
            .collect();
        resources.sort_by_key(CanonicalResource::id);
        Ok(resources)
    }

    async fn delete(
        &self,
        id: ResourceId,
        expected_revision: Revision,
        deleted_at: Timestamp,
    ) -> Result<ResourceTombstone, RepositoryError> {
        let mut state = self.state.borrow_mut();
        let current = state
            .current
            .get(&id)
            .ok_or(RepositoryError::NotFound(id))?;
        let tombstone = validate_delete(current, expected_revision, deleted_at)?;
        let deleted = ResourceState::Deleted(tombstone.clone());
        state
            .history
            .insert((id, tombstone.revision), deleted.clone());
        state.current.insert(id, deleted);
        Ok(tombstone)
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use mirabile_core::{PointId, PointSelector, PointSet, ResourceEnvelope, Timestamp};

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

    fn next_point_resource(
        resource: &CanonicalResource,
        title: &str,
        timestamp: i64,
    ) -> CanonicalResource {
        let CanonicalResource::PointSet(envelope) = resource else {
            panic!("point set")
        };
        let mut next = envelope
            .next_with_payload(
                envelope.payload.clone(),
                Timestamp::from_unix_millis(timestamp),
            )
            .expect("next revision");
        next.title = title.into();
        CanonicalResource::PointSet(next)
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
            let historical = repository
                .get_revision(id, Revision::INITIAL)
                .await
                .expect("read")
                .expect("revision exists");
            let ResourceState::Present(historical) = historical else {
                panic!("live historical revision")
            };
            assert_eq!(historical.title(), "Visible points");
            let stale = repository.save(Revision::INITIAL, next).await;
            assert!(matches!(stale, Err(RepositoryError::Conflict { .. })));
        });
    }

    #[test]
    fn deletion_is_versioned_hidden_and_permanent_for_the_stable_id() {
        block_on(async {
            let repository = MemoryRepository::default();
            let resource = point_resource();
            let id = resource.id();
            repository
                .create(resource.clone())
                .await
                .expect("create resource");

            let stale = repository
                .delete(
                    id,
                    Revision::new(2).expect("valid revision"),
                    Timestamp::from_unix_millis(10),
                )
                .await;
            assert!(matches!(stale, Err(RepositoryError::Conflict { .. })));

            let tombstone = repository
                .delete(id, Revision::INITIAL, Timestamp::from_unix_millis(10))
                .await
                .expect("delete resource");
            assert_eq!(tombstone.revision.get(), 2);
            assert!(repository.get(id).await.expect("live read").is_none());
            assert!(repository.list(None).await.expect("list").is_empty());
            assert!(matches!(
                repository.get_head(id).await.expect("head"),
                Some(ResourceState::Deleted(ref value)) if value == &tombstone
            ));
            assert!(matches!(
                repository
                    .get_revision(id, Revision::INITIAL)
                    .await
                    .expect("history"),
                Some(ResourceState::Present(_))
            ));
            assert!(matches!(
                repository
                    .get_revision(id, tombstone.revision)
                    .await
                    .expect("history"),
                Some(ResourceState::Deleted(ref value)) if value == &tombstone
            ));
            let CanonicalResource::PointSet(envelope) = resource.clone() else {
                panic!("point set")
            };
            let after_delete = CanonicalResource::PointSet(
                envelope
                    .next_with_payload(
                        PointSet { points: Vec::new() },
                        Timestamp::from_unix_millis(11),
                    )
                    .expect("next live revision"),
            );
            assert!(matches!(
                repository
                    .save(Revision::INITIAL, after_delete)
                    .await,
                Err(RepositoryError::ResourceDeleted(value)) if value == id
            ));
            assert!(matches!(
                repository.create(resource).await,
                Err(RepositoryError::AlreadyExists(value)) if value == id
            ));
        });
    }

    #[test]
    fn invalid_payloads_are_rejected_on_create_and_save() {
        block_on(async {
            let repository = MemoryRepository::default();
            let mut invalid = point_resource();
            let CanonicalResource::PointSet(envelope) = &mut invalid else {
                panic!("point set")
            };
            envelope.tags = vec!["duplicate".into(), "duplicate".into()];
            assert!(matches!(
                repository.create(invalid).await,
                Err(RepositoryError::InvalidResource(_))
            ));

            let first = point_resource();
            repository
                .create(first.clone())
                .await
                .expect("valid create");
            let CanonicalResource::PointSet(envelope) = first else {
                panic!("point set")
            };
            let mut invalid_next = envelope
                .next_with_payload(
                    PointSet {
                        points: vec![
                            PointSelector::Point(PointId::new("sun").expect("valid ID")),
                            PointSelector::Point(PointId::new("sun").expect("valid ID")),
                        ],
                    },
                    Timestamp::from_unix_millis(1),
                )
                .expect("next revision");
            invalid_next.title = "Invalid duplicate points".into();
            assert!(matches!(
                repository
                    .save(Revision::INITIAL, CanonicalResource::PointSet(invalid_next))
                    .await,
                Err(RepositoryError::InvalidResource(_))
            ));
        });
    }

    #[test]
    fn create_batch_is_all_or_nothing() {
        block_on(async {
            let repository = MemoryRepository::default();
            let first = point_resource();
            let existing = point_resource();
            let first_id = first.id();
            repository
                .create(existing.clone())
                .await
                .expect("existing resource");

            assert!(matches!(
                repository.create_batch(vec![first, existing]).await,
                Err(RepositoryError::AlreadyExists(_))
            ));
            assert_eq!(repository.current_count(), 1);
            assert_eq!(repository.revision_count(), 1);
            assert!(repository.get(first_id).await.expect("read").is_none());

            let duplicate = point_resource();
            assert!(matches!(
                repository
                    .create_batch(vec![duplicate.clone(), duplicate])
                    .await,
                Err(RepositoryError::DuplicateBatchIdentity(_))
            ));
            assert_eq!(repository.current_count(), 1);
        });
    }

    #[test]
    fn save_batch_compare_only_conflict_publishes_nothing() {
        block_on(async {
            let repository = MemoryRepository::default();
            let changed_base = point_resource();
            let compare_base = point_resource();
            repository
                .create_batch(vec![changed_base.clone(), compare_base.clone()])
                .await
                .expect("create bases");
            let compare_next = next_point_resource(&compare_base, "Remote compare head", 1);
            repository
                .save(Revision::INITIAL, compare_next.clone())
                .await
                .expect("advance compare-only head");
            let changed_next = next_point_resource(&changed_base, "Local changed head", 2);

            let result = repository
                .save_batch(AtomicSaveBatch {
                    expectations: vec![
                        crate::RevisionExpectation {
                            id: changed_base.id(),
                            expected_revision: Revision::INITIAL,
                        },
                        crate::RevisionExpectation {
                            id: compare_base.id(),
                            expected_revision: Revision::INITIAL,
                        },
                    ],
                    changes: vec![changed_next],
                })
                .await;

            assert!(matches!(
                result,
                Err(RepositoryError::BatchConflict { conflicts })
                    if conflicts == vec![crate::RevisionConflict {
                        id: compare_base.id(),
                        expected: Revision::INITIAL,
                        actual: compare_next.revision(),
                    }]
            ));
            assert_eq!(
                repository
                    .get(changed_base.id())
                    .await
                    .expect("read changed base"),
                Some(changed_base)
            );
            assert_eq!(repository.revision_count(), 3);
        });
    }

    #[test]
    fn save_batch_collects_conflicts_and_rejects_malformed_batches() {
        block_on(async {
            let repository = MemoryRepository::default();
            let first = point_resource();
            let second = point_resource();
            repository
                .create_batch(vec![first.clone(), second.clone()])
                .await
                .expect("create bases");
            repository
                .save(
                    Revision::INITIAL,
                    next_point_resource(&first, "First remote", 1),
                )
                .await
                .expect("advance first");
            repository
                .save(
                    Revision::INITIAL,
                    next_point_resource(&second, "Second remote", 2),
                )
                .await
                .expect("advance second");

            let conflicts = repository
                .save_batch(AtomicSaveBatch {
                    expectations: vec![
                        crate::RevisionExpectation {
                            id: first.id(),
                            expected_revision: Revision::INITIAL,
                        },
                        crate::RevisionExpectation {
                            id: second.id(),
                            expected_revision: Revision::INITIAL,
                        },
                    ],
                    changes: Vec::new(),
                })
                .await;
            assert!(matches!(
                conflicts,
                Err(RepositoryError::BatchConflict { conflicts }) if conflicts.len() == 2
            ));

            let duplicate = crate::RevisionExpectation {
                id: first.id(),
                expected_revision: Revision::new(2).expect("revision two"),
            };
            assert!(matches!(
                repository
                    .save_batch(AtomicSaveBatch {
                        expectations: vec![duplicate, duplicate],
                        changes: Vec::new(),
                    })
                    .await,
                Err(RepositoryError::DuplicateBatchIdentity(id)) if id == first.id()
            ));
            assert!(matches!(
                repository
                    .save_batch(AtomicSaveBatch {
                        expectations: vec![duplicate],
                        changes: vec![next_point_resource(&second, "Unmatched", 3)],
                    })
                    .await,
                Err(RepositoryError::MissingBatchExpectation(id)) if id == second.id()
            ));
            assert_eq!(repository.revision_count(), 4);
        });
    }
}

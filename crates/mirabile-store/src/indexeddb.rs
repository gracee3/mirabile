use std::rc::Rc;

use async_trait::async_trait;
use mirabile_core::{CanonicalResource, ResourceId, ResourceKind, Revision, Timestamp};
use rexie::{ObjectStore, Rexie, TransactionMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::{
    RepositoryError, ResourceRepository, ResourceState, ResourceTombstone, resource_from_json,
    resource_to_json, validate_create, validate_create_batch, validate_delete, validate_save,
};

const CURRENT_STORE: &str = "resources";
const REVISION_STORE: &str = "resource_revisions";

#[derive(Clone, Debug)]
pub struct IndexedDbRepository {
    database: Rc<Rexie>,
}

impl IndexedDbRepository {
    pub async fn open(database_name: &str) -> Result<Self, RepositoryError> {
        let database = Rexie::builder(database_name)
            .version(1)
            .add_object_store(ObjectStore::new(CURRENT_STORE))
            .add_object_store(ObjectStore::new(REVISION_STORE))
            .build()
            .await
            .map_err(adapter_error)?;
        Ok(Self {
            database: Rc::new(database),
        })
    }

    #[cfg(feature = "browser-contract")]
    pub async fn force_history_key_collision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<(), RepositoryError> {
        let state = self
            .get_head(id)
            .await?
            .ok_or(RepositoryError::NotFound(id))?;
        let transaction = self
            .database
            .transaction(&[REVISION_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let store = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        let json = JsValue::from_str(&state_to_storage_json(&state)?);
        store
            .add(&json, Some(&JsValue::from_str(&revision_key(id, revision))))
            .await
            .map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }

    #[cfg(feature = "browser-contract")]
    pub async fn force_initial_history_collision(
        &self,
        id: ResourceId,
    ) -> Result<(), RepositoryError> {
        let transaction = self
            .database
            .transaction(&[REVISION_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let store = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        store
            .add(
                &JsValue::from_str("forced collision"),
                Some(&JsValue::from_str(&revision_key(id, Revision::INITIAL))),
            )
            .await
            .map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl ResourceRepository for IndexedDbRepository {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError> {
        validate_create(&resource)?;
        let id = resource.id();
        let current_key = JsValue::from_str(&id.to_string());
        let revision_key = JsValue::from_str(&revision_key(id, resource.revision()));
        let json = JsValue::from_str(&resource_to_json(&resource)?);
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE, REVISION_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let current = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        if current
            .key_exists(current_key.clone())
            .await
            .map_err(adapter_error)?
        {
            return Err(RepositoryError::AlreadyExists(id));
        }
        let revisions = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        if let Err(error) = current.add(&json, Some(&current_key)).await {
            let _ = transaction.abort().await;
            return Err(adapter_error(error));
        }
        if let Err(error) = revisions.add(&json, Some(&revision_key)).await {
            let _ = transaction.abort().await;
            return Err(adapter_error(error));
        }
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }

    async fn create_batch(&self, resources: Vec<CanonicalResource>) -> Result<(), RepositoryError> {
        validate_create_batch(&resources)?;
        let serialized = resources
            .iter()
            .map(|resource| resource_to_json(resource).map(|json| (resource.id(), json)))
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE, REVISION_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let current = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        for (id, _) in &serialized {
            if current
                .key_exists(JsValue::from_str(&id.to_string()))
                .await
                .map_err(adapter_error)?
            {
                let _ = transaction.abort().await;
                return Err(RepositoryError::AlreadyExists(*id));
            }
        }
        let revisions = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        for (id, json) in serialized {
            let value = JsValue::from_str(&json);
            if let Err(error) = current
                .add(&value, Some(&JsValue::from_str(&id.to_string())))
                .await
            {
                let _ = transaction.abort().await;
                return Err(adapter_error(error));
            }
            if let Err(error) = revisions
                .add(
                    &value,
                    Some(&JsValue::from_str(&revision_key(id, Revision::INITIAL))),
                )
                .await
            {
                let _ = transaction.abort().await;
                return Err(adapter_error(error));
            }
        }
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }

    async fn save(
        &self,
        expected_revision: Revision,
        resource: CanonicalResource,
    ) -> Result<(), RepositoryError> {
        let id = resource.id();
        let current_key = JsValue::from_str(&id.to_string());
        let revision_key = JsValue::from_str(&revision_key(id, resource.revision()));
        let json = JsValue::from_str(&resource_to_json(&resource)?);
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE, REVISION_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let current_store = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        let current_value = current_store
            .get(current_key.clone())
            .await
            .map_err(adapter_error)?
            .ok_or(RepositoryError::NotFound(id))?;
        let current = state_from_js_string(&current_value)?;
        validate_save(&current, expected_revision, &resource)?;
        let revision_store = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        // Put current first deliberately. A subsequent history-key failure must
        // abort this whole transaction, which the browser contract verifies.
        if let Err(error) = current_store.put(&json, Some(&current_key)).await {
            let _ = transaction.abort().await;
            return Err(adapter_error(error));
        }
        if let Err(error) = revision_store.add(&json, Some(&revision_key)).await {
            let _ = transaction.abort().await;
            return Err(adapter_error(error));
        }
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError> {
        Ok(match self.get_head(id).await? {
            Some(ResourceState::Present(resource)) => Some(resource),
            Some(ResourceState::Deleted(_)) | None => None,
        })
    }

    async fn get_head(&self, id: ResourceId) -> Result<Option<ResourceState>, RepositoryError> {
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE], TransactionMode::ReadOnly)
            .map_err(adapter_error)?;
        let store = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        let value = store
            .get(JsValue::from_str(&id.to_string()))
            .await
            .map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        value.as_ref().map(state_from_js_string).transpose()
    }

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<ResourceState>, RepositoryError> {
        let transaction = self
            .database
            .transaction(&[REVISION_STORE], TransactionMode::ReadOnly)
            .map_err(adapter_error)?;
        let store = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        let value = store
            .get(JsValue::from_str(&revision_key(id, revision)))
            .await
            .map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        value.as_ref().map(state_from_js_string).transpose()
    }

    async fn list(
        &self,
        kind: Option<ResourceKind>,
    ) -> Result<Vec<CanonicalResource>, RepositoryError> {
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE], TransactionMode::ReadOnly)
            .map_err(adapter_error)?;
        let store = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        let values = store.get_all(None, None).await.map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        let mut resources = values
            .into_iter()
            .map(|value| state_from_js_string(&value))
            .filter_map(|result| match result {
                Ok(ResourceState::Present(resource)) => Some(Ok(resource)),
                Ok(ResourceState::Deleted(_)) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        resources.retain(|resource| kind.is_none_or(|expected| resource.kind() == expected));
        resources.sort_by_key(CanonicalResource::id);
        Ok(resources)
    }

    async fn delete(
        &self,
        id: ResourceId,
        expected_revision: Revision,
        deleted_at: Timestamp,
    ) -> Result<ResourceTombstone, RepositoryError> {
        let current_key = JsValue::from_str(&id.to_string());
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE, REVISION_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let current_store = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        let current_value = current_store
            .get(current_key.clone())
            .await
            .map_err(adapter_error)?
            .ok_or(RepositoryError::NotFound(id))?;
        let current = state_from_js_string(&current_value)?;
        let tombstone = validate_delete(&current, expected_revision, deleted_at)?;
        let deleted = ResourceState::Deleted(tombstone.clone());
        let json = JsValue::from_str(&state_to_storage_json(&deleted)?);
        let history_key = JsValue::from_str(&revision_key(id, tombstone.revision));
        let revision_store = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        if let Err(error) = current_store.put(&json, Some(&current_key)).await {
            let _ = transaction.abort().await;
            return Err(adapter_error(error));
        }
        if let Err(error) = revision_store.add(&json, Some(&history_key)).await {
            let _ = transaction.abort().await;
            return Err(adapter_error(error));
        }
        transaction.done().await.map_err(adapter_error)?;
        Ok(tombstone)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "storage_type", content = "value", rename_all = "snake_case")]
enum StorageEnvelope {
    Tombstone(ResourceTombstone),
}

fn revision_key(id: ResourceId, revision: Revision) -> String {
    format!("{id}@{revision}")
}

fn state_to_storage_json(state: &ResourceState) -> Result<String, RepositoryError> {
    match state {
        ResourceState::Present(resource) => resource_to_json(resource),
        ResourceState::Deleted(tombstone) => Ok(serde_json::to_string(
            &StorageEnvelope::Tombstone(tombstone.clone()),
        )?),
    }
}

fn state_from_js_string(value: &JsValue) -> Result<ResourceState, RepositoryError> {
    let json = value
        .as_string()
        .ok_or_else(|| RepositoryError::Adapter("IndexedDB value is not JSON text".into()))?;
    let probe: serde_json::Value = serde_json::from_str(&json)?;
    if probe.get("storage_type").is_some() {
        return match serde_json::from_value::<StorageEnvelope>(probe)? {
            StorageEnvelope::Tombstone(tombstone) => Ok(ResourceState::Deleted(tombstone)),
        };
    }
    resource_from_json(&json).map(ResourceState::Present)
}

fn adapter_error(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Adapter(error.to_string())
}

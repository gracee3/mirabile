use astra_core::{CanonicalResource, ResourceId, ResourceKind, Revision};
use async_trait::async_trait;
use rexie::{ObjectStore, Rexie, TransactionMode};
use wasm_bindgen::JsValue;

use crate::{
    RepositoryError, ResourceRepository, resource_from_json, resource_to_json, validate_create,
    validate_save,
};

const CURRENT_STORE: &str = "resources";
const REVISION_STORE: &str = "resource_revisions";

pub struct IndexedDbRepository {
    database: Rexie,
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
        Ok(Self { database })
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
        current
            .add(&json, Some(&current_key))
            .await
            .map_err(adapter_error)?;
        revisions
            .add(&json, Some(&revision_key))
            .await
            .map_err(adapter_error)?;
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
        let current = from_js_string(current_value)?;
        validate_save(&current, expected_revision, &resource)?;
        let revision_store = transaction.store(REVISION_STORE).map_err(adapter_error)?;
        current_store
            .put(&json, Some(&current_key))
            .await
            .map_err(adapter_error)?;
        revision_store
            .add(&json, Some(&revision_key))
            .await
            .map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError> {
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
        value.map(from_js_string).transpose()
    }

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<CanonicalResource>, RepositoryError> {
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
        value.map(from_js_string).transpose()
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
            .map(from_js_string)
            .collect::<Result<Vec<_>, _>>()?;
        resources.retain(|resource| kind.is_none_or(|expected| resource.kind() == expected));
        resources.sort_by_key(CanonicalResource::id);
        Ok(resources)
    }

    async fn delete(&self, id: ResourceId) -> Result<(), RepositoryError> {
        let key = JsValue::from_str(&id.to_string());
        let transaction = self
            .database
            .transaction(&[CURRENT_STORE], TransactionMode::ReadWrite)
            .map_err(adapter_error)?;
        let store = transaction.store(CURRENT_STORE).map_err(adapter_error)?;
        if !store.key_exists(key.clone()).await.map_err(adapter_error)? {
            return Err(RepositoryError::NotFound(id));
        }
        store.delete(key).await.map_err(adapter_error)?;
        transaction.done().await.map_err(adapter_error)?;
        Ok(())
    }
}

fn revision_key(id: ResourceId, revision: Revision) -> String {
    format!("{id}@{revision}")
}

fn from_js_string(value: JsValue) -> Result<CanonicalResource, RepositoryError> {
    let json = value
        .as_string()
        .ok_or_else(|| RepositoryError::Adapter("IndexedDB value is not JSON text".into()))?;
    resource_from_json(&json)
}

fn adapter_error(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Adapter(error.to_string())
}

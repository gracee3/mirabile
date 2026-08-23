use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

#[cfg(feature = "xalen-backend")]
use astra_engine::XalenBackend;
use astra_engine::{
    BackendDescriptor, CalculationOutcome, CalculationWorkerRequest, CalculationWorkerResult,
    DeterministicBackend, ImplementationIdentity,
};
use async_trait::async_trait;
use js_sys::Array;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Blob, BlobPropertyBag, ErrorEvent, MessageEvent, Url, Worker};

use crate::{CalculationRuntime, CalculationRuntimeError, RuntimeInbox};

const WORKER_READY: &str = "ASTRA_CALCULATION_WORKER_READY_V1";

#[derive(Clone)]
pub struct WorkerCalculationRuntime {
    inner: Rc<WorkerRuntimeInner>,
}

struct WorkerRuntimeInner {
    descriptor: BackendDescriptor,
    worker: Option<Worker>,
    startup_error: Option<CalculationRuntimeError>,
    state: Rc<RefCell<WorkerTransportState>>,
    inbox: RuntimeInbox,
    completed_results: Rc<Cell<u64>>,
    last_backend: Rc<RefCell<Option<ImplementationIdentity>>>,
    _onmessage: Option<Closure<dyn FnMut(MessageEvent)>>,
    _onerror: Option<Closure<dyn FnMut(ErrorEvent)>>,
}

#[derive(Default)]
struct WorkerTransportState {
    ready: bool,
    queued_requests: VecDeque<String>,
}

impl WorkerCalculationRuntime {
    pub fn deterministic() -> Self {
        let descriptor = astra_engine::CalculationBackend::descriptor(&DeterministicBackend);
        match create_worker() {
            Ok(worker) => Self::with_worker(descriptor, worker),
            Err(error) => Self {
                inner: Rc::new(WorkerRuntimeInner {
                    descriptor,
                    worker: None,
                    startup_error: Some(error),
                    state: Rc::new(RefCell::new(WorkerTransportState::default())),
                    inbox: RuntimeInbox::default(),
                    completed_results: Rc::new(Cell::new(0)),
                    last_backend: Rc::new(RefCell::new(None)),
                    _onmessage: None,
                    _onerror: None,
                }),
            },
        }
    }

    #[cfg(feature = "xalen-backend")]
    pub fn xalen() -> Self {
        let descriptor = astra_engine::CalculationBackend::descriptor(&XalenBackend);
        match create_worker() {
            Ok(worker) => Self::with_worker(descriptor, worker),
            Err(error) => Self {
                inner: Rc::new(WorkerRuntimeInner {
                    descriptor,
                    worker: None,
                    startup_error: Some(error),
                    state: Rc::new(RefCell::new(WorkerTransportState::default())),
                    inbox: RuntimeInbox::default(),
                    completed_results: Rc::new(Cell::new(0)),
                    last_backend: Rc::new(RefCell::new(None)),
                    _onmessage: None,
                    _onerror: None,
                }),
            },
        }
    }

    fn with_worker(descriptor: BackendDescriptor, worker: Worker) -> Self {
        let state = Rc::new(RefCell::new(WorkerTransportState::default()));
        let inbox = RuntimeInbox::default();
        let completed_results = Rc::new(Cell::new(0_u64));
        let last_backend = Rc::new(RefCell::new(None));

        let message_worker = worker.clone();
        let message_state = Rc::clone(&state);
        let message_inbox = inbox.clone();
        let message_count = Rc::clone(&completed_results);
        let message_backend = Rc::clone(&last_backend);
        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            let Some(message) = event.data().as_string() else {
                message_inbox.push(Err(CalculationRuntimeError::new(
                    "calculation worker returned a non-string transport message",
                )));
                return;
            };
            if message == WORKER_READY {
                let queued = {
                    let mut state = message_state.borrow_mut();
                    state.ready = true;
                    std::mem::take(&mut state.queued_requests)
                };
                for request in queued {
                    if let Err(error) = message_worker.post_message(&JsValue::from_str(&request)) {
                        message_inbox.push(Err(js_error(
                            "could not post queued calculation request",
                            &error,
                        )));
                    }
                }
                return;
            }
            match serde_json::from_str::<CalculationWorkerResult>(&message) {
                Ok(result) => {
                    if let CalculationOutcome::Success(value) = &result.outcome {
                        message_backend.replace(Some(value.provenance.backend.clone()));
                    }
                    message_count.set(message_count.get().saturating_add(1));
                    message_inbox.push(Ok(result));
                }
                Err(error) => message_inbox.push(Err(CalculationRuntimeError::new(format!(
                    "could not decode calculation worker result: {error}"
                )))),
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let error_inbox = inbox.clone();
        let onerror = Closure::wrap(Box::new(move |event: ErrorEvent| {
            error_inbox.push(Err(CalculationRuntimeError::new(format!(
                "calculation worker execution failed: {}",
                event.message()
            ))));
        }) as Box<dyn FnMut(ErrorEvent)>);
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Self {
            inner: Rc::new(WorkerRuntimeInner {
                descriptor,
                worker: Some(worker),
                startup_error: None,
                state,
                inbox,
                completed_results,
                last_backend,
                _onmessage: Some(onmessage),
                _onerror: Some(onerror),
            }),
        }
    }

    pub fn completed_results(&self) -> u64 {
        self.inner.completed_results.get()
    }

    pub fn last_backend_identity(&self) -> Option<ImplementationIdentity> {
        self.inner.last_backend.borrow().clone()
    }
}

#[async_trait(?Send)]
impl CalculationRuntime for WorkerCalculationRuntime {
    fn backend_descriptor(&self) -> BackendDescriptor {
        self.inner.descriptor.clone()
    }

    fn submit(&self, request: CalculationWorkerRequest) -> Result<(), CalculationRuntimeError> {
        if let Some(error) = &self.inner.startup_error {
            return Err(error.clone());
        }
        let worker = self
            .inner
            .worker
            .as_ref()
            .ok_or_else(|| CalculationRuntimeError::new("calculation worker is unavailable"))?;
        let encoded = serde_json::to_string(&request).map_err(|error| {
            CalculationRuntimeError::new(format!(
                "could not encode calculation worker request: {error}"
            ))
        })?;
        let mut state = self.inner.state.borrow_mut();
        if state.ready {
            worker
                .post_message(&JsValue::from_str(&encoded))
                .map_err(|error| js_error("could not post calculation request", &error))
        } else {
            state.queued_requests.push_back(encoded);
            Ok(())
        }
    }

    async fn receive(&self) -> Result<CalculationWorkerResult, CalculationRuntimeError> {
        self.inner.inbox.receive().await
    }
}

fn create_worker() -> Result<Worker, CalculationRuntimeError> {
    let window = web_sys::window()
        .ok_or_else(|| CalculationRuntimeError::new("browser window is unavailable"))?;
    let page = window
        .location()
        .href()
        .map_err(|error| js_error("could not read browser location", &error))?;
    let base = Url::new_with_base("./", &page)
        .map_err(|error| js_error("could not resolve worker base URL", &error))?
        .href();
    let source = format!(
        "importScripts(\"{base}calculation-worker.js\");wasm_bindgen(\"{base}calculation-worker_bg.wasm\");"
    );
    let sequence = Array::new();
    sequence.push(&JsValue::from_str(&source));
    let options = BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = Blob::new_with_str_sequence_and_options(&sequence, &options)
        .map_err(|error| js_error("could not create calculation worker script", &error))?;
    let object_url = Url::create_object_url_with_blob(&blob)
        .map_err(|error| js_error("could not create calculation worker URL", &error))?;
    let worker = Worker::new(&object_url)
        .map_err(|error| js_error("could not start calculation worker", &error))?;
    Url::revoke_object_url(&object_url)
        .map_err(|error| js_error("could not revoke calculation worker URL", &error))?;
    Ok(worker)
}

fn js_error(context: &str, value: &JsValue) -> CalculationRuntimeError {
    CalculationRuntimeError::new(format!("{context}: {value:?}"))
}

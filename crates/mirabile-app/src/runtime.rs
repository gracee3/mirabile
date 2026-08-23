use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use async_trait::async_trait;
use futures::channel::oneshot;
use mirabile_engine::{
    BackendDescriptor, CalculationBackend, CalculationWorkerRequest, CalculationWorkerResult,
    execute_calculation_request,
};
use thiserror::Error;

#[async_trait(?Send)]
pub trait CalculationRuntime: Clone {
    fn backend_descriptor(&self) -> BackendDescriptor;

    fn submit(&self, request: CalculationWorkerRequest) -> Result<(), CalculationRuntimeError>;

    async fn receive(&self) -> Result<CalculationWorkerResult, CalculationRuntimeError>;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct CalculationRuntimeError {
    pub message: String,
}

impl CalculationRuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

type RuntimeEvent = Result<CalculationWorkerResult, CalculationRuntimeError>;

#[derive(Clone, Default)]
pub(crate) struct RuntimeInbox {
    inner: Rc<RefCell<RuntimeInboxState>>,
}

#[derive(Default)]
struct RuntimeInboxState {
    queued: VecDeque<RuntimeEvent>,
    waiters: VecDeque<oneshot::Sender<RuntimeEvent>>,
}

impl RuntimeInbox {
    pub(crate) fn push(&self, event: RuntimeEvent) {
        let mut state = self.inner.borrow_mut();
        if let Some(waiter) = state.waiters.pop_front() {
            let _ = waiter.send(event);
        } else {
            state.queued.push_back(event);
        }
    }

    pub(crate) async fn receive(&self) -> RuntimeEvent {
        let receiver = {
            let mut state = self.inner.borrow_mut();
            if let Some(event) = state.queued.pop_front() {
                return event;
            }
            let (sender, receiver) = oneshot::channel();
            state.waiters.push_back(sender);
            receiver
        };
        receiver.await.unwrap_or_else(|_| {
            Err(CalculationRuntimeError::new(
                "calculation runtime result channel closed",
            ))
        })
    }
}

/// Native/test runtime that exercises the same worker request/result contract inline.
#[derive(Clone)]
pub struct InlineCalculationRuntime<B> {
    backend: B,
    inbox: RuntimeInbox,
}

impl<B> InlineCalculationRuntime<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            inbox: RuntimeInbox::default(),
        }
    }
}

#[async_trait(?Send)]
impl<B> CalculationRuntime for InlineCalculationRuntime<B>
where
    B: CalculationBackend + Clone,
{
    fn backend_descriptor(&self) -> BackendDescriptor {
        self.backend.descriptor()
    }

    fn submit(&self, request: CalculationWorkerRequest) -> Result<(), CalculationRuntimeError> {
        self.inbox
            .push(Ok(execute_calculation_request(&self.backend, request)));
        Ok(())
    }

    async fn receive(&self) -> Result<CalculationWorkerResult, CalculationRuntimeError> {
        self.inbox.receive().await
    }
}

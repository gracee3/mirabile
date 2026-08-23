#[cfg(all(target_arch = "wasm32", feature = "xalen-backend"))]
use astra_engine::XalenBackend;
#[cfg(target_arch = "wasm32")]
use astra_engine::{CalculationWorkerRequest, DeterministicBackend, execute_calculation_request};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

#[cfg(target_arch = "wasm32")]
const WORKER_READY: &str = "ASTRA_CALCULATION_WORKER_READY_V1";

#[cfg(target_arch = "wasm32")]
fn main() {
    let scope = DedicatedWorkerGlobalScope::from(JsValue::from(js_sys::global()));
    let response_scope = scope.clone();
    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        let Some(encoded) = event.data().as_string() else {
            web_sys::console::error_1(
                &"Astra calculation worker received a non-string message".into(),
            );
            return;
        };
        let request = match serde_json::from_str::<CalculationWorkerRequest>(&encoded) {
            Ok(request) => request,
            Err(error) => {
                web_sys::console::error_1(
                    &format!("Astra calculation worker could not decode request: {error}").into(),
                );
                return;
            }
        };
        #[cfg(feature = "xalen-backend")]
        let result = if request.backend.backend.id == XalenBackend::ID {
            execute_calculation_request(&XalenBackend, request)
        } else {
            execute_calculation_request(&DeterministicBackend, request)
        };
        #[cfg(not(feature = "xalen-backend"))]
        let result = execute_calculation_request(&DeterministicBackend, request);
        match serde_json::to_string(&result) {
            Ok(encoded) => {
                if let Err(error) = response_scope.post_message(&JsValue::from_str(&encoded)) {
                    web_sys::console::error_2(
                        &"Astra calculation worker could not post result".into(),
                        &error,
                    );
                }
            }
            Err(error) => web_sys::console::error_1(
                &format!("Astra calculation worker could not encode result: {error}").into(),
            ),
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    if let Err(error) = scope.post_message(&JsValue::from_str(WORKER_READY)) {
        web_sys::console::error_2(
            &"Astra calculation worker could not announce readiness".into(),
            &error,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("calculation-worker is a WebAssembly Web Worker entry point");
}

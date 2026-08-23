#[cfg(not(feature = "browser-contract"))]
mod app;
#[cfg(all(target_arch = "wasm32", feature = "browser-contract"))]
mod browser_contract;
#[cfg(not(feature = "browser-contract"))]
mod commands;
#[cfg(all(
    not(feature = "browser-contract"),
    any(test, not(target_arch = "wasm32"))
))]
mod mock_application;
#[cfg(not(feature = "browser-contract"))]
mod render;

#[cfg(all(target_arch = "wasm32", not(feature = "browser-contract")))]
fn main() {
    leptos::mount::mount_to_body(app::App);
}

#[cfg(all(target_arch = "wasm32", feature = "browser-contract"))]
fn main() {
    leptos::mount::mount_to_body(browser_contract::BrowserContract);
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "browser-contract")))]
fn main() {
    std::hint::black_box(app::App);
    println!("mirabile-web is a WebAssembly CSR application; use Trunk to run it");
}

#[cfg(all(not(target_arch = "wasm32"), feature = "browser-contract"))]
fn main() {
    println!("the browser contract must run on wasm32-unknown-unknown");
}

mod app;
mod demo;
mod persistence;
mod render;

#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount::mount_to_body(app::App);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    std::hint::black_box(app::App);
    println!("astra-web is a WebAssembly CSR application; use Trunk to run it");
}

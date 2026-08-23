//! Repository contracts and persistence adapters.

pub mod command_handler;
pub mod format;
pub mod memory;
pub mod repository;
pub mod sync;

#[cfg(target_arch = "wasm32")]
pub mod indexeddb;

pub use command_handler::*;
pub use format::*;
pub use memory::*;
pub use repository::*;
pub use sync::*;

#[cfg(target_arch = "wasm32")]
pub use indexeddb::*;

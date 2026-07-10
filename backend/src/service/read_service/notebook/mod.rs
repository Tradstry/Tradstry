//! Notebook read/write orchestration over the table layer. Re-exported flat so
//! callers use `notebook_service::create_notebook_note` regardless of the split.
mod folders;
mod images;
mod notes;

pub use folders::*;
pub use images::*;
pub use notes::*;

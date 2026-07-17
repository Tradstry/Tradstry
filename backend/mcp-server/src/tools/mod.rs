//! MCP tool implementations, grouped by domain.
//!
//! Each module owns its parameter structs and its own `#[tool_router]`; `server.rs` merges
//! them. Splitting by domain keeps any one file small enough to hold in your head.

pub mod read;
pub mod write;

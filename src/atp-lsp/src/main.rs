//! # ATP Language Server
//!
//! A minimal Language Server Protocol (LSP) implementation for AQL
//! (ATP Query Language). Communicates over stdin/stdout using the
//! JSON-RPC 2.0 protocol defined by the LSP specification.
//!
//! ## Supported Capabilities
//!
//! - **textDocument/completion** — keyword and stage completions for AQL
//! - **textDocument/hover** — documentation on hover for AQL keywords
//! - **textDocument/diagnostic** — real-time syntax validation via `aql::parse`
//! - **textDocument/formatting** — canonical formatting for AQL queries
//!
//! ## Usage
//!
//! ```sh
//! atp-lsp --stdio
//! ```
//!
//! Configure your editor to launch `atp-lsp --stdio` as the language server
//! for `.aql` files.

mod server;

fn main() {
    if let Err(e) = server::run() {
        eprintln!("atp-lsp: fatal error: {e:#}");
        std::process::exit(1);
    }
}

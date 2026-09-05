//! Standard LSP 3.17 server for the Locus language.

mod capabilities;
mod diagnostics;
mod document;
mod handlers;
mod server;
mod state;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    server::run_stdio()
}

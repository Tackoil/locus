//! Feature-gated `ReX` integration seam.
//!
//! Upstream `ReX` does not compile on the workspace's pinned Rust 1.88 toolchain:
//! its `static_map` procedural macros panic while generating STIX2 tables. This
//! type keeps engine selection feature-gated and typed until `ReX` becomes viable.
//! The exact tested revision and reproduction steps are recorded in the crate's
//! `REX-VIABILITY.md`.

use crate::{FontSize, TexBox, TexEngine, TexError};

const UNAVAILABLE_REASON: &str =
    "upstream ReX's STIX2 static_map macros fail to compile on Rust 1.88";

/// The feature-gated `ReX` engine seam.
#[derive(Clone, Copy, Debug, Default)]
pub struct RexEngine;

impl TexEngine for RexEngine {
    fn render(&self, _tex: &str, _size: FontSize) -> Result<TexBox, TexError> {
        Err(TexError::EngineUnavailable {
            engine: "ReX",
            reason: UNAVAILABLE_REASON,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_backend_returns_typed_error() {
        let result = RexEngine.render("x^2", FontSize::DEFAULT);

        assert!(matches!(
            result,
            Err(TexError::EngineUnavailable { engine: "ReX", .. })
        ));
    }
}

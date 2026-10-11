//! Expand and measure immutable content before geometry evaluation.

use locus_core::{EvaluationInputs, preset::LayoutMetrics};
use locus_layout::{LayoutError, TextBoxes, measurement::measure_file};
use locus_syntax::File;
use locus_tex::{MockEngine, PlaintextEngine, RexEngine, TexEngine};

use crate::TextEngine;

pub(super) fn prepare(
    source: &File,
    engine: TextEngine,
) -> Result<(EvaluationInputs, TextBoxes), LayoutError> {
    let tex: &dyn TexEngine = match engine {
        TextEngine::Rex => &RexEngine,
        TextEngine::Mock => &MockEngine,
    };
    measure_file(
        source,
        &PlaintextEngine::new(),
        tex,
        LayoutMetrics::default(),
    )
}

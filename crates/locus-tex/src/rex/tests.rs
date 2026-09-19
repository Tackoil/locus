use super::*;
use crate::{PlaintextEngine, TextSegment, render_segments};

type Result = std::result::Result<(), TexError>;

#[test]
fn real_formulas_produce_deterministic_outlines() -> Result {
    for source in [
        "x^2",
        r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}",
        r"\sum_{i=1}^{n} x_i",
        r"\int_0^1 x^2\,dx",
        r"\left(\frac{a}{b}\right)",
        r"\widehat{ABC}",
        r"\mathbb{R} \to \alpha + \beta",
        r"\begin{matrix}a & b \\ c & d\end{matrix}",
    ] {
        let rendered = RexEngine.render(source, FontSize::DEFAULT)?;
        assert_eq!(rendered, RexEngine.render(source, FontSize::DEFAULT)?);
        assert!(rendered.svg_body.contains("<path"), "{source}");
        assert!(!rendered.svg_body.contains("<text"));
        assert!(!rendered.svg_body.contains("http"));
        assert!(rendered.metrics.width > 0.0, "{source}");
        assert!(rendered.metrics.height > 0.0, "{source}");
        rendered.validate()?;
    }
    Ok(())
}

#[test]
fn scripts_and_fractions_have_real_vertical_metrics() -> Result {
    let base = RexEngine.render("x", FontSize::DEFAULT)?;
    let upper = RexEngine.render("x^2", FontSize::DEFAULT)?;
    let lower = RexEngine.render("x_2", FontSize::DEFAULT)?;
    let fraction = RexEngine.render(r"\frac{x}{y}", FontSize::DEFAULT)?;
    assert!(upper.metrics.height > base.metrics.height);
    assert!(lower.metrics.depth > base.metrics.depth);
    assert!(fraction.metrics.height > base.metrics.height);
    assert!(fraction.metrics.depth > base.metrics.depth);
    assert!(fraction.svg_body.contains("<rect"));
    Ok(())
}

#[test]
fn font_size_scales_all_metrics() -> Result {
    let source = r"\frac{x_1}{\sqrt{y}}";
    let base = RexEngine.render(source, FontSize::DEFAULT)?;
    for factor in [0.5, 1.25, 2.0] {
        let scaled = RexEngine.render(source, FontSize::new(factor))?;
        for (actual, expected) in [
            (scaled.metrics.width, base.metrics.width),
            (scaled.metrics.height, base.metrics.height),
            (scaled.metrics.depth, base.metrics.depth),
            (scaled.advance, base.advance),
            (scaled.baseline_x, base.baseline_x),
        ] {
            assert!((actual - expected * factor).abs() < 1e-12);
        }
    }
    Ok(())
}

#[test]
fn invalid_input_returns_typed_errors() {
    for source in [
        r"\notacommand{x}",
        r"\frac{1}",
        "{x",
        "x}",
        "x^",
        "x^^2",
        "🙂",
    ] {
        assert!(
            matches!(
                RexEngine.render(source, FontSize::DEFAULT),
                Err(TexError::InvalidTex { .. })
            ),
            "{source}"
        );
    }
    for source in ["x\ny", "x\ry"] {
        assert_eq!(
            RexEngine.render(source, FontSize::DEFAULT),
            Err(TexError::RawTexNewline)
        );
    }
    assert_eq!(
        RexEngine.render("x\0", FontSize::DEFAULT),
        Err(TexError::InvalidXmlCharacter { codepoint: 0 })
    );
}

#[test]
fn invalid_sizes_and_overflow_are_rejected() {
    for size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            RexEngine.render("x", FontSize::new(size)),
            Err(TexError::InvalidFontSize { .. })
        ));
    }
    assert!(matches!(
        RexEngine.render("WWWW", FontSize::new(f64::MAX)),
        Err(TexError::InvalidMetrics { .. })
    ));
}

#[test]
fn empty_input_has_zero_extents() -> Result {
    for source in ["", " ", "{}"] {
        let rendered = RexEngine.render(source, FontSize::DEFAULT)?;
        assert_eq!(rendered.metrics, BoxMetrics::ZERO);
        assert!(!rendered.svg_body.contains("<path"));
    }
    Ok(())
}

#[test]
fn source_metadata_is_xml_escaped() -> Result {
    let rendered = RexEngine.render("x < y", FontSize::DEFAULT)?;
    assert!(rendered.svg_body.contains("x &lt; y"));
    Ok(())
}

#[test]
fn mixed_segments_preserve_inline_advance_and_baselines() -> Result {
    let plain = PlaintextEngine::new();
    let left = plain.render("value = ", FontSize::DEFAULT)?;
    let math = RexEngine.render(r"\frac{x}{y}", FontSize::DEFAULT)?;
    let right = plain.render(" units", FontSize::DEFAULT)?;
    let mixed = render_segments(
        &[
            TextSegment::Plain("value = "),
            TextSegment::Tex(r"\frac{x}{y}"),
            TextSegment::Plain(" units"),
        ],
        &plain,
        &RexEngine,
        FontSize::DEFAULT,
    )?;
    assert!((mixed.advance - left.advance - math.advance - right.advance).abs() < 1e-12);
    assert!(mixed.metrics.height >= math.metrics.height);
    assert!(mixed.metrics.depth >= math.metrics.depth);
    Ok(())
}

#[test]
fn bundled_font_matches_rex_tables() -> Result {
    let face = bundled_font()?;
    assert_eq!(face.units_per_em(), 1000);
    let rendered = RexEngine.render("2", FontSize::DEFAULT)?;
    assert!((rendered.advance - 0.495).abs() < 1e-12);
    assert!((rendered.metrics.height - 0.644).abs() < 1e-12);
    Ok(())
}

#[test]
fn glyph_overhang_expands_visual_bounds_without_adding_inline_space() -> Result {
    // STIX's italic A has a -12 font-unit left bearing and 725-unit advance.
    let rendered = RexEngine.render("A", FontSize::DEFAULT)?;
    assert!((rendered.baseline_x - 0.012).abs() < 1e-12);
    assert!((rendered.advance - 0.725).abs() < 1e-12);
    assert!((rendered.metrics.width - 0.737).abs() < 1e-12);
    assert!(rendered.svg_body.contains("translate(12.0 0)"));
    Ok(())
}

#[test]
fn unsupported_upstream_constructs_stay_inside_error_boundary() {
    // Upstream panics on a missing rule dimension or mismatched environment.
    for source in [r"\rule", r"\begin{matrix}x\end{array}", r"\rule -1em 1em"] {
        assert!(matches!(
            RexEngine.render(source, FontSize::DEFAULT),
            Err(TexError::InvalidTex { .. })
        ));
    }
}

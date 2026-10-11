//! Timing collection stays observational, scoped, and thread-local.

use locus_core::metrics::{collect, measure, span};

#[test]
fn nested_and_repeated_steps_preserve_structure_and_error_values() {
    let (value, report) = collect(|| {
        let _outer = span("outer");
        measure("repeated", || 1);
        measure("repeated", || Err::<(), _>("failure"))
    });
    assert_eq!(value, Err("failure"));
    assert_eq!(report.steps.len(), 3);
    assert_eq!(report.steps[0].parent, None);
    for step in &report.steps[1..] {
        assert_eq!(step.name, "repeated");
        assert_eq!(step.parent, Some(0));
        assert!(step.start_ms >= report.steps[0].start_ms);
        assert!(step.duration_ms >= 0.0);
        assert!(step.start_ms + step.duration_ms <= report.total_ms);
    }
    assert!(report.steps[0].duration_ms >= report.steps[1].duration_ms);
    assert!(report.total_ms >= report.steps[0].duration_ms);
}

#[test]
fn nested_collections_restore_the_previous_collection() {
    let ((), outer) = collect(|| {
        let _outer = span("outer");
        let ((), inner) = collect(|| measure("inner", || ()));
        assert_eq!(inner.steps.len(), 1);
        assert_eq!(inner.steps[0].parent, None);
        measure("after", || ());
    });
    assert_eq!(outer.steps.len(), 2);
    assert_eq!(outer.steps[1].name, "after");
    assert_eq!(outer.steps[1].parent, Some(0));
}

#[test]
fn disabled_spans_do_not_leak_into_later_collections() {
    let _disabled = span("disabled");
    let ((), report) = collect(|| measure("enabled", || ()));
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].name, "enabled");
    assert_eq!(report.steps[0].parent, None);
}

#[test]
fn child_threads_collect_independently() -> std::thread::Result<()> {
    let (result, outer) = collect(|| {
        let child = std::thread::spawn(|| {
            let _disabled = span("uncollected_child");
            let ((), report) = collect(|| measure("child", || ()));
            report
        });
        let child_report = child.join()?;
        assert_eq!(child_report.steps.len(), 1);
        assert_eq!(child_report.steps[0].name, "child");
        Ok(())
    });
    result?;
    assert!(outer.steps.is_empty());
    Ok(())
}

#[test]
fn unwinding_restores_the_previous_collection() {
    let ((), report) = collect(|| {
        let _outer = span("outer");
        let failed = std::panic::catch_unwind(|| {
            collect(|| {
                let _inner = span("inner");
                // Simulate an unwinding third-party operation without a panic macro.
                std::panic::resume_unwind(Box::new("test unwind"));
            });
        });
        assert!(failed.is_err());
        measure("after", || ());
    });
    assert_eq!(report.steps.len(), 2);
    assert_eq!(report.steps[1].parent, Some(0));
}

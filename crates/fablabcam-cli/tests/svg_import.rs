use std::fs;

#[test]
fn svg_conversion_applies_geometry_and_preserves_output_on_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("drawing.svg");
    let output = dir.path().join("drawing.cb");
    let args = vec![
        "convert".into(),
        input.display().to_string(),
        output.display().to_string(),
    ];
    fs::write(&input, r#"<svg viewBox="0 0 40 40"><defs><rect width="500" height="500"/></defs><g transform="scale(0.1)"><path d="M0 0 H1e2 V100 H0 Z"/></g></svg>"#).unwrap();
    let report = fablabcam_cli::run(&args).unwrap();
    assert!(report.contains("1 formes"), "{report}");
    assert!(!report.contains("ignoré"));
    let saved = fs::read(&output).unwrap();
    let project = fablabcam_studio::persist::load(std::str::from_utf8(&saved).unwrap()).unwrap();
    assert_eq!(project.shapes.len(), 1);
    assert_eq!(
        fablabcam_studio::shape::bounds(&project.shapes[0].entity).unwrap(),
        ([0.0, 30.0], [10.0, 40.0])
    );
    fs::write(&input, r#"<svg><path id="bad" d="M0 0 L1e- 0"/></svg>"#).unwrap();
    let error = fablabcam_cli::run(&args).unwrap_err();
    assert!(error.contains("#bad"), "{error}");
    assert_eq!(fs::read(&output).unwrap(), saved);
}

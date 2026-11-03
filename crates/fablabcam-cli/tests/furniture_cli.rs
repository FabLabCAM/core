use fablabcam_cli::run;

fn command(args: &[&str]) -> Result<String, String> {
    run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

#[test]
fn documented_furniture_workflow_and_failure_modes() {
    let root = tempfile::tempdir().unwrap();
    let template = root.path().join("model.fcm-template.json");
    let project = root.path().join("job.fcm.json");
    let zip = root.path().join("job.zip");
    let t = template.to_str().unwrap();
    let p = project.to_str().unwrap();
    let z = zip.to_str().unwrap();
    command(&["furniture", "template", "cabinet", "-o", t]).unwrap();
    command(&["furniture", "new", t, "--width", "700", "-o", p]).unwrap();
    assert!(command(&["furniture", "validate", p]).unwrap().contains("Document valide"));
    assert!(command(&["furniture", "plan", p]).unwrap().contains("660.0"));
    assert!(command(&["furniture", "pack", p, "-o", z]).unwrap().contains("3 dalles"));
    let before = std::fs::read(&zip).unwrap();
    assert!(command(&["furniture", "pack", p, "-o", z]).is_err());
    assert_eq!(std::fs::read(&zip).unwrap(), before);
    assert!(command(&["furniture", "new", t, "--width", "NaN", "-o", p]).is_err());
    assert!(command(&["furniture", "new", t, "--unknown", "1"]).is_err());
    assert!(command(&["furniture", "new", t, "--width"]).is_err());
    assert!(command(&["furniture", "new", t, "--width", "600", "--width", "700"]).is_err());
}

use std::path::Path;

#[test]
fn validate_and_dispatch_terrain_region() {
    let project = diamond::compile_project(Path::new("tests/fixtures/probe.diamond.toml"))
        .expect("probe project must compile");
    let mut grants = diamond::RuntimeGrants::default();
    arteries_runner::register(&mut grants).expect("registration must succeed");
    project
        .validate_compute(&grants)
        .expect("declared export must be registered");
    let value = project
        .evaluate_with(&grants)
        .expect("evaluation must succeed");
    let rendered = format!("{value:?}");
    assert!(
        rendered.contains("1,1,6,5"),
        "packed region missing from {rendered}"
    );
}

#[test]
fn unregistered_runtime_fails_validation() {
    let project = diamond::compile_project(Path::new("tests/fixtures/probe.diamond.toml"))
        .expect("probe project must compile");
    assert!(project
        .validate_compute(&diamond::RuntimeGrants::default())
        .is_err());
}

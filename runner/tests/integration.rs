use arteries_core::{MAP_H, MAP_W, MAX_SEED};
use arteries_runner::TerrainRegion;
use serde_json::{json, Value as Json};
use std::path::Path;

/// Expected packed output for seed 0, `u0 = 0`, `v0 = 0`, `4 × 3`.
const SEED_ZERO_4X3: &str = "1,1,6,5;1,1,6,5;2,1,6,5";

fn wire(arguments: &[i64]) -> Vec<Json> {
    arguments
        .iter()
        .map(|value| json!(value.to_string()))
        .collect()
}

fn evaluate(arguments: &[i64]) -> String {
    let reply = TerrainRegion::evaluate(&wire(arguments)).expect("valid region");
    reply.as_str().expect("Text reply").to_string()
}

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
        .expect("evaluation must succeed")
        .into_result()
        .expect("probe must return Ok");
    assert_eq!(
        value.display_text().expect("probe must return Text"),
        SEED_ZERO_4X3
    );
}

#[test]
fn packed_regions_match_the_map_including_offsets_and_padding() {
    // A 4 × 3 window at (2, 1) on seed 7, taken from the frozen Diamond map.
    assert_eq!(evaluate(&[7, 1, 0, 2, 1, 4, 3]), "2,6,5,6;1,6,5,6;1,1,6,5");
    // A window crossing the right and bottom edges pads with water (code 5).
    assert_eq!(evaluate(&[7, 1, 0, 8, 4, 4, 3]), "6,5,5,5;6,5,5,5;5,5,5,5");
    // Extreme offsets wrap rather than panic; every cell falls outside the map.
    for offset in [i64::MIN, i64::MAX] {
        let packed = evaluate(&[MAX_SEED, 1, 0, offset, offset, MAP_W, MAP_H]);
        assert!(
            packed
                .split(';')
                .flat_map(|row| row.split(','))
                .all(|cell| cell == "5"),
            "expected water at offset {offset}, got {packed}"
        );
    }
}

#[test]
fn rejects_out_of_range_and_malformed_wire_arguments() {
    let good = [0, 1, 0, 0, 0, 4, 3];
    for seed in [-1, MAX_SEED + 1] {
        let mut arguments = good;
        arguments[0] = seed;
        assert_eq!(
            TerrainRegion::evaluate(&wire(&arguments)),
            Err(diamond::ComputeError::InvalidInput),
            "seed {seed}"
        );
    }
    for version in [0, 2] {
        let mut arguments = good;
        arguments[1] = version;
        assert_eq!(
            TerrainRegion::evaluate(&wire(&arguments)),
            Err(diamond::ComputeError::InvalidInput),
            "version {version}"
        );
    }
    let mut arguments = good;
    arguments[2] = 1;
    assert_eq!(
        TerrainRegion::evaluate(&wire(&arguments)),
        Err(diamond::ComputeError::InvalidInput),
        "face"
    );
    for (index, value) in [(5, 0), (5, MAP_W + 1), (6, 0), (6, MAP_H + 1)] {
        let mut arguments = good;
        arguments[index] = value;
        assert_eq!(
            TerrainRegion::evaluate(&wire(&arguments)),
            Err(diamond::ComputeError::InvalidInput),
            "argument {index} = {value}"
        );
    }
    for arguments in [wire(&[]), wire(&good[..6]), wire(&[0, 1, 0, 0, 0, 4, 3, 9])] {
        assert_eq!(
            TerrainRegion::evaluate(&arguments),
            Err(diamond::ComputeError::InvalidInput),
            "arity"
        );
    }
    let mut wrong_type = wire(&good);
    wrong_type[0] = json!(0);
    assert_eq!(
        TerrainRegion::evaluate(&wrong_type),
        Err(diamond::ComputeError::InvalidInput),
        "Int must be a canonical decimal string"
    );
}

#[test]
fn real_app_manifest_matches_the_registered_kernels() {
    let project = diamond::compile_project(Path::new("../app/diamond.toml"))
        .expect("Arteries project must compile");
    let mut grants = diamond::RuntimeGrants::default();
    arteries_runner::register(&mut grants).expect("registration must succeed");
    project
        .validate_compute(&grants)
        .expect("app manifest and runner registrations must agree");
}

#[test]
fn unregistered_runtime_fails_validation() {
    let project = diamond::compile_project(Path::new("tests/fixtures/probe.diamond.toml"))
        .expect("probe project must compile");
    assert!(project
        .validate_compute(&diamond::RuntimeGrants::default())
        .is_err());
}

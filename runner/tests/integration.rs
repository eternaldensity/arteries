use arteries_core::{MAP_H, MAP_W, MAX_SEED};
use arteries_runner::{Finish, Market, Pathfind, RouteCost, TerrainRegion};
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
fn validate_and_dispatch_pathfinding_through_diamond() {
    let project = diamond::compile_project(Path::new("tests/fixtures/pathfind.diamond.toml"))
        .expect("pathfind probe must compile");
    let mut grants = diamond::RuntimeGrants::default();
    arteries_runner::register(&mut grants).expect("registration must succeed");
    project
        .validate_compute(&grants)
        .expect("declared exports must be registered");
    let value = project
        .evaluate_with(&grants)
        .expect("evaluation must succeed")
        .into_result()
        .expect("probe must return Ok");
    assert_eq!(value.display_text().expect("Int reply"), "7");
}

#[test]
fn pathfind_and_route_cost_round_trip() {
    let grid = "5,3|1,1,1,1,1;1,5,1,1,1;1,1,1,1,1";
    let reply = Pathfind::evaluate(&[json!(grid), json!("0,0"), json!("4,0"), json!("road")])
        .expect("path");
    assert_eq!(reply, json!(["0", "0", "4", "0"]));

    let stats = RouteCost::evaluate(&[json!(grid), reply, json!("road")]).expect("cost");
    assert_eq!(stats, json!(["4", "4", "0", "0", "0"]));
}

#[test]
fn route_cost_counts_water_on_the_drawn_line() {
    let grid = "3,1|1,5,1";
    let reply = Pathfind::evaluate(&[json!(grid), json!("0,0"), json!("2,0"), json!("road")])
        .expect("path");
    assert_eq!(reply, json!(["0", "0", "2", "0"]));

    let stats = RouteCost::evaluate(&[json!(grid), reply, json!("road")]).expect("cost");
    assert_eq!(stats, json!(["7", "2", "1", "0", "0"]));
}

#[test]
fn pathfind_runs_on_a_64_by_64_grid_through_the_abi() {
    let row = vec!["1"; 64].join(",");
    let grid = format!("64,64|{}", vec![row; 64].join(";"));
    let reply = Pathfind::evaluate(&[json!(grid), json!("0,0"), json!("63,63"), json!("road")])
        .expect("path");
    let points = reply.as_array().expect("List<Int> reply");
    assert!(points.len() <= 1000);
    assert_eq!(points.first(), Some(&json!("0")));
    assert_eq!(points.get(1), Some(&json!("0")));
    assert_eq!(points.get(points.len() - 2), Some(&json!("63")));
    assert_eq!(points.last(), Some(&json!("63")));
}

#[test]
fn market_kernel_matches_the_diamond_market_period() {
    let towns = json!([
        [
            "4", "6", "0", "4", "0", "0", "0", "1", "0", "0", "0", "16", "4", "4", "4", "2", "0",
            "0", "4"
        ],
        [
            "4", "6", "0", "0", "3", "1", "0", "0", "0", "0", "0", "8", "6", "2", "0", "0", "0",
            "0", "0"
        ]
    ]);
    let routes = json!([["0", "1", "1", "0", "20", "1"]]);
    let reply =
        Market::evaluate(&[json!("160"), towns, routes, json!("7"), json!("100")]).expect("market");
    assert_eq!(
        reply,
        json!([
            ["163"],
            ["5", "26", "98", "11", "3", "3", "4", "2", "0", "0", "0"],
            ["3", "0", "42", "3", "27", "9", "0", "0", "0", "0", "0"],
            ["3"]
        ])
    );
}

#[test]
fn finish_kernel_matches_the_diamond_finish_period() {
    let towns = json!([
        [
            "4", "6", "0", "4", "0", "0", "0", "1", "0", "0", "0", "16", "4", "4", "4", "2", "0",
            "0", "4"
        ],
        [
            "4", "6", "0", "0", "3", "1", "0", "0", "0", "0", "0", "8", "6", "2", "0", "0", "0",
            "0", "0"
        ]
    ]);
    let reply = Finish::evaluate(&[
        json!("7"),
        json!("0"),
        json!("28"),
        json!("2"),
        json!("0"),
        towns,
    ])
    .expect("finish");
    assert_eq!(
        reply,
        json!([
            ["0", "10"],
            ["4", "6", "0", "19", "4", "4", "4", "2", "0", "0", "4"],
            ["4", "4", "0", "11", "8", "3", "0", "0", "0", "0", "0"]
        ])
    );
}

#[test]
fn finish_kernel_pages_a_hundred_towns() {
    let mut towns = Vec::with_capacity(100);
    for _ in 0..100 {
        let mut row = vec!["2".to_string(), "6".to_string(), "0".to_string()];
        row.extend(["4", "0", "0", "0", "1", "0", "0", "0"].map(String::from));
        row.extend(["1", "1", "1", "1", "1", "1", "1", "1"].map(String::from));
        towns.push(Json::Array(row.into_iter().map(Json::String).collect()));
    }
    let reply = Finish::evaluate(&[
        json!("7"),
        json!("0"),
        json!("28"),
        json!("100"),
        json!("0"),
        Json::Array(towns),
    ])
    .expect("finish");
    let rows = reply.as_array().expect("nested list");
    assert_eq!(rows.len(), 101);
    assert_eq!(rows[0], json!(["0", "598"]));
    assert_eq!(
        rows[8],
        json!(["2", "4", "0", "4", "3", "2", "1", "1", "1", "1", "1"])
    );
}

#[test]
fn finish_kernel_rejects_out_of_range_pages() {
    let row = json!([
        "2", "6", "0", "4", "0", "0", "0", "1", "0", "0", "0", "1", "1", "1", "1", "1", "1", "1",
        "1"
    ]);
    for (count, start) in [("0", "0"), ("1", "0"), ("1", "1")] {
        assert_eq!(
            Finish::evaluate(&[
                json!("7"),
                json!("0"),
                json!("28"),
                json!(count),
                json!(start),
                json!([row.clone(), row.clone()]),
            ]),
            Err(diamond::ComputeError::InvalidInput)
        );
    }
}

#[test]
fn market_kernel_handles_a_hundred_towns_through_the_abi() {
    let mut towns = Vec::with_capacity(100);
    for i in 0..100 {
        let mut row = vec![
            (3 + i % 5).to_string(),
            (4 + i % 7).to_string(),
            i.to_string(),
        ];
        row.extend(["4", "1", "1", "1", "1", "0", "0", "1"].map(String::from));
        row.extend(["12", "4", "4", "4", "2", "0", "0", "3"].map(String::from));
        towns.push(Json::Array(row.into_iter().map(Json::String).collect()));
    }
    let mut routes = Vec::with_capacity(50);
    for i in 0..50 {
        routes.push(json!([
            i.to_string(),
            (i + 1).to_string(),
            "1",
            "0",
            "20",
            "1"
        ]));
    }
    let reply = Market::evaluate(&[
        json!("10000"),
        Json::Array(towns),
        Json::Array(routes),
        json!("28"),
        json!("100"),
    ])
    .expect("market");
    let rows = reply.as_array().expect("nested list");
    assert_eq!(rows.len(), 1 + 100 + 50);
    assert_eq!(
        rows[0].as_array().map(Vec::len),
        Some(1),
        "money row must be scalar"
    );
    assert!(rows[1..=100]
        .iter()
        .all(|row| row.as_array().is_some_and(|values| values.len() == 11)));
    assert!(rows[101..]
        .iter()
        .all(|row| row.as_array().is_some_and(|values| values.len() == 1)));
}

#[test]
fn market_kernel_rejects_malformed_rows() {
    let good_town = json!([
        "4", "6", "0", "4", "0", "0", "0", "1", "0", "0", "0", "16", "4", "4", "4", "2", "0", "0",
        "4"
    ]);
    for (towns, routes) in [
        (json!([["4"]]), json!([])),
        (json!([good_town.clone()]), json!([["0", "0", "1"]])),
        (
            json!([good_town.clone()]),
            json!([["0", "2", "1", "0", "20", "1"]]),
        ),
        (
            json!([good_town.clone()]),
            json!([["-1", "0", "1", "0", "20", "1"]]),
        ),
    ] {
        assert_eq!(
            Market::evaluate(&[json!("0"), towns, routes, json!("1"), json!("100")]),
            Err(diamond::ComputeError::InvalidInput)
        );
    }
}

#[test]
fn pathfinding_kernels_reject_malformed_wire_arguments() {
    for arguments in [
        vec![],
        vec![json!("5,1|1,1,1,1,1"), json!("0,0"), json!("4,0")],
        vec![json!("bad"), json!("0,0"), json!("4,0"), json!("road")],
        vec![
            json!("5,1|1,1,1,1,1"),
            json!("zero"),
            json!("4,0"),
            json!("road"),
        ],
        vec![
            json!("5,1|1,1,1,1,1"),
            json!("0,0"),
            json!("4,0"),
            json!("swim"),
        ],
    ] {
        assert_eq!(
            Pathfind::evaluate(&arguments),
            Err(diamond::ComputeError::InvalidInput)
        );
    }
    let grid = json!("3,1|1,0,1");
    for path in [json!([]), json!(["0", "0", "2"]), json!([0, 0, 2, 0])] {
        assert_eq!(
            RouteCost::evaluate(&[grid.clone(), path, json!("road")]),
            Err(diamond::ComputeError::InvalidInput)
        );
    }
    assert_eq!(
        RouteCost::evaluate(&[
            json!("3,1|1,0,1"),
            json!(["0", "0", "1", "0"]),
            json!("road"),
        ]),
        Err(diamond::ComputeError::InvalidInput)
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

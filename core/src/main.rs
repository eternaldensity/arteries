//! `arteries-gen`: emit deterministic Arteries world data for build tooling.

use arteries_core::{manifest_line, MAP_COUNT, MAX_SEED};
use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "\
usage: arteries-gen terrain [--seed N | --count N] [--out PATH]

Emit map data in the asset pipeline format (`seed|row;row`), one line per seed.
Without `--seed`, emits seeds `0..count` (default: the built-in map count).
`--seed` must be within `0..=MAX_SEED`.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("arteries-gen: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        return Err(format!("missing command\n\n{USAGE}"));
    };
    if command != "terrain" {
        return Err(format!("unknown command {command:?}\n\n{USAGE}"));
    }

    let mut seed: Option<i64> = None;
    let mut count: Option<i64> = None;
    let mut out: Option<std::path::PathBuf> = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--seed" => seed = Some(parse_number("--seed", arguments.next())?),
            "--count" => count = Some(parse_number("--count", arguments.next())?),
            "--out" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| "--out needs a path".to_string())?;
                out = Some(std::path::PathBuf::from(path));
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown option {other:?}\n\n{USAGE}")),
        }
    }

    let seeds = match seed {
        Some(seed) => {
            if count.is_some() {
                return Err("--seed and --count are mutually exclusive".into());
            }
            vec![seed]
        }
        None => {
            let count = count.unwrap_or(MAP_COUNT);
            if !(1..=4096).contains(&count) {
                return Err("--count must be within 1..=4096".into());
            }
            (0..count).collect()
        }
    };

    let mut output = String::new();
    for seed in seeds {
        if !(0..=MAX_SEED).contains(&seed) {
            return Err(format!("--seed must be within 0..={MAX_SEED}"));
        }
        output.push_str(&manifest_line(seed));
        output.push('\n');
    }

    match out {
        Some(path) => std::fs::write(&path, output)
            .map_err(|error| format!("cannot write {}: {error}", path.display())),
        None => std::io::stdout()
            .write_all(output.as_bytes())
            .map_err(|error| format!("cannot write stdout: {error}")),
    }
}

fn parse_number(option: &str, value: Option<String>) -> Result<i64, String> {
    let value = value.ok_or_else(|| format!("{option} needs a number"))?;
    value
        .parse()
        .map_err(|_| format!("{option} needs a number, got {value:?}"))
}

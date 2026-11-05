//! `arteries-runner`: serve an Arteries project with its compute kernels attached.

use arteries_runner::register;
use std::net::SocketAddr;
use std::path::Path;

const USAGE: &str = "\
usage: arteries-runner [PROJECT] [LISTEN] [ORIGIN]

Serve PROJECT (default app/diamond.toml) on LISTEN (default 127.0.0.1:3010),
using ORIGIN (default http://127.0.0.1:3010) as the public origin.

Options:
  --mailbox PATH   private identity mail directory (default .diamond/mailbox)
  --project PATH   project manifest (default app/diamond.toml)
  --listen ADDR    listen address
  --origin URL     public origin

PostgreSQL configuration comes from DIAMOND_POSTGRES_URL.";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    if let Err(error) = run(arguments) {
        eprintln!("arteries-runner: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let mut project_path = "app/diamond.toml".to_string();
    let mut listen = "127.0.0.1:3010".to_string();
    let mut origin = "http://127.0.0.1:3010".to_string();
    let mut mailbox = std::path::PathBuf::from(".diamond/mailbox");
    let mut positionals: Vec<String> = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--mailbox" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| "--mailbox needs a path".to_string())?;
                mailbox = std::path::PathBuf::from(path);
            }
            "--project" => {
                project_path = arguments
                    .next()
                    .ok_or_else(|| "--project needs a path".to_string())?;
            }
            "--listen" => {
                listen = arguments
                    .next()
                    .ok_or_else(|| "--listen needs an address".to_string())?;
            }
            "--origin" => {
                origin = arguments
                    .next()
                    .ok_or_else(|| "--origin needs a URL".to_string())?;
            }
            other if !other.starts_with("--") => positionals.push(other.to_string()),
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    if let Some(value) = positionals.first() {
        project_path = value.clone();
    }
    if let Some(value) = positionals.get(1) {
        listen = value.clone();
    }
    if let Some(value) = positionals.get(2) {
        origin = value.clone();
    }

    let configuration = std::env::var("DIAMOND_POSTGRES_URL").unwrap_or_default();
    if configuration.is_empty() {
        return Err("DIAMOND_POSTGRES_URL is not set".into());
    }

    let project = diamond::compile_project(Path::new(&project_path))
        .map_err(|error| format!("cannot compile {project_path}: {error}"))?;
    let mut grants = diamond::RuntimeGrants::default();
    register(&mut grants).map_err(|error| format!("cannot register kernels: {error}"))?;
    project
        .validate_compute(&grants)
        .map_err(|error| format!("compute validation failed: {error}"))?;

    let address: SocketAddr = listen
        .parse()
        .map_err(|error| format!("bad listen address {listen:?}: {error}"))?;
    project
        .serve_with_runtime(address, &origin, &configuration, &mailbox, grants)
        .map_err(|error| format!("serve failed: {error}"))?;
    Ok(())
}

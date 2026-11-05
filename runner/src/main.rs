//! `arteries-runner`: serve an Arteries project with its compute kernels attached.

use arteries_runner::register;
use std::net::SocketAddr;
use std::path::Path;

const USAGE: &str = "\
usage: arteries-runner [PROJECT] [LISTEN] [ORIGIN]

Serve PROJECT (default app/diamond.toml) on LISTEN (default 127.0.0.1:3010),
using ORIGIN (default http://127.0.0.1:3010) as the public origin.

PostgreSQL configuration comes from DIAMOND_POSTGRES_URL. The mailbox lives at
.diamond/mailbox relative to the current directory.";

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
    let mut arguments = arguments.into_iter();
    let project_path = arguments
        .next()
        .unwrap_or_else(|| "app/diamond.toml".into());
    let listen = arguments.next().unwrap_or_else(|| "127.0.0.1:3010".into());
    let origin = arguments
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:3010".into());
    let configuration = std::env::var("DIAMOND_POSTGRES_URL").unwrap_or_default();
    if configuration.is_empty() {
        return Err("DIAMOND_POSTGRES_URL is not set".into());
    }
    let mailbox = std::path::PathBuf::from(".diamond/mailbox");

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

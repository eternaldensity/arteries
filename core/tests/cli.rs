use std::process::Command;

fn generator() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arteries-gen"))
}

#[test]
fn terrain_defaults_to_every_builtin_seed() {
    let Ok(output) = generator().arg("terrain").output() else {
        panic!("cannot run arteries-gen");
    };
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 16, "{text}");
    for (seed, line) in lines.iter().enumerate() {
        assert!(line.starts_with(&format!("{seed}|")), "{line}");
    }
}

#[test]
fn terrain_can_write_one_seed_to_a_file() {
    let path = std::env::temp_dir().join(format!("arteries-gen-{}.txt", std::process::id()));
    let Ok(output) = generator()
        .args(["terrain", "--seed", "7", "--out"])
        .arg(&path)
        .output()
    else {
        panic!("cannot run arteries-gen");
    };
    assert!(output.status.success(), "{output:?}");
    let Ok(text) = std::fs::read_to_string(&path) else {
        panic!("missing {:?}", path);
    };
    let _ = std::fs::remove_file(&path);
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "{text}");
    assert!(lines[0].starts_with("7|"), "{text}");
}

#[test]
fn terrain_rejects_empty_and_unknown_requests() {
    for arguments in [
        vec!["terrain", "--count", "0"],
        vec!["terrain", "--count", "4097"],
        vec!["terrain", "--seed", "-1"],
        vec!["terrain", "--seed", "1000000001"],
        vec!["terrain", "--seed", "1", "--count", "2"],
        vec!["terrain", "--bogus"],
        vec!["nope"],
    ] {
        let Ok(output) = generator().args(&arguments).output() else {
            panic!("cannot run arteries-gen");
        };
        assert!(!output.status.success(), "accepted {arguments:?}");
    }
}

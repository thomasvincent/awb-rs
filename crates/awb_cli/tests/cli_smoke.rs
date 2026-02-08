use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .current_dir(".")
        .output()
        .expect("Failed to run CLI with --help");

    assert!(
        output.status.success(),
        "CLI --help should exit with code 0. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("awb-rs") || stdout.contains("AutoWikiBrowser"),
        "Help output should contain CLI name or description"
    );
}

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--version"])
        .current_dir(".")
        .output()
        .expect("Failed to run CLI with --version");

    assert!(
        output.status.success(),
        "CLI --version should exit with code 0. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Version output should not be empty");
}

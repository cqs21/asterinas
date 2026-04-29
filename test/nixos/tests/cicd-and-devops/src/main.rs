// SPDX-License-Identifier: MPL-2.0

//! The test suite for CI/CD and DevOps applications on Asterinas NixOS.

use nixos_test_framework::*;

nixos_test_main!();

// ============================================================================
// CI/CD Runners - just
// ============================================================================

#[nixos_test]
fn just_basic(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("mkdir -p /tmp/just-test && cd /tmp/just-test")?;
    nixos_shell.run_cmd(r#"echo -e 'build:\n\techo "Hello from Just"' > justfile"#)?;

    nixos_shell.run_cmd_and_expect("just --list", "build")?;
    nixos_shell.run_cmd_and_expect("just build", "Hello from Just")?;
    Ok(())
}

// ============================================================================
// CI/CD Runners - Task
// ============================================================================

#[nixos_test]
fn task_basic(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("mkdir -p /tmp/task-test && cd /tmp/task-test")?;
    nixos_shell.run_cmd(
        r#"echo -e 'version: 3\ntasks:\n  build:\n    cmds:\n      - echo "Hello from Task"' > taskfile.yml"#,
    )?;

    nixos_shell.run_cmd_and_expect("task --list-all", "build")?;
    nixos_shell.run_cmd_and_expect("task build", "Hello from Task")?;
    Ok(())
}

// ============================================================================
// Release Automation - GoReleaser
// ============================================================================

#[nixos_test]
fn goreleaser_project(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("mkdir -p /tmp/goreleaser-test && cd /tmp/goreleaser-test")?;
    nixos_shell.run_cmd("go mod init goreleaser-test")?;
    nixos_shell.run_cmd(r#"echo -e 'package main\nimport "fmt"\nfunc main() { fmt.Println("Hello from GoReleaser") }' > main.go"#)?;

    nixos_shell.run_cmd("goreleaser init")?;
    nixos_shell.run_cmd_and_expect("goreleaser release --snapshot --clean", "succeeded")?;

    nixos_shell.run_cmd_and_expect("ls dist/goreleaser-test_linux_amd64*", "goreleaser-test")?;
    nixos_shell.run_cmd_and_expect("ls dist/goreleaser-test_linux_arm64*", "goreleaser-test")?;
    nixos_shell.run_cmd_and_expect(
        "ls dist/goreleaser-test_windows_amd64*",
        "goreleaser-test.exe",
    )?;
    nixos_shell.run_cmd_and_expect(
        "ls dist/goreleaser-test_windows_arm64*",
        "goreleaser-test.exe",
    )?;
    nixos_shell.run_cmd_and_expect("ls dist/goreleaser-test_darwin_amd64*", "goreleaser-test")?;
    nixos_shell.run_cmd_and_expect("ls dist/goreleaser-test_darwin_arm64*", "goreleaser-test")?;

    nixos_shell.run_cmd_and_expect(
        "./dist/goreleaser-test_linux_amd64_v1/goreleaser-test",
        "Hello from GoReleaser",
    )?;
    Ok(())
}

// SPDX-License-Identifier: MPL-2.0

//! The test suite for <TargetAppName> on Asterinas NixOS.

use nixos_test_framework::*;

nixos_test_main!();

#[nixos_test]
fn hello_world(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("echo 'Hello, World!' > out.txt")?;
    nixos_shell.run_cmd_and_expect("ls out.txt", "out.txt")?;
    
    Ok(())
}
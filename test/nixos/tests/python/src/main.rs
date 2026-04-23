// SPDX-License-Identifier: MPL-2.0

//! The test suite for Python regression tests on Asterinas NixOS.
//!
//! # Document maintenance
//!
//! An application's test suite and its "Verified Usage" section in Asterinas Book
//! should always be kept in sync.
//! So whenever you modify the test suite,
//! review the documentation and see if it should be updated accordingly.

use nixos_test_framework::*;

nixos_test_main!();

#[nixos_test]
fn python_regrtest(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("mkdir $PWD/python-src")?;
    nixos_shell.run_cmd("tar xf /etc/python-src.tar.xz --strip-components=1 -C $PWD/python-src")?;
    nixos_shell.run_cmd("export PYTHONPATH=$PWD/python-src/Lib")?;

    // Printed once by regrtest at the end of every run as `Result: {state}`:
    //   https://github.com/python/cpython/blob/v3.12.12/Lib/test/libregrtest/main.py#L455-L456
    // `state` is the literal "SUCCESS" iff no failure/env-change/interrupt/etc.
    // flag is set; any failure produces a disjoint string ("FAILURE",
    // "NO TESTS RAN, INTERRUPTED", ...), so this substring match is sufficient:
    //   https://github.com/python/cpython/blob/v3.12.12/Lib/test/libregrtest/results.py#L55-L69
    const RESULT_SUCCESS: &str = "Result: SUCCESS";

    let testcases = std::fs::read_to_string("src/testcases.txt")?;
    let mut failed_tests = Vec::new();
    for testcase in testcases
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let cmd = format!("python -m test -u all {testcase}");
        if nixos_shell
            .run_cmd_and_expect(&cmd, RESULT_SUCCESS)
            .is_err()
        {
            failed_tests.push(testcase);
        }
    }

    if !failed_tests.is_empty() {
        println!("=== Failed Python regression tests ===");
        for test in &failed_tests {
            println!("  - {test}");
        }
        println!("======================================");

        return Err(Error::EOF {
            expected: "All selected Python regression tests to pass".to_string(),
            got: format!("Failed cases: {}", failed_tests.join(", ")),
            exit_code: None,
        });
    }

    Ok(())
}

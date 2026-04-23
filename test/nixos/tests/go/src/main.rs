// SPDX-License-Identifier: MPL-2.0

//! The test suite for Go standard library tests on Asterinas NixOS.
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
fn go_stdlib(nixos_shell: &mut Session) -> Result<(), Error> {
    // `go test -short <pkg>` prints one summary line per package.
    // From Go Source: src/cmd/go/internal/test/test.go. Fields are TAB-separated.
    //
    //   ok  \t<pkg>\t<time>[\s<coverage>][\s[no tests to run]]
    //   ok  \t<pkg>\t(cached)
    //   ?   \t<pkg>\t[no test files]
    //   FAIL\t<pkg>\t<time>                        (test failure; preceded
    //                                               by `--- FAIL: TestFoo`)
    //   FAIL\t<pkg> [build failed]                 (compile error; space,
    //                                               not tab, before `[`)
    //
    // Each invocation runs `go test -short <pkg>` for exactly one package,
    // so the test command prints only that package's result. A successful
    // run ends with an `ok` or `?` summary line; a failed run prints `FAIL`
    // and may include `--- FAIL:` details, but it does not also emit an
    // `ok` / `?` summary for the same invocation. Because of that,
    // matching the success summary line is sufficient here without also
    // anchoring the package name.
    let success_regex = Regex::new(r"(?m)^(?:ok|\?)\s+")?;

    let testcases = std::fs::read_to_string("src/testcases.txt")?;
    let mut failed_tests = Vec::new();
    for testcase in testcases
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let cmd = format!("go test -short {testcase}");
        if nixos_shell
            .run_cmd_and_expect_regex(&cmd, &success_regex)
            .is_err()
        {
            failed_tests.push(testcase);
        }
    }

    if !failed_tests.is_empty() {
        println!("=== Failed Go stdlib testcases ===");
        for test in &failed_tests {
            println!("  - {test}");
        }
        println!("==================================");

        return Err(Error::EOF {
            expected: "All selected Go stdlib testcases to pass".to_string(),
            got: format!("Failed cases: {}", failed_tests.join(", ")),
            exit_code: None,
        });
    }

    Ok(())
}

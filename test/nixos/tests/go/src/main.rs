// SPDX-License-Identifier: MPL-2.0

//! The test suite for Go standard library tests on Asterinas NixOS.
//!
//! # Document maintenance
//!
//! An application's test suite and its "Verified Usage" section in Asterinas Book
//! should always be kept in sync.
//! So whenever you modify the test suite,
//! review the documentation and see if should be updated accordingly.

use nixos_test_framework::*;

nixos_test_main!();

/// Macro to generate Go test functions for specific directories.
macro_rules! go_test {
    (
        $(
            $fn_name:ident => $dir:literal $( : $args:literal )?
        ),* $(,)?
    ) => {
        $(
            go_test!(@one $fn_name, $dir $(, $args)?);
        )*
    };

    (@one $fn_name:ident, $dir:literal) => {
        go_test!(@one $fn_name, $dir, "");
    };

    (@one $fn_name:ident, $dir:literal, $args:literal) => {
        #[nixos_test]
        fn $fn_name(nixos_shell: &mut Session) -> Result<(), Error> {
            let cmd = format!("go test {} {}", $dir, $args);
            nixos_shell.run_cmd(&cmd)?;
            Ok(())
        }
    };
}

// Generate Go std test cases, obtained via `go list std`.
go_test! {
    archive => "archive/...",
    bufio => "bufio",
    bytes => "bytes",
    compress => "compress/...",
    container => "container/...",
    context => "context",
    crypto => "crypto/...",
    database => "database/...",
    debug => "debug/...",
    embed => "embed/...",
    encoding => "encoding/...": "-skip='TestCountDecodeMallocs'",
    errors => "errors",
    expvar => "expvar",
    flag => "flag",
    fmt => "fmt",
    // FIXME: This test can pass when executed individually, but adding this test causes ext2 to panic.
    // go => "go/...": "-skip='TestImportStdLib'",
    hash => "hash/...",
    html => "html/...",
    image => "image/...",
    index => "index/suffixarray",
    internal => "internal/...",
    io => "io/..",
    iter => "iter",
    log => "log/...",
    maps => "maps",
    math => "math/...",
    mime => "mime/...",
    // FIXME: The Go runtime panics directly, unable to pass `-skip` to filter runnable tests.
    // net => "net/...",
    net_http => "net/http/...": "-skip='TestServerKeepAliveAfterWriteError|TestDisableKeepAliveUpgrade|TestSOCKS5Proxy|TestTransportProxy|TestOmitHTTP2'",
    net_internal => "net/internal/...",
    net_mail => "net/mail",
    net_netip => "net/netip",
    net_rpc => "net/rpc/...",
    net_smtp => "net/smtp",
    net_textproto => "net/textproto",
    net_url => "net/url",
    os => "os": "-skip='TestLargeCopyViaNetwork|TestSpliceFile|TestGetPollFDAndNetwork|TestSendFile|TestRootConsistencyOpen/symlink_slash|TestRootConsistencyCreate/symlink_slash|TestRootConsistencyRemove/unreadable_directory|TestRootConsistencyStat/symlink_slash|TestRootRaceRenameDir|TestNonpollableDeadline|TestSymlinkWithTrailingSlash|TestRemoveAll'",
    // FIXME: Filtering out `TestExtraFiles` causes `os/exec` test to fail (even on Linux).
    // os_exec => "os/exec": "-skip='TestExtraFiles|TestFindExecutableVsNoexec'",
    os_exec_internal => "os/exec/internal/...",
    os_signal => "os/signal",
    os_user => "os/user",
    path => "path/...": "-skip='TestWalkSymlinkRoot'",
    plugin => "plugin",
    reflect => "reflect/...",
    regexp => "regexp/...",
    runtime => "runtime": "-skip='TestBreakpoint|TestAbort|TestDebugCall|TestNonblockingPipe|TestMincoreErrorSign'",
    runtime_coverage => "runtime/coverage",
    runtime_debug => "runtime/debug": "-skip='TestPanicOnFault'",
    runtime_internal => "runtime/internal/...",
    runtime_metrics => "runtime/metrics",
    runtime_prof => "runtime/pprof": "-skip='TestCPUProfile|TestMathBigDivide|TestMorestack|TestLabelRace|TestLabelSystemstack|TestTimeVDSO'",
    runtime_race => "runtime/race",
    runtime_trace => "runtime/trace",
    slices => "slices",
    sort => "sort",
    strconv => "strconv",
    strings => "strings",
    structs => "structs",
    sync => "sync/...",
    syscall => "syscall": "-skip='TestSCMCredentials|TestUseCgroupFD|TestFchmodat|TestSetuidEtc|TestPrlimitOtherProcess|TestPrlimitFileLimit|TestExecPtrace|TestFcntlFlock|TestPassFD'",
    testing => "testing/...",
    text => "text/...",
    time => "time/...",
    unicode => "unicode/...",
    unique => "unique",
    r#unsafe => "unsafe",
    vendor => "vendor/...",
    weak => "weak",
}

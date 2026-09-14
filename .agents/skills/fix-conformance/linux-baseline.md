# Establishing the Linux baseline

Read this when selecting, diagnosing, or fixing a test, and when verifying that it should be enabled.

## Derive the contract from source

Read the pinned test and its harness invocation, including arguments, filters, setup, and skip conditions.
Resolve the blocked configuration from `pool_file` and `run_vars`.
Then trace the operation through the corresponding Linux implementation at an explicit revision,
including dispatch to the actual filesystem or subsystem and configuration-dependent checks.
Use available local sources or retrieve the relevant upstream files; a full kernel checkout is unnecessary.

State the expected returns and effects separately from the test's assertions.
Cite the Linux revision, source URLs or paths, functions, and decisive checks.
Man pages provide context; a different backend's implementation does not establish this one's contract.
If Linux rejects the operation and Asterinas agrees, conclude `no-bug` and retain the blocklist entry.
Implementing an extra capability requires an explicit user direction beyond naming a test to fix.

## Corroborate in a comparable environment

After deriving the expectation, run the pinned binary or a minimal repro when practical.
Match the conditions relevant to the assertion: arguments/filter, actual test-file location,
filesystem implementation, mount options, kernel configuration, credentials, and namespaces as needed.
Check the harness's temporary-directory controls; changing the shell's working directory may not move its files.
Preserve the exact `gvisor_filter` when present.

For filesystem-dependent behavior, record evidence for the actual driver as well as the advertised type.
A mount type or filesystem magic alone is insufficient; check the kernel configuration and registration path.
For example, [`CONFIG_EXT4_USE_FOR_EXT2`](https://github.com/torvalds/linux/blob/v6.12/fs/ext4/Kconfig)
allows ext4 to register as `ext2` when the native ext2 driver is disabled.
Treat a run on a different or unconfirmed backend as non-comparable, even if it passes.

If matching the environment requires rebuilding or booting another kernel, changing host drivers,
or similarly complex setup, skip host validation, record why, and use the source-derived conclusion.
Resolve contradictions between source and observations before declaring an Asterinas bug.
If the source evidence itself is unavailable or inconclusive, leave diagnosis unfinished.

## Check that the fix addresses the mechanism

Trace the failure beyond its immediate trigger to the code responsible for the violated contract.
When a test derives inputs from a reported limit, inspect both the reporting path and the mechanism
that enforces that limit. Changing the reported value is sufficient only if actual behavior satisfies
the resulting contract, including at the new boundary and during repeated use.

Compare the Linux and Asterinas behavior needed to establish that contract.
For resource allocation this can include bounds, reuse after release, cycling, and exhaustion;
include concurrency or namespace behavior when it affects the invariant or changed code.
Fix missing behavior required by the contract even if changing a parameter alone passes the test.
Treat relevant FIXMEs or warnings as evidence to investigate, rather than dismissing them as pre-existing.
Asterinas may use different algorithms and data structures; unrelated subsystem gaps do not expand this fix.

Choose focused checks that could expose an incomplete fix beyond the original assertion.
Use source analysis and meaningful boundary or lifecycle tests as appropriate to the change;
for large limits, an allocator test with a small range can exercise the relevant behavior cheaply.
Record the compared paths, checks, and limitations in diagnose's `root_cause` / `invariant`
and fix's `generalizes`. A shared constant or the absence of test-specific branches is not proof of completeness.

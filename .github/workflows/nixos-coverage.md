# NixOS CI coverage

The full and ISO workflows build the kernel before building a NixOS image.
Their path filters must cover that build chain as well as the NixOS tests.
Run `python3 tools/github_workflows/test_nixos_paths.py` to check the filters
and execute the full workflow's actual selection shell.
The check requires PyYAML, Bash, Git and jq.
`Test CI paths / check-paths` runs it on every PR and main push.

## Build inputs and selection

| Inputs | Full workflow | ISO workflow |
| --- | --- | --- |
| Root Makefile, Cargo manifests/lock, `Components.toml`, `OSDK.toml`, `VERSION`, Rust toolchain and `.cargo/` | All 19 suites | Build/install ISO, then hello |
| `kernel/`, `ostd/`, `osdk/`, `test/initramfs/` | All 19 suites | Build/install ISO, then hello |
| `flake.nix`, `flake.lock`, `distro/nixpkgs.nix` | All 19 suites | Build/install ISO, then hello |
| `test/nixos/Makefile`, workspace manifest/lock, `test/nixos/common/` | All 19 suites | Build/install ISO, then hello |
| `tools/nixos/`, `tools/qemu_args.sh`, `tools/net/`, `tools/run_virtiofsd.sh` | All 19 suites | Build/install ISO, then hello |
| `.github/actions/test/` | All 19 suites (conservative coverage for shared action changes) | Build/install ISO, then hello (conservative) |
| Full workflow, selector helper or path regression script | All 19 suites | No additional ISO run |
| Other `distro/` changes | Existing selection: system-core if no suite selected; desktop changes also add DRM | Build/install ISO, then hello |
| `test/nixos/tests/<suite>/` | Corresponding suite; combine multiple suites | Hello files and every workspace member's `Cargo.toml` trigger ISO |
| Other files | Workflow filtered out | Workflow filtered out |

The kernel targets use `test/initramfs/Makefile` and its Nix/source inputs.
NixOS build scripts import `distro/nixpkgs.nix`, which reads `flake.lock`.
The initramfs Makefile reads the cache settings in `flake.nix`.
`Components.toml` is read by the component macros during kernel compilation.
The NixOS driver uses the shared framework, test macro and workspace lockfile.
Even hello resolves all Cargo workspace member manifests.

Common inputs take precedence over suite-local changes.
For example, changing the root Makefile together with networking selects all
19 suites, including networking.
The full workflow's outer filters and `FULL_TEST_PATH_PREFIXES` /
`INFRA_PATH_PREFIXES` must be updated together; the regression check verifies this.
File names match exactly and directory prefixes end in `/`.

PR comparisons use merge-base-to-head, matching GitHub's three-dot comparison.
Push comparisons use before-to-after, with root-commit support for a first push.
Rename detection is disabled when computing paths so both the source and
destination participate in selection; deleted inputs also retain their paths.
This fixes shared-input moves outside a selected directory.
Unknown or removed suite names and general selection-error fallback remain part
of the subsequent selector refactor.
GitHub evaluates workflow-level filters before this shell runs;
its native changed-file limits still apply.

## Original checks and configurations

| Entry / check | Existing configuration and coverage | Effect of this change |
| --- | --- | --- |
| `Test AsterNixOS (full)` / `select-test-suites` | PR/main path selection; daily 16:00 UTC (00:00 Beijing) all; manual all or one suite | More shared inputs trigger and select all; explicit selected-suite output |
| `Test AsterNixOS (full)` / `Test <suite>` | 19 dynamic suites on ubuntu-latest, kernel-dev container, `fail-fast: false`; each job builds its image before running its driver | Same matrix metadata, build/test commands and driver result |
| `Test AsterNixOS (minimal)` / `minimal-nixos-test` | Every PR/main push and manual; hello; 6144 MiB disk | Unfiltered entry and hello validation retained |
| `Test ISO image` / `iso-test` | Manual and filtered PR/main push; 4-core/150GB runner; `make iso ...hello`, `make run_iso`, installation success marker, `make run_nixos ...hello` | More actual inputs trigger; original three stages and result checks retained |
| `Test Nix flake` / `evaluate` | Includes `test/nixos/Makefile`; nixpkgs consistency, flake evaluation and benchmark-client evaluation | Existing coverage retained; evaluation does not replace full suites or ISO installation |
| `Test Nix flake` / `dev-shell-smoke` | x86 host with KVM and ARM host with TCG; `make check` and kernel boot | Existing coverage retained; kernel boot does not replace NixOS full/ISO validation |
| Dev cache / `push-pkgs` | PR dry run; push/manual publication; own workflow path fixed in #4162 | Existing behavior retained and independently regressed |
| Prebuilt cache / `push-prebuilt` | x86/ARM package builds; PR dry run; `flake.lock` added in #4162 | Existing behavior retained and independently regressed |
| `Test CI paths` / `check-paths` | New lightweight workflow, unfiltered PR/main push/manual | Validates paths/selection without building images; provides a terminal result for unrelated changes |

Full/minimal and ISO run an x86-64 dev kernel with SMP1 and KVM by the root
Makefile's defaults; NixOS/ISO build uses `linux-efi-handover64`.
The installed system runs with OVMF off through the original run target.
The test driver retains its own timeout and failure behavior.
These configurations are distinct from the generic architecture matrices.

The full suite matrix remains:

| Suite | Disk (MiB) | Build/test step timeout (min) | Driver timeout (min) |
| --- | ---: | ---: | ---: |
| system-core | 10240 | 45 | 10 |
| drm | 8192 | 45 | 10 |
| nix-and-nixos-tools | 8192 | 60 | 30 |
| containerization-and-virtualization | 10240 | 45 | 10 |
| networking | 10240 | 45 | 10 |
| web-servers-and-proxies | 10240 | 45 | 10 |
| databases-and-middleware | 10240 | 45 | 10 |
| development-tools | 10240 | 45 | 10 |
| development-tools-go | 8192 | 60 | 10 |
| development-tools-python | 8192 | 60 | 10 |
| development-tools-openjdk | 8192 | 60 | 10 |
| cicd-and-devops | 8192 | 45 | 10 |
| monitoring-and-observability | 8192 | 30 | 10 |
| web-browsers | 8192 | 30 | 10 |
| office-and-productivity | 8192 | 30 | 10 |
| multimedia | 8192 | 30 | 10 |
| communication | 8192 | 30 | 10 |
| file-management-and-terminal | 8192 | 30 | 10 |
| ai-and-machine-learning | 10240 | 45 | 10 |

Hello is exercised by minimal and ISO, outside this 19-suite matrix.
A hello-only selection returns `matrix=[]`, `run=false` and succeeds at the
selector; the full suite job is skipped.

Other original checks retain their definitions and names:

| Entry | Original jobs / coverage |
| --- | --- |
| x86 | 34: basic 4, OSDK 2, boot 4, regression 2, LTP/gVisor/kselftest 15, ARM-host cross boot 1, xfstests 4, pjdfstest 2 |
| AArch64 | 8: lint/compile and boot/LTP across x86 and ARM hosts |
| RISC-V | 8: basic 4, boot 2, LTP 2; parameter correction remains in its separate PR |
| LoongArch | 2: lint and compile |
| TDX | 9: OSDK 1, boot 2, regression 2, LTP 4 |
| xfstests full | Original ext2 full.list scheduled/manual entry |
| Firecracker, cargo-component, devcontainer, VMX | Original workflow entry points and checks |
| Licenses and SCML | Original PR/main entries and checks |

## Terminal results and required checks

Keep minimal and the lightweight path check unfiltered.
Unrelated changes still get these checks; full and ISO do not start.
When the selector itself receives unrelated paths, it emits an empty matrix and
`run=false` and finishes successfully.
Shared input changes emit all 19 suites and `run=true`.
Invalid manual suite names fail explicitly.

The branch rules visible on 2026-11-11 did not enforce required CI contexts.
This does not describe the team's manual merge requirements.
This change does not alter branch protection or make the filtered full/ISO
checks required.
An unfiltered entry or an always-running aggregation check is necessary before
requiring a filtered or dynamically named check.
The later terminal-record task owns any required-check migration.

To undo the path/selection change, revert its commit and rerun an appropriate
current-version check.
The revert restores the documented full/ISO input gaps.
The full workflow's existing manual `suite=all` remains available for a
selection-independent verification run.

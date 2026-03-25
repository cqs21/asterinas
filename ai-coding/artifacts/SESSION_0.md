# Session Dialogue Transcript

This document records the user-assistant dialogue from this session.
It includes user messages and assistant replies, but does not include
tool outputs or hidden system/developer instructions.

## Turn 1

### User

```markdown
# AGENTS.md instructions for /root/asterinas

<INSTRUCTIONS>
# Agents Guidelines for Asterinas

Asterinas is a Linux-compatible, general-purpose OS kernel
written in Rust using the framekernel architecture.
`unsafe` Rust is confined to OSTD (`ostd/`);
the kernel (`kernel/`) is entirely safe Rust.

## Repository Layout

| Directory    | Purpose                                                  |
|--------------|----------------------------------------------------------|
| `kernel/`    | Safe-Rust OS kernel (syscalls, VFS, networking, etc.)    |
| `ostd/`      | OS framework — the only crate permitted to use `unsafe`  |
| `osdk/`      | `cargo-osdk` CLI tool for building/running/testing       |
| `test/`      | Regression and syscall tests (C user-space programs)     |
| `distro/`    | Asterinas NixOS distribution configuration               |
| `tools/`     | Utility scripts (formatting, Docker, benchmarking, etc.) |
| `book/`      | The Asterinas Book (mdBook documentation)                |

## Building and Running

All development is done inside the project Docker container:

```bash
docker run -it --privileged --network=host --device=/dev/kvm -v /dev:/dev \
  -v $(pwd)/asterinas:/root/asterinas \
  asterinas/asterinas:0.17.1-20260317
```

Key Makefile targets:

| Command              | What it does                                         |
|----------------------|------------------------------------------------------|
| `make kernel`        | Build initramfs and the kernel                       |
| `make run_kernel`    | Build and run in QEMU                                |
| `make test`          | Unit tests for non-OSDK crates (`cargo test`)        |
| `make ktest`         | Kernel-mode unit tests via `cargo osdk test` in QEMU |
| `make check`         | Full lint: rustfmt, clippy, typos, license checks    |
| `make format`        | Auto-format Rust, Nix, and C code                    |
| `make docs`          | Build rustdocs for all crates                        |

Set `OSDK_TARGET_ARCH` to `x86_64` (default), `riscv64`, or `loongarch64`.

## Toolchain

- **Rust nightly** pinned in `rust-toolchain.toml` (nightly-2025-12-06).
- **Edition:** 2024.
- `rustfmt.toml`: imports grouped as Std / External / Crate
  (`imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`).
- Clippy lints are configured in the workspace `Cargo.toml`
  under `[workspace.lints.clippy]`.
  Every member crate must have `[lints] workspace = true`.

## Coding Guidelines

The full coding guidelines live in
`book/src/to-contribute/coding-guidelines/`.
Below is a condensed summary of the most important rules.

### General

- **Be descriptive.** No single-letter names or ambiguous abbreviations.
- **Explain why, not what.** Comments that restate code are noise.
- **One concept per file.** Split when files grow long.
- **Organize for top-down reading.** High-level entry points first.
- **Hide implementation details.** Narrowest visibility by default.
- **Validate at boundaries, trust internally.**
  Validate at syscall entry; trust already-validated values inside.

### Rust

- **Naming:** CamelCase with title-cased acronyms (`IoMemoryArea`).
  Closure variables end in `_fn`.
- **Functions:** Keep small and focused; minimize nesting (max 3 levels);
  use early returns, `let...else`, and `?`.
  Avoid boolean arguments — use an enum or split into two functions.
- **Types:** Use types to enforce invariants.
  Prefer enums over trait objects for closed sets.
  Encapsulate fields behind getters.
- **Unsafety:**
  - Every `unsafe` block requires a `// SAFETY:` comment.
  - Every `unsafe fn` or `unsafe trait` requires a `# Safety` doc section.
  - All crates under `kernel/` must have `#![deny(unsafe_code)]`.
    Only `ostd/` may contain unsafe code.
- **Modules:** Default to `pub(super)` or `pub(crate)`;
  use `pub` only when truly needed.
  Always use `workspace.dependencies`.
- **Error handling:** Propagate errors with `?`.
  Do not `.unwrap()` where failure is possible.
- **Logging:** Use `log` crate macros only (`trace!`..`error!`).
  No `println!` in production code.
- **Concurrency:** Establish and document lock order.
  Never do I/O or blocking under a spinlock.
  Avoid casual use of atomics.
- **Performance:** Avoid O(n) on hot paths.
  Minimize unnecessary copies, allocations, and `Arc::clone`s.
  No premature optimization without benchmark evidence.
- **Macros and attributes:** Prefer functions over macros.
  Suppress lints at the narrowest scope.
  Prefer `#[expect(...)]` over `#[allow(...)]`.
- **Doc comments:** First line uses third-person singular present
  ("Returns", "Creates"). End sentence comments with punctuation.
  Wrap identifiers in backticks.
- **Arithmetic:** Use checked or saturating arithmetic.

### Git

- **Subject line:** Imperative mood, at or below 72 characters.
  Common prefixes: `Fix`, `Add`, `Remove`, `Refactor`, `Rename`,
  `Implement`, `Enable`, `Clean up`, `Bump`.
- **Atomic commits:** One logical change per commit.
- **Separate refactoring from features** into distinct commits.
- **Focused PRs:** One topic per PR. Ensure CI passes before review.

### Testing

- **Add regression tests for every bug fix** (with issue reference).
- **Test user-visible behavior** through public APIs, not internals.
- **Use assertion macros**, not manual output inspection.
- **Clean up resources** after every test (fds, temp files, child processes).

### Assembly

- Use `.balign` over `.align` for unambiguous byte-count alignment.
- Add `.type` and `.size` for Rust-callable functions.
- Use unique label prefixes to avoid name clashes in `global_asm!`.

## Architecture Notes

- **Framekernel:** The kernel is split into a safe upper half (`kernel/`)
  and an unsafe lower half (`ostd/`).
  This is a hard architectural boundary — never add `unsafe` to `kernel/`.
- **Components** (`kernel/comps/`): block, console, network, PCI, virtio, etc.
  Each is a separate crate.
- **OSTD** (`ostd/`): memory management, page tables, interrupt handling,
  synchronization primitives, task scheduling, boot, and arch-specific code.
- **Architectures:** x86-64 (primary), RISC-V 64, LoongArch 64.
  Arch-specific code lives in `ostd/src/arch/` and `kernel/src/arch/`.

## CI

CI runs in the project Docker container with KVM.
Key test matrices:
- x86-64: lint, compile, usermode tests, kernel tests, integration tests
  (boot, syscall, general), multiple boot protocols, SMP configurations.
- RISC-V 64, LoongArch 64, and Intel TDX have dedicated workflows.
- License headers and SCML validation are also checked.

</INSTRUCTIONS><environment_context>
  <cwd>/root/asterinas</cwd>
  <shell>bash</shell>
  <current_date>2026-03-24</current_date>
  <timezone>Etc/UTC</timezone>
</environment_context>
```

```markdown
# Context from my IDE setup:

## Active file: test/initramfs/src/syscall/ltp/testcases/all.txt

## Open tabs:
- all.txt: test/initramfs/src/syscall/ltp/testcases/all.txt
- common.rs: kernel/src/fs/pipe/common.rs

## My request for Codex:
目前这个项目的LTP还有很多testcase无法通过，详见 [all.txt](test/initramfs/src/syscall/ltp/testcases/all.txt) ，帮我整理一份调试和开发的计划
```

### Assistant

我先看 `test/initramfs/src/syscall/ltp/testcases/all.txt` 的结构和现有标注，整理出失败类型、依赖模块和优先级，再给出一份可执行的调试与开发计划。

### Assistant

我在继续看 `all.txt` 的全量内容和 LTP 运行脚本，先确认这个文件里哪些 case 是启用、哪些是注释掉，以及当前测试入口怎样汇总结果。

### Assistant

接下来我会把 `all.txt` 里启用的 testcase 做一次静态分组，看看失败修复大致会落在哪些内核子系统上，这样计划不会停留在泛泛而谈。

### Assistant

`all.txt` 里还有一些纯注释行和分组说明，我在重新做一次解析，把启用集、注释集、以及按 syscall 前缀的聚类统计拉出来。

### Assistant

我已经拿到一个大概规模：`all.txt` 里当前启用了 475 个 LTP case，注释掉了 1000 多个。接下来补一层执行层面的信息，确认单测、批量跑、以及按文件系统过滤的现有支持，这样计划里能直接给出调试工作流。

### Assistant

现有脚本只负责批量跑并检查 `Total Failures: 0`，对单个失败 case 的隔离和重跑支持比较弱。我再补一层顶层运行入口，确保计划里把“如何复现”和“如何缩小范围”写得足够具体。

### Assistant

我再补一个关键点：当前仓库固定了哪一版 LTP，这会影响你后续分类问题时，哪些是“Asterinas 缺实现”，哪些是“测试假设更新了”。

### Assistant

我已经有足够上下文了。最后补一下关键文件的行号引用，方便把计划直接落到仓库里的具体入口和改动点。

### Assistant

我准备把 475 个启用 case 按内核子系统粗分一遍，给出一个更实用的推进顺序，而不是按字母表修。

### Assistant

还有一个细节值得纳入计划：顶层 `Makefile` 已经预留了 `EXTRA_BLOCKLISTS_DIRS`，这意味着可以把“临时隔离失败集”和“正式仓库 blocklist”分开管理，适合多人并行调试。

### Assistant

`EXTRA_BLOCKLISTS_DIRS` 现在只在 gVisor 跑法里生效，LTP 没接上。这其实是个很具体的开发项，可以放进计划里优先补齐，能明显降低单 case 调试成本。

### Assistant

如果工作区里已经有最近一次的 `qemu.log`，我可以顺手看一下失败集中在哪些 case 上，计划的优先级会更贴近当前状态。不存在也没关系。

### Assistant

先说结论：`all.txt` 更像“启用清单”，不是“失败清单”。LTP 打包时会直接过滤掉注释行，只把未注释的 case 放进 initramfs，见 [test/initramfs/src/syscall/ltp/Makefile](/root/asterinas/test/initramfs/src/syscall/ltp/Makefile#L16) 和 [all.txt](/root/asterinas/test/initramfs/src/syscall/ltp/testcases/all.txt#L1)。我本地静态数了一下，当前大约是 475 个启用、1048 个注释掉；`ext2` 和 `exfat` 还各自有额外 blocklist。并且当前工作区最近一次 `/tmp` 跑法已经出现 `Total Failures: 0`，见 [qemu.log](/root/asterinas/qemu.log#L14157)。所以主任务应该拆成两条线：一条守住现有启用集不回退，另一条系统性地把被注释或被 block 的 case 逐步启用。

**计划**
1. 先补调试基础设施，优先级最高。
- 给 LTP 跑法补“单 case / 子集运行”能力，比如支持 `LTP_CASES=fcntl07,fcntl08` 或 `LTP_CASE_FILE=/path/to/list`。现在的入口只会整批 `runltp -f syscalls`，见 [run_ltp_test.sh](/root/asterinas/test/initramfs/src/syscall/ltp/run_ltp_test.sh#L5)。
- 给 LTP runner 补失败清单和分 case 日志输出，做成和 gVisor 类似的 `fail_cases` 机制。现在 gVisor 已经有这套逻辑，LTP 没有，见 [run_gvisor_test.sh](/root/asterinas/test/initramfs/src/syscall/gvisor/run_gvisor_test.sh#L18)。
- 把 `EXTRA_BLOCKLISTS_DIRS` 接到 LTP 跑法里。顶层 `Makefile` 已经把这个参数传进内核命令行了，但目前只有 gVisor 真在用，见 [Makefile](/root/asterinas/Makefile#L94)。
- 每个 case 跑完清理工作目录，避免前一个 case 污染后一个 case。这个对文件系统、信号、权限类问题尤其重要。

2. 建立固定的三层复现顺序，先别直接大面积取消注释。
- 第一层先跑 `/tmp`，命令是 `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp`。
- 第二层跑 `ext2`，命令是 `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2`。
- 第三层跑 `exfat`，命令是 `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat`。
- 每次只解开 5 到 10 个“同一 syscall 家族”的 case，不要跨子系统混跑。这样失败归因才稳定。
- 每个失败按 5 类登记：`未实现 syscall/flag`、`errno 不对`、`权限/credential 语义不对`、`时序/阻塞/信号竞态`、`文件系统语义不对`。

3. 开发顺序按“收益 / 风险比”排，不按字母表排。
- 第一优先级是已经部分支持、但还能继续扩展覆盖的家族：`fcntl`、`open/openat`、`rename/link/statx`、`sched`、`clock_*`、`futex`、`signal/signalfd`、`epoll`。这些家族在 `all.txt` 里占比已经很高，修一处通常能解锁一串 case。
- 第二优先级是文件系统差异。当前 `ext2` blocklist 54 项、`exfat` blocklist 98 项，说明大量问题不是 syscall 入口不存在，而是 VFS/具体 FS 语义和 Linux 不一致。
- 第三优先级才是大功能缺口：`ioctl`、`inotify/fanotify`、新 mount API (`fsopen/fsmount/fspick`) 、`landlock`、`bpf`、`keyctl`、SysV IPC、`quotactl`。这些通常不是“小修小补”，需要单独立项。

4. 每一批的完成标准要固定，不然 backlog 会越修越乱。
- `/tmp` 先通过。
- 然后确认 `ext2` / `exfat` 是否也通过，能删 blocklist 就删。
- 把 case 正式加回 [all.txt](/root/asterinas/test/initramfs/src/syscall/ltp/testcases/all.txt)；如果只是某个文件系统还不稳，就只留在对应 blocklist。
- 这批里如果修的是内核 bug，要补回归测试；涉及底层内存、trap、futex、架构细节的问题，优先在 `ostd/` 修，不要把 `unsafe` 带进 `kernel/`。

**建议的第一周切入点**
- 先做 runner 改造：单 case、失败清单、额外 blocklist、per-case log。
- 然后从一个高收益家族开始，我建议 `fcntl` 或 `clock_*`。
- 跑完 `/tmp` 后立刻复测 `ext2` / `exfat`，不要等功能堆多了再回头看文件系统差异。

如果你要，我下一步可以直接帮你把这份计划落成一张可执行的 triage 表，或者先把 LTP 的单 case 调试入口补出来。

## Turn 2

### User

```markdown
# Context from my IDE setup:

## Active file: test/initramfs/src/syscall/ltp/testcases/all.txt

## Open tabs:
- all.txt: test/initramfs/src/syscall/ltp/testcases/all.txt
- common.rs: kernel/src/fs/pipe/common.rs

## My request for Codex:
把这份计划落实到一份task文档
```

### Assistant

我先确认仓库里适合放这类执行计划的位置和现有文档风格，然后直接落成一份可维护的 task 文档。

### Assistant

我准备把文档放在 LTP 测试目录旁边，避免把内部执行计划塞进书册导航里；先看一下该目录现有文档和命名习惯，再落文件。

### Assistant

我会在 `test/initramfs/src/syscall/ltp/` 下新增一份 `TASKS.md`，内容聚焦三件事：调试工作流、分阶段开发任务、每批任务的完成标准。

### Assistant

文档已经落下来了。我再做一次快速校对，确认路径、表述和任务分层没有明显问题。

### Assistant

任务文档已经落在 [test/initramfs/src/syscall/ltp/TASKS.md](/root/asterinas/test/initramfs/src/syscall/ltp/TASKS.md)。

内容已经整理成可执行的结构：先补 LTP 调试基础设施，再按 syscall family 分批启用，最后按优先级推进内核修复，并给出了每批任务的复现步骤、失败分类、完成标准和第一轮建议切入点。这里没有跑新的测试，只是把现有结论沉淀成文档。

如果你要，我下一步可以继续把这份文档再细化成一个 batch tracker，例如先把 `fcntl` 或 `clock_*` 拆成第一批具体 case 清单。

## Turn 3

### User

```markdown
<environment_context>
  <shell>bash</shell>
  <current_date>2026-03-25</current_date>
  <timezone>Etc/UTC</timezone>
</environment_context>
```

```markdown
把这个session的所有对话写到一个markdown文档里，放到 [TASKS.md](test/initramfs/src/syscall/ltp/TASKS.md) 同级目录
```

### Assistant

我会把本次会话里用户和助手的对话整理成一份逐条转录的 Markdown，放在 `test/initramfs/src/syscall/ltp/` 目录下，与 `TASKS.md` 同级。

### Assistant

我现在在落文档，只记录用户与助手的对话文本，不包含命令输出和隐藏的系统/开发者指令。

### Assistant

转录文件已经生成。我先做一次校对，把本轮新增部分里不准确或重复的助手话语修掉。

### Assistant

我在修正转录尾部：把误收进去的旧话删除，并把这轮真正新增的助手更新补全。

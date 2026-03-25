# Experiment Record

## 对话记录

- [Session 0](./artifacts/SESSION_0.md) 生成任务描述[TASKS.md](./artifacts/TASKS.md)
- [Session 1](./artifacts/SESSION_1.md) 根据任务清单，按照优先级完成任务，生成 debug 文档和commit。

## 遇到的问题

### subagent/git worktree 并行问题

可能会造成端口冲突或者OVMF_VARS.fd被污染，导致启动失败，导致 Agent 偏离原本任务

### 单个 agent 长时间运行会导致上下文缺失

- 不会使用make clean
- 忘记todolist

### 不是所有 testcase 都是内核 bug

`statx05-07` 和 `linkat02` 被明确记录为 blocker，而不是硬修：

- `statx05`、`statx06` 依赖 `ext4` 和 `mkfs.ext4`。
- `statx07` 依赖 NFS 和 `exportfs`。
- `linkat02` 在 ext2 设备准备阶段就失败。

Agent 需要学会停止，把“问题不在当前层”写清楚，而不是为了“看起来在推进”去制造伪修复。

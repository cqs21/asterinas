# Lessons Learned

## 1. 先修“调试系统”，再修 testcase

- 单 case / 小批量选择
- 更详细的失败信息

## 2. 不是所有 case 都值得立刻修

`statx05-07`、`rename11`、`renameat01`、`linkat02` 都说明一个事实：有些 case 当前不该“硬修”，而应该被明确归类为：

- 环境前置缺失，缺少文件`mkfs.ext*`
- 文件系统专属行为，未支持`NFS`

## 3. 每修一个 case，留下必要材料

每个 testcase 基本都伴随一份 debug 文档，便于后续复现、归因和review：

- 失败现象是什么
- 根因是什么
- 如何修复的

## 4. 真实的 blocker 可能不在代码本身

OVMF 启动问题、文件系统镜像准备失败，都一度阻塞 Agent。在整个调试开发过程中，Agent 必须同时关心两类问题：

- 代码层面的修复是否正确
- 测试运行环境是否稳定

忽略后者，Agent 的代码能力再强，也会频繁停在“无法有效验证”的状态。

## 5. 如何让 Agent 持续工作

```shell
while :; do cat PROMPT.md | claude ; done
```

repo: https://github.com/mikeyobrien/ralph-orchestrator

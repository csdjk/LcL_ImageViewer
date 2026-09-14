# LcL ImageViewer — 待决策事项

> 只记录必须由用户决定、会阻塞开发的产品或重要架构问题。普通 bug、实现细节、依赖任务和是否立即 push 不进入本文。

## 当前待决策

当前没有 OPEN 的待决策问题。

## 使用规则

遇到需要用户决定的问题时：

1. 创建 `ND-xxx`。
2. 将关联 Task 标记为 `BLOCKED`。
3. 写清问题、方案、影响、Agent 推荐和恢复条件。
4. 保存当前安全进度并停止，不代替用户决定。
5. 用户决定后把结论同步到 Task、spec 或 ADR，再标记 `RESOLVED`。

状态：`OPEN / RESOLVED / SUPERSEDED`。

## 条目模板

```markdown
## ND-001 — <问题标题>

**Status:** OPEN
**Related Task:** <TASK_ID>
**Raised:** YYYY-MM-DD

### 问题

<需要用户决定什么。>

### 可选方案与影响

- A：<方案、优缺点和影响>
- B：<方案、优缺点和影响>

### Agent 推荐

<推荐及理由。>

### 恢复条件

<用户做出什么决定并同步到哪里后恢复哪个 Task。>

### Decision

等待用户决策。
```

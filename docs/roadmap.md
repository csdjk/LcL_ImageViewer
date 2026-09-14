# LcL ImageViewer — 路线图

> 本文是阶段顺序、目标和 Stage Gate 的权威来源。当前进度见 `status.md`，具体执行见 `task-board.md`。

## 阶段顺序

```text
P0 Viewer Foundation
  ↓ VALIDATED
P1 Glass UI Stabilization
  ↓ VALIDATED
P2 Release Hardening
```

## P0 — Viewer Foundation

**状态：** VALIDATED

**目标：** 在 Windows x64 上提供可运行的轻量图片查看器，具备常见与游戏贴图格式解码、动画、通道/Mip/HDR 检查、缩放平移和 Explorer 集成基础。

已知证据：

- Git 标签 `v0.2.0` 与后续 `main` 提交；
- `cargo test --workspace` 在 2026-09-14 的当前 HEAD 上 10 项通过；
- README 已记录现有功能、构建与快捷键。

## P1 — Glass UI Stabilization

**状态：** ACTIVE

**目标：** 让当前玻璃界面实现具备可重复的 Windows 运行、视觉、交互、DPI、桌面捕获恢复和性能证据，修复验收中发现的范围内缺陷。

阶段任务顺序：

```text
IV-P1-01 UI 验收工具链
     ├──→ IV-P1-02 视觉/交互矩阵与缺陷闭环
     └──→ IV-P1-03 桌面捕获恢复与性能证据
                    ↓
             IV-P1-04 P1 集成门禁
```

阶段验收：

- 动态发现应用窗口，不依赖固定 HWND 或旧机器路径；
- 截图与结果包含 commit、输入、主题、状态、逻辑/物理客户区和 DPI 元数据；
- 深浅主题、窄/标准窗口、关键交互和弹层实际运行检查完成；
- 图片显示和像素取样不受玻璃材质污染；
- 桌面捕获在拖动、最小化/恢复和失败路径可观察且可恢复；
- 性能报告注明环境、样本数、缓存边界、中位数和 P95；
- `main` 上相关测试、构建和文档复验通过。

缺少真实运行截图或同条件证据时，P1 不得标记为 VALIDATED。

## P2 — Release Hardening

**状态：** BACKLOG

**进入条件：** P1 VALIDATED。

**目标：** 建立可重复的 release 构建、安装器、便携包、文件关联/缩略图与发布前回归流程。

阶段验收至少包括：

- `cargo build --release --workspace` 成功；
- 主程序和 `iv_shell.dll` 的最小安装/卸载回归；
- 安装包与便携包版本、内容和大小记录；
- Release 文档与实际产物一致；
- 真实发布和 Git push 仅在用户明确要求后执行。

## 当前不进入主线

- macOS / Linux 移植；
- 新增图片格式或重写解码架构；
- 为视觉调整更换 egui/wgpu 技术栈；
- 自动发布到 GitHub Releases。

这些事项需要新的需求、范围和验收，不从 P1 的 UI 稳定化任务中顺带扩展。

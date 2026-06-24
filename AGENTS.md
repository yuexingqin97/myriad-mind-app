# AGENTS.md

> 📍 **项目架构、目录结构、数据流、AI / Pipeline / 配置 / 密钥等细节一律以 [`docs/架构与结构.md`](docs/架构与结构.md) 为准，本文件不重复介绍。**

## 项目定位

独立桌面优先 App，把大衍决 Claude Code Skill 原型迁移为可配置、可视化、可扩展的学习笔记工具。丢入视频链接 / 文章 URL / 本地文件，自动炼化为结构化学习笔记。免费开源（MIT）、本地运行、无服务器。

🚧 **Alpha 阶段** — UI 前端已基本完成，Windows 桌面端优先；当前主线是接入 DeepSeek V4 Pro / Flash、打通 MindEngine 管线、分批迁移 Skill 能力。移动端 / macOS / Linux 延后至 v2。

---

## 一、开发工作流

### 1. 设计先行

以下任务必须先与开发者讨论方案、记录到 [`docs/设计文档/`](docs/设计文档/) 对应分类后，再开始编码：

- 新管线阶段 / 新 InputMode
- 新 Rust IPC 命令或 `commands/` 模块结构调整
- AI 路由（Pro/Flash）或 Prompt 体系变更
- 配置 Schema 或密钥存储模型变更
- UI 信息架构大改 / 新建共享组件
- 跨包（core / ui / desktop）接口调整

Plan 至少包含：目标与背景、方案设计、模块职责、数据流与调用关系、风险与兼容性、实施步骤。

未经方案确认，不得直接进入实现。

### 2. 文档优先

- 项目级设计、架构、流程记录在 `docs/*.md`。**架构与目录结构权威是 [`docs/架构与结构.md`](docs/架构与结构.md)**，模块设计见 [`docs/设计文档/README.md`](docs/设计文档/README.md)。
- 实现产生新的长期决策时，同步更新对应文档。
- 发现文档与实现不一致，显式指出并提出修正方案。
- 聊天记录不视为长期记忆，稳定结论必须沉淀到文档。

### 3. 编码与文档规范

本项目暂无独立规范文档，遵循以下既定约定：

- **注释与命名** — 代码用中文注释说明职责；文档用中文文件名，按 `docs/` 主题分目录。
- **packages/ui 禁止 Tailwind** — 用原生 CSS + 语义变量。
- **提示词外置** — AI 提示词放 `apps/desktop/src-tauri/prompts/*.md`，由 PromptManager 用 minijinja 渲染；**改提示词只改 .md，不动 Rust、不重编译**（详见架构与结构.md §4.3）。
- **Rust 错误统一** — 走 `error.rs::AppError`（thiserror），不散落字符串错误。
- **Python 脚本黑盒** — `scripts/*.py` 不改逻辑，Rust 只做类型化封装（`Command::new → output → 检查 exit → 解析 JSON`）。
- **链接而非复制** — 文档间用链接引用，不复制其他文档内容。

### 4. 计划与进度

- 复杂任务创建 / 更新 [`docs/项目管理/开发任务清单.md`](docs/项目管理/开发任务清单.md)，用 `[ ]` / `[x]` 跟踪。
- Trivial 任务（小 bug、变量调整、配置改动、提示词微调）可直接实施。

### 5. 一致性

代码优先遵循现有架构与文档约定。如发现 `docs/` 与当前需求冲突，必须显式说明冲突并与开发者确认修正方案，不允许默默忽略或自行改方向。

### 6. 架构变更需确认

涉及以下核心改动，禁止直接实现，必须先给至少一种推荐方案 + 优缺点分析，等开发者确认：

- `commands/` 模块树或 IPC 命令注册结构
- `pipeline.rs` 编排（4 条内容管线分流 / 管线阶段）
- `ai/engine.rs` 模型路由逻辑
- 配置 / 密钥存储模型（`config.rs`）
- `prompts/` 提示词体系结构
- `packages/core` 的公共导出接口

---

## 二、项目结构

| 目录 | 说明 |
|------|------|
| `packages/core/` | 共享纯逻辑（TS）：类型、Schema、分类、预估、AI 路由、修为计算 |
| `packages/ui/` | 共享 React 组件（ConfigWizard / SettingsPage / Dashboard / NoteRenderer / common） |
| `apps/desktop/` | Tauri 2.x 桌面端 — `src-tauri/`（Rust）+ `src/`（React） |
| `apps/mobile/` | React Native Expo（v2 延后，仅骨架） |
| `scripts/` | Python 脚本（6 个，git submodule，黑盒子子进程） |
| `docs/` | 项目文档（见下） |

> 完整目录树、Rust 模块树、数据流见 [`docs/架构与结构.md`](docs/架构与结构.md)。

### docs 目录结构

```
docs/
├── 架构与结构.md      # ⭐ 架构 / 目录 / 数据流 / AI / Pipeline / 配置 权威
├── 开发启动指南.md    # 本地启动、常用脚本、排错
├── 需求与排期.md      # 需求 / 任务拆解 / 时间评估
├── 设计文档/          # 各模块设计（AI与模型 / Skill迁移 / UI与交互 / 配置系统 / 数据与存储）
├── 学习资料/          # Tauri_React 入门等
├── 问题排查/          # B站下载诊断等
├── 项目管理/          # 开发任务清单、需求归档
└── 参考资料/          # 法律风险分析 + 免责模板
```

### 开发环境

| 项 | 版本 / 工具 |
|----|------------|
| OS | Windows 11 Pro, RTX 4070 Ti SUPER |
| 前端 | React 19 + TypeScript 5.9 + Vite 6 |
| 桌面壳 | Tauri 2.x（Rust 1.93+，edition 2024） |
| Python | 3.9+（仅桌面，跑 scripts/） |
| 媒体 | FFmpeg 8.x、yt-dlp |
| 包管理 | pnpm（workspace）、Cargo |

---

## 三、关键约束（产品红线）

1. **不内置任何 API Key 或 Cookie** — 用户自行申请第三方服务凭据
2. **不提供分享 / 发布功能** — 笔记默认私有
3. **不做商业化** — 不收费、不加广告、不做 SaaS
4. **保留 MIT 许可证和上游版权声明**
5. **README 顶部必须有免责声明** — 仅供个人学习 / 研究 / 教育
6. **宣传措辞避免** "免费下载""破解""绕过"等

详见 [`docs/参考资料/法律风险分析.md`](docs/参考资料/法律风险分析.md)。

---

## 四、上游依赖

大衍决 Skill 源码与笔记仍在原项目 `D:/Project/MyClaude/`：

- Skill 源码：`D:/Project/MyClaude/myriad-mind/`
- 笔记输出：`D:/Project/MyClaude/大衍决残卷/`
- 项目记忆：`C:/Users/Yxqin/.claude/projects/D--Project-MyClaude/memory/myriad-mind.md`

---

## 五、禁止操作

未经开发者明确授权，不得执行任何版本控制写操作：

- `git add` / `git commit` / `git push`
- `git merge` / `git rebase` / `git cherry-pick`

只允许读取版本控制信息（`git status` / `git diff` / `git log`）。所有变更保留在本地工作区，由开发者自行审核提交。

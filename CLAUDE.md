# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目定位

独立桌面优先 App，将大衍决 Claude Code Skill 原型迁移为可配置、可视化、可扩展的学习笔记工具。免费开源（MIT）、本地运行、无服务器。用户丢入视频链接/文章 URL/本地文件，自动炼化为结构化学习笔记。

## 当前状态

🚧 **Alpha 开发阶段** — UI 前端已基本完成，Windows 桌面端优先。当前主线是接入 DeepSeek V4 Pro / Flash、打通 MindEngine 管线，并把 Skill 原型能力分批迁移进 App。移动端、macOS/Linux 延后至 v2。

## 技术栈

| 平台 | 框架 | 后端 | 前端 |
|------|------|------|------|
| Windows / Mac | Tauri 2.x | Rust | React 19 + TypeScript |
| Android / iOS | React Native Expo (v2) | TypeScript | React Native |
| 鸿蒙 | PWA / ArkTS WebView (v2+) | — | React |

**关键版本**: Rust 1.93+, Node.js 24+, TypeScript 5.9+, React 19+, Tauri 2.x, Python 3.9+ (仅桌面), FFmpeg 8.x (仅桌面)

## 架构核心

### Monorepo 结构 (pnpm workspace)

```
myriad-mind-app/
├── packages/
│   ├── core/          # 共享纯逻辑 — 配置 Schema/Prompt 模板/笔记解析/面板计算/搜索/灵力预估
│   └── ui/            # 共享 React UI — ConfigWizard/NoteRenderer/Dashboard/common
├── apps/
│   ├── desktop/       # Tauri 2.x — src-tauri/ (Rust) + src/ (React)
│   └── mobile/        # React Native Expo
└── scripts/           # Python 脚本 (git submodule) — 6 个脚本作为黑盒子子进程调用
```

**设计原则**: 核心逻辑只写一次 (`packages/core/`)，平台差异各自实现；Python 脚本不动，作为独立子进程调度。

### 桌面端数据流 (完整 10 步管线)

```
用户输入 → 模式识别 (8种) → 灵力预估 → 用户确认
  → Python 脚本管线 (下载→音频提取→ASR→关键帧)
  → MindEngine 调用 DeepSeek V4 Pro / Flash 流式生成笔记
  → 清理临时文件 → 更新修为面板
```

Rust 后端通过 `commands/` 模块暴露 Tauri IPC 命令，每个 Python 脚本遵循统一模式：`Command::new → .output() → 检查 exit code → 解析 JSON stdout`。进度通过 Tauri events 实时推送到 React 前端。

### 移动端功能裁剪

移动端 v1 不做；v2 再考虑轻量阅读、配置同步和文章处理。移动端不跑重型视频/ASR 工具，也不作为当前排期依据。

### 安全模型

- `myriad-mind-config.json` — 非敏感配置 (功能开关/输出路径/模型参数)
- **OS 密钥链** — 所有 API Key/Token (DeepSeek Key, AI Douyin Key, TikHub Token)，永不明文存储
- Windows: Credential Manager / macOS: Keychain / Linux: libsecret / Android: Keystore / iOS: Keychain
- 配置文件有版本号 (`version: 1`) 支持迁移

### 笔记存储

纯 `.md` + `assets/{VIDEO_ID}/` 截图子目录。SQLite (FTS5) 轻量索引加速搜索和面板统计。不做同步服务器 — 用户把输出目录选在云盘文件夹即可跨设备。

## 上游依赖

大衍决 Skill 源码和笔记仍在原项目 `D:/Project/MyClaude/`：
- Skill 源码: `D:/Project/MyClaude/myriad-mind/`
- 笔记输出: `D:/Project/MyClaude/大衍决残卷/`
- 项目记忆: `C:/Users/Yxqin/.claude/projects/D--Project-MyClaude/memory/myriad-mind.md`

## 关键约束

1. **不内置任何 API Key 或 Cookie** — 用户自行申请所有第三方服务凭据
2. **不提供分享/发布功能** — 笔记默认私有
3. **不做商业化** — 不收费、不加广告、不做 SaaS
4. **保留 MIT 许可证和上游版权声明**
5. **README 顶部必须有免责声明** — 工具仅供个人学习/研究/教育
6. **宣传措辞避免** "免费下载""破解""绕过"等

详见 `docs/参考资料/法律风险分析.md` 完整风险分析。

## 现有文档

- `README.md` — 项目介绍（做什么、特点、快速开始）
- `docs/项目结构.md` — Monorepo 目录结构、各包职责、模块依赖关系、数据流
- `docs/架构设计.md` — 详细架构设计（含 Rust 模块树、Python 调度代码示例、Zod Schema 完整定义）
- `docs/参考资料/法律风险分析.md` — 法律风险分析 + 免责模板
- `docs/需求与排期.md` — 需求整理 & 任务拆解 & 时间评估
- `docs/项目管理/开发任务清单.md` — 当前开发进度跟踪
- `.gitignore` — 注意 `myriad-mind-config.json` 被忽略（含占位示例文件），密钥文件 (.jks/.p8/.key/.mobileprovision) 被忽略

## 开发环境

- Windows 11 Pro, RTX 4070 Ti SUPER
- Rust 1.93, Node.js 24, Python 3.12, FFmpeg 8.1.1, yt-dlp
- pnpm (包管理), Cargo (Rust 构建)
- Flutter/Dart 未安装 (选了 Tauri + RN 方案，暂不需要)

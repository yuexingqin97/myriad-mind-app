# Code Review: Python → Rust 迁移（第1批：2 个脚本）

> **审查日期**: 2026-06-24  
> **审查范围**: 7 文件修改 + 2 新文件，共计 ~700 行变更  
> **迁移目标**: `list_ai_douyin_tasks.py` + `extract_keyframes.py` → Rust 直连

---

## 变更摘要

| 文件 | 变更类型 | 核心内容 |
|------|---------|---------|
| `commands/ai_douyin.rs` | **新增** (~195行) | AI Douyin API Rust reqwest 直连；含 `list_ai_douyin_tasks` Tauri 命令 + `resolve_via_ai_douyin` 解析函数 |
| `commands/media.rs` | **新增** (~430行) | FFmpeg 直连关键帧提取；含 `extract_keyframes` Tauri 命令 + `extract_keyframes_direct` 内部函数；支持 interval/scene/both/guided 四种模式 |
| `commands/python.rs` | **修改** (-110行) | 移除已迁的 `extract_keyframes` / `list_ai_douyin_tasks` 命令及类型定义 |
| `commands/pipeline.rs` | **修改** (-47行) | `extract_keyframes_guided` 切换为调用 `media::extract_keyframes_direct` |
| `commands/mod.rs` | **修改** (+2行) | 新增 `pub mod ai_douyin; pub mod media;` |
| `commands/tools/handlers/acquire.rs` | **修改** (-18行) | 导入路径从 `python::list_ai_douyin_tasks` 改为 `ai_douyin::list_ai_douyin_tasks`；移除 `python_path` 参数 |
| `lib.rs` | **修改** (+14/-8行) | 注册新命令、移除旧 import |
| `tauri.conf.json` | **修改** | 资源清单从 `../../../scripts/*.py` 改为逐个列出保留的 4 个脚本 |
| `api.ts` | **修改** (-2行) | `listAiDouyinTasks` 移除 `pythonPath` 参数 |
| `docs/设计文档/工程化/Python到Rust迁移计划.md` | **修改** (+78行) | 新增 §六 本次实施方案 |

---

## 影响面分析

```
list_ai_douyin_tasks (ai_douyin.rs)
  ├─ 调用方: lib.rs::generate_handler! (Tauri IPC 注册)
  │          tools/handlers/acquire.rs::QueryAiDouyinHandler
  ├─ 上游: AI Douyin HTTP API (GET /api/v1/tasks)
  ├─ 安全提升: api_key 从命令行 --api-key argv 改为内存中 HTTP Header
  └─ 日志: log::debug!("[douyin] phase=list_tasks ...")

extract_keyframes (media.rs)
  ├─ Tauri 命令版: lib.rs::generate_handler! → extract_keyframes (IPC)
  ├─ 内部调用版: pipeline.rs::extract_keyframes_guided → extract_keyframes_direct
  │               tools/handlers/analyze.rs → pipeline::extract_keyframes_guided
  ├─ 上游: FFmpeg 子进程 (std::process::Command)
  ├─ 模式: interval (fps=1/N) / scene (select+showinfo 两遍法) / both / guided
  └─ 输出: frames/*.png + keyframes.json 索引
```

---

## 问题分级清单

### 🔴 Critical — 无

### 🟠 High — 无

### 🟡 Medium — 2 项

1. **`resolve_via_ai_douyin` 暂未使用** — 在 `ai_douyin.rs` 中预置了 `resolve_via_ai_douyin` 函数（对应迁移计划 §4 的 `download_video_candidates.py` API 查询部分迁移），但当前 `pipeline.rs::download_douyin_video` 仍使用旧的 `resolve_via_ai_douyin` 函数（`pipeline.rs` 中同名函数）。后续迁移 `download_video_candidates.py` 时需统一。
   - **影响**: 目前无影响，两处函数独立。
   - **处理**: 标记为 TODO，待第2批迁移时统一。

2. **`extract_keyframes` Tauri 命令签名变化** — 旧版命令接收 `python_path` 参数（已移除）。如前端有直接调用此 IPC 命令的代码（当前 `api.ts` 中未发现），需同步更新。
   - **影响**: 已检查 `api.ts`，前端无直接调用 `extract_keyframes` IPC 命令（pipeline 内部使用 `extract_keyframes_guided`），无兼容性问题。

### 🟢 Low — 1 项

1. **`ai_douyin.rs` 中 `AiDouyinResolveResponse` 等类型未使用** — 为后续迁移预置的类型定义，目前仅产生 dead_code warning。可接受。

---

## 编译验证

```
cargo check: 0 errors, 33 warnings (均为预先存在的 dead_code/unused 警告)
pnpm run build (前端): 76 modules, built in 590ms ✅
```

---

## 追问与诊断（开发者 QA）

### Q1: `pnpm run build` 报 `Cannot find module '@myriad-mind/ui'/'@myriad-mind/core'`

| 项目 | 内容 |
|------|------|
| **现象** | `tsc && vite build` 时 TypeScript 找不到 workspace 依赖包 |
| **根因** | `packages/core/dist/` 和 `packages/ui/dist/` 不存在 — 首次构建需先编译依赖包 |
| **修复** | `pnpm --filter @myriad-mind/core build` → `pnpm --filter @myriad-mind/ui build` → `pnpm run build` |
| **结论** | monorepo 正常构建流程，非迁移 bug |

### Q2: 视频下载失败（端到端测试）

| 项目 | 内容 |
|------|------|
| **现象** | 炼化 B站视频 `BV1BG41117xB`，最终笔记仅有元数据无实际视频内容 |
| **链路** | `query_ai_douyin` → AI Douyin HTTP 400 captcha → `download_video_candidates.py` 失败 → yt-dlp 412 + cookie 错误 → 降级生成 |
| **分析** | 详见下方完整链路追踪 |
| **结论** | 与本次迁移无关。`download_video_candidates.py` 是未迁移脚本（迁移计划 §4 "⚠️ 半迁"），失败来自 yt-dlp + B站反爬。`list_ai_douyin_tasks` Rust 迁移工作正常（成功连接、返回正确错误信息、非崩溃） |

#### 失败链路详细追踪

```
[13:41:46] query_ai_douyin (Rust reqwest ✅)
  → endpoint=https://ai-douyin.top9.cc/api/v1/tasks page=1 search=BV1BG41117xB
  → proxy(http://127.0.0.1:7897/) tunneling HTTPS
  → ❌ HTTP 400: {"error":"搜索前请完成人机验证"}
  → AI 收到失败反馈，继续尝试其他路径

[13:42:08] download_video 工具被 Agent 调用
  → resolve_via_ai_douyin ✅ 解析出 download_url.json
  → download_video_candidates.py ❌ exit_code=1 duration_ms=17420
    stderr_summary="Trying candidate 1: ***"  (密钥脱敏生效)
  → yt-dlp 裸跑 ❌ HTTP 412 (B站反爬)
  → yt-dlp --cookies-from-browser edge ❌
    "Could not copy Chrome cookie database" (Chrome 运行中锁文件)

[13:43:54] Agent 终止，笔记降级生成
  → 仅有 title/author/duration 元数据，无字幕/截图/视频内容
```

#### 责任归属

| 组件 | 归属 | 状态 |
|------|------|------|
| `list_ai_douyin_tasks` (Rust) | ✅ 本次迁移 | 正常工作，上游 captcha 是业务问题 |
| `extract_keyframes` (Rust) | ✅ 本次迁移 | 未触发（视频没下载下来，管线提前终止） |
| `download_video_candidates.py` | ⚠️ 未迁移 | 候选 URL 下载失败 |
| yt-dlp B站 cookies | ❌ 环境问题 | Chrome 锁库、B站反爬 |

---

## 编译验证

```
cargo check: 0 errors, 33 warnings (均为预先存在的 dead_code/unused 警告)
pnpm run build (前端): 76 modules, built in 590ms ✅
```

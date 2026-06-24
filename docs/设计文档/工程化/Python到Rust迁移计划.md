# Python 到 Rust 迁移计划

> 编写日期：2026-06-22
> 状态：设计完成，待实施（后台进行，非当前主线）
> 关联：日志脚手架（已落地，迁移后子进程调度日志更清晰）

---

## 一、背景

当前 6 个 Python 脚本中有 3 个是"纯中转层"——Python 只是把参数传给 FFmpeg / HTTP API / venv，本身没有不可替代的 Python 生态依赖。迁到 Rust 直接调用，减少依赖、省子进程开销、日志更清晰。

---

## 二、6 脚本分类

| 脚本 | 核心依赖 | 能迁 | 理由 |
|------|---------|------|------|
| `list_ai_douyin_tasks.py` | HTTP API | ✅ 迁 | `reqwest` GET 几行 |
| `extract_keyframes.py` | FFmpeg 命令行 | ✅ 迁 | Rust 已调 FFmpeg（音频提取），截图同理 |
| `install_faster_whisper.py` | `python -m venv` + `pip install` | ✅ 迁 | 低频操作，Rust 调子进程即可 |
| `download_video_candidates.py` | AI Douyin API + yt-dlp | ⚠️ 半迁 | API 查询迁，下载保留 yt-dlp |
| `transcribe_faster_whisper.py` | faster-whisper（whisper.cpp） | ❌ 保留 | whisper 无 Rust 等价实现 |
| `download_youtube_subtitles.py` | yt-dlp | ❌ 保留 | yt-dlp 是 Python 项目 |

---

## 三、迁移顺序（优先级）

### 1. `list_ai_douyin_tasks.py` → `commands/ai_douyin.rs`（最简单）

- 现状：Python 调 `requests.get(AI_DOUYIN_BASE/tasks)`，返回 JSON
- 迁 Rust：`reqwest::get(url)` + `serde_json::from_str`，新增 Tauri 命令 `list_ai_douyin_tasks`
- 删：`scripts/list_ai_douyin_tasks.py`、`python.rs` 里对应的命令
- 受益：少一次 `Command::new("python")` 中转，日志直接 `log::debug!("[douyin] phase=list_tasks")`

### 2. `extract_keyframes.py` → `commands/media.rs` 或 `pipeline.rs` 内联

- 现状：Python 用 `subprocess.run(["ffmpeg", ...])` 截图，参数由 JSON 配
- 迁 Rust：`Command::new("ffmpeg").args([...])`，参数与当前脚本完全一致
- 删：`scripts/extract_keyframes.py`、`python.rs` 里对应命令
- 受益：Rust 已调 FFmpeg 提取音频（`pipeline.rs` 里 `run_ffmpeg_extract_audio`），截图是同类操作，统一路径

### 3. `install_faster_whisper.py` → `commands/python.rs` 内联

- 现状：`python -m venv` 创建 .venv → `pip install faster-whisper`
- 迁 Rust：`Command::new("python").args(["-m", "venv", ...])` → `.args(["-m", "pip", "install", ...])`
- 删：`scripts/install_faster_whisper.py`
- 受益：和现有 `check_python_env` 风格一致

### 4.（可选）`download_video_candidates.py` 的 API 查询部分

- AI Douyin 的查询/解析接口用 Rust reqwest
- 实际下载保留 Python（yt-dlp）

---

## 四、迁移后剩余

- `transcribe_faster_whisper.py` — whisper 无替代，保留
- `download_youtube_subtitles.py` — yt-dlp 无替代，保留
- `download_video_candidates.py` — yt-dlp 下载部分保留

→ 最终从 6 脚本减到 2.5 脚本，Python 仅用于 whisper + yt-dlp 硬依赖。

---

## 五、注意事项

- 迁 FFmpeg 截图时，参数（`-vf fps=...` / `scene_threshold`）与现有脚本保持一致，避免行为差异
- 删 Python 脚本后，更新 `tauri.conf.json` 的 `bundle.resources`（去掉不再打包的 .py）
- 每次迁一个脚本，`cargo check` + 跑一次对应的管线分流验证，逐脚本推进

---

## 六、本次实施（2026-06-24）

### 实施范围

一次性完成前 2 个脚本迁移：

| 脚本 | 目标模块 | 核心方案 |
|------|---------|---------|
| `list_ai_douyin_tasks.py` | `commands/ai_douyin.rs` | `reqwest::Client` 直调 AI Douyin API，替代 Python `urllib.request` |
| `extract_keyframes.py` | `commands/media.rs` | `std::process::Command` 直调 FFmpeg，替代 Python `subprocess.run` |

### 影响范围

**新增文件：**
- `apps/desktop/src-tauri/src/commands/ai_douyin.rs` — AI Douyin 任务查询（纯 reqwest HTTP）
- `apps/desktop/src-tauri/src/commands/media.rs` — 关键帧提取（纯 FFmpeg 子进程）+ FFmpeg 定位

**修改文件：**
- `commands/mod.rs` — 添加 `pub mod ai_douyin;` + `pub mod media;`
- `lib.rs` — 改 import 源
- `commands/python.rs` — 删除 `extract_keyframes`、`list_ai_douyin_tasks` 命令及相关类型
- `commands/pipeline.rs` — `extract_keyframes_guided` 改为调用 `media.rs` 函数，不再走 Python 子进程
- `commands/tools/handlers/acquire.rs` — `QueryAiDouyinHandler` 调用新函数签名（去除 `python_path`）
- `apps/desktop/src/api.ts` — `listAiDouyinTasks` 去除 `pythonPath` 参数

**删除文件：**
- `scripts/list_ai_douyin_tasks.py`
- `scripts/extract_keyframes.py`

### 行为一致性保证

| 维度 | 保证方式 |
|------|---------|
| 参数 | 新函数签名逻辑等价：`list_ai_douyin_tasks(api_key, api_base?, page?, page_size?, status?, search?)` |
| 输出 | JSON 结构完全相同（`AiDouyinTaskList { #[serde(flatten)] data: Value }` / `KeyframeResult { result: KeyframeData }`） |
| 错误语义 | HTTP 错误 → `AppError::Http`，FFmpeg 失败 → `AppError::Other`，同原 Python stderr 信息 |
| 日志 | 保留 `target: "agent"` 埋点，格式一致（`[douyin]` / `[media]` prefix） |

### 回滚策略

1. `git checkout` 恢复被删的两个 `.py` 文件
2. 恢复 `python.rs` 中两段代码（从 git history）
3. 恢复 `lib.rs` / `mod.rs` / `acquire.rs` / `api.ts` 的 import 改动
4. 删除新增的 `ai_douyin.rs` 和 `media.rs`
5. `cargo check` 验证

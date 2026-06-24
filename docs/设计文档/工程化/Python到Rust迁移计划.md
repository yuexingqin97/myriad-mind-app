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

## 六、本次实施方案（2026-06-24）

> 目标：完成迁移顺序 **#1 `list_ai_douyin_tasks.py`** 与 **#2 `extract_keyframes.py`** 的 Rust 直连迁移。  
> 状态：设计中 → 实现中

### 6.1 范围与决策

| 项目 | 决策 |
|------|------|
| 迁移脚本 | `list_ai_douyin_tasks.py`、`extract_keyframes.py` |
| 保留脚本 | `transcribe_faster_whisper.py`、`download_youtube_subtitles.py`、`download_video_candidates.py`、`install_faster_whisper.py` |
| 新增 Rust 模块 | `commands/ai_douyin.rs`（HTTP 直连）、`commands/media.rs`（FFmpeg 直连） |
| 命令注册 | `list_ai_douyin_tasks` 改由 `ai_douyin.rs` 注册；`extract_keyframes` 改由 `media.rs` 注册 |
| Python 脚本文件 | 删除 `scripts/list_ai_douyin_tasks.py`、`scripts/extract_keyframes.py` |
| 打包资源 | `tauri.conf.json` 仍使用 `scripts/*.py` 通配，删除文件后自动不再打包；无需额外修改 |
| 前端 API | `api.ts` 中 `listAiDouyinTasks` 移除已废弃的 `pythonPath` 参数 |

### 6.2 模块职责与数据流

```text
前端 / Agent Tool Handler
    │
    ├─ listAiDouyinTasks(apiKey, opts) ──▶ commands/ai_douyin.rs::list_ai_douyin_tasks
    │                                        reqwest GET /api/v1/tasks
    │                                        返回 { data: serde_json::Value }
    │
    └─ extract_keyframes(...) ──▶ commands/media.rs::extract_keyframes
                                   直接 spawn ffmpeg（interval / scene / guided）
                                   写 output_dir/frames/keyframes.json
                                   返回 KeyframeResult
```

- `acquire.rs` 中 `query_ai_douyin` handler 改调用 `commands/ai_douyin::list_ai_douyin_tasks`，不再传递 `python_path`。
- `analyze.rs` 中 `ExtractKeyframesHandler` 改调用 `commands/media::extract_keyframes_guided`，不再传递 `python_path`。
- `pipeline.rs` 中原 `extract_keyframes_guided` 移除；`analyze.rs` 直接引用 `media.rs`。

### 6.3 行为一致性要求

#### `list_ai_douyin_tasks`

- 默认 `api_base = "https://ai-douyin.top9.cc"`。
- endpoint 构建规则与 Python 脚本完全一致：
  - 以 `/api/v1` 结尾 → `+/tasks`
  - 以 `/api` 结尾 → `+/v1/tasks`
  - 其它 → `+/api/v1/tasks`
- query 参数：`page`、`pageSize`、可选 `status` / `search`。
- header：`X-API-Key: {api_key}`，timeout 30s。
- 返回结构保持 `AiDouyinTaskList { #[serde(flatten)] data: Value }`。
- 错误不回显原始响应中的密钥；HTTP 失败走 `AppError::Http` / `AppError::Other`。

#### `extract_keyframes`

- 完全复刻 `extract_keyframes.py` 的 CLI 默认与逻辑：
  - `mode` 默认 `both`；`interval` 默认 `30`；`max_frames` 默认 `40`
  - `scene_threshold=0.25`、`min_gap=3`、`max_gap=120`
  - `both` 模式下 scene 上限为 `max(max_frames // 3, 5)`
- 输出目录结构：`{output_dir}/frames/` 下放置 PNG + `keyframes.json`。
- 文件名规则：
  - interval：`frame_0001.png` …
  - scene：`scene_0001_12_3s.png`（与 Python 的 `{ts:.1f}s` 转 `_` 一致）
  - guided：`guided_0001_{slug_reason}.png`
- timestamp_label：四舍五入到秒；小时>0 用 `00h00m00s`，否则 `00m00s`。
- 对 guided JSON 支持数组数字或 `{ts,timestamp,timestamp_seconds,reason}` 对象；按 2s 去重。
- 最终 keyframes 按 `timestamp_seconds` 排序、按文件名去重、截断到 `max_frames`。

### 6.4 错误处理

- 统一走 `error.rs::AppError`：
  - FFmpeg 找不到 → `MissingDependency`
  - FFmpeg 命令失败 → `Other`（含 stderr 前 500 字符）
  - JSON 解析失败 → `Json`
  - HTTP 失败 → `Http` / `Other`

### 6.5 影响文件清单

新增：

- `apps/desktop/src-tauri/src/commands/ai_douyin.rs`
- `apps/desktop/src-tauri/src/commands/media.rs`

修改：

- `apps/desktop/src-tauri/src/commands/mod.rs`（注册新模块）
- `apps/desktop/src-tauri/src/lib.rs`（命令注册与 import）
- `apps/desktop/src-tauri/src/commands/python.rs`（移除两个命令）
- `apps/desktop/src-tauri/src/commands/pipeline.rs`（移除旧 `extract_keyframes_guided`）
- `apps/desktop/src-tauri/src/commands/tools/handlers/acquire.rs`（改 import / 调用）
- `apps/desktop/src-tauri/src/commands/tools/handlers/analyze.rs`（改 import / 调用）
- `apps/desktop/src/api.ts`（`listAiDouyinTasks` 签名）
- `scripts/README.md`（脚本清单更新）
- `docs/架构与结构.md` §八（脚本数量与说明更新）
- 本文档（状态与实施记录）

删除：

- `scripts/list_ai_douyin_tasks.py`
- `scripts/extract_keyframes.py`

### 6.6 回滚策略

1. 代码回滚：
   ```bash
   git checkout -- apps/desktop/src-tauri/src/commands/mod.rs \
                     apps/desktop/src-tauri/src/lib.rs \
                     apps/desktop/src-tauri/src/commands/python.rs \
                     apps/desktop/src-tauri/src/commands/pipeline.rs \
                     apps/desktop/src-tauri/src/commands/tools/handlers/acquire.rs \
                     apps/desktop/src-tauri/src/commands/tools/handlers/analyze.rs \
                     apps/desktop/src/api.ts \
                     scripts/README.md \
                     docs/架构与结构.md \
                     docs/设计文档/工程化/Python到Rust迁移计划.md
   ```
2. 恢复脚本：
   ```bash
   git checkout -- scripts/list_ai_douyin_tasks.py scripts/extract_keyframes.py
   ```
3. 删除新增文件：
   ```bash
   rm apps/desktop/src-tauri/src/commands/ai_douyin.rs \
      apps/desktop/src-tauri/src/commands/media.rs
   ```
4. 重新 `cargo check` 验证可编译。

### 6.7 验证清单

- [x] `cargo check` 在 `apps/desktop/src-tauri` 下 0 错误
- [x] `commands/ai_douyin.rs` 单元测试通过（endpoint 构建）
- [x] `commands/media.rs` 单元测试通过（timestamp_label / slug_reason / guided 解析 / FFmpeg end-to-end）
- [x] 删除的两个 `.py` 文件不再被 `tauri.conf.json` resources 打包（glob 匹配剩余 4 个脚本）
- [x] `api.ts` 类型检查通过（`pnpm -F @myriad-mind/desktop typecheck`）

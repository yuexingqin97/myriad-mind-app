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

## 六、本次实施方案（2026-06-24 落地）

> 本次完成迁移顺序前 2 个脚本：`list_ai_douyin_tasks.py` 与 `extract_keyframes.py`。

### 6.1 Codegraph 调用链（迁移前）

#### `list_ai_douyin_tasks` 调用链

```
前端 api.ts:192  listAiDouyinTasks()
  └─Tauri IPC─▶ python.rs:602  list_ai_douyin_tasks (#[tauri::command])
                 └─ run_and_parse → run_python_script
                      └─ Command::new(python) scripts/list_ai_douyin_tasks.py --api-key <key> --json ...
                           └─ Python urllib.request GET {base}/api/v1/tasks?page=...&pageSize=...
                                └─ 返回 JSON 响应对象

Agent 工具 acquire.rs:619  QueryAiDouyinHandler
  └─ python::list_ai_douyin_tasks() (直接调用，非 IPC)
       └─ 同上
```

#### `extract_keyframes` 调用链

```
Agent 工具 analyze.rs:55  ExtractKeyframesHandler
  └─ pipeline.rs:847  extract_keyframes_guided()  [同步 block_in_place]
       └─ run_python_script("extract_keyframes.py") --video ... --mode scene --max-frames 40 ...
            └─ Python: ffmpeg 场景检测(showinfo) + 逐帧截图 + keyframes.json
                 └─ 返回 JSON {result: {keyframes: [...]}}

Tauri IPC "extract_keyframes" (lib.rs 注册，无前端调用方)
  └─ python.rs:452  extract_keyframes (#[tauri::command])
       └─ run_and_parse → run_python_script (同上)
```

### 6.2 实施方案

#### 脚本 1：`list_ai_douyin_tasks.py` → `commands/ai_douyin.rs`

| 项 | 内容 |
|----|------|
| 新增模块 | `commands/ai_douyin.rs` |
| 迁移方式 | `reqwest::Client` GET + `X-API-Key` 头，解析 JSON 响应 |
| 端点构造 | 复刻 Python `build_tasks_endpoint` 逻辑：trim `/`，`/api/v1` 结尾 → `/tasks`，`/api` 结尾 → `/v1/tasks`，否则 → `/api/v1/tasks` |
| 默认 base | `https://ai-douyin.top9.cc`（与 Python 脚本一致） |
| 查询参数 | `page`, `pageSize`, 可选 `status`, `search` |
| 返回类型 | `AiDouyinTaskList { #[serde(flatten)] data: serde_json::Value }` — 从 python.rs 迁移到此模块 |
| Tauri 命令签名 | 保持不变（含 `python_path` 参数，保留但不再使用，避免前端 IPC 契约破坏） |
| 错误处理 | HTTP 失败 → `AppError::Other`（含状态码 + 响应体摘要，脱敏 api-key）；网络错误 → `AppError::Http`；JSON 解析失败 → `AppError::Json` |
| 日志 | `log::debug!("[douyin] phase=list_tasks")`，不记 api-key |
| 调用方更新 | `acquire.rs` import 改为 `crate::commands::ai_douyin::list_ai_douyin_tasks` |

#### 脚本 2：`extract_keyframes.py` → `commands/pipeline.rs` 内联

| 项 | 内容 |
|----|------|
| 实现位置 | `commands/pipeline.rs`（复用已有 `resolve_ffmpeg_binary` + `apply_windows_no_window`） |
| 迁移方式 | `std::process::Command::new("ffmpeg")` 直接调用，复刻 Python 三种模式 |
| 场景检测 | 两遍法：Pass1 `ffmpeg -vf "select=gt(scene\,{threshold}),showinfo" -f null NUL` 解析 stderr `pts_time:xxx`；Pass2 逐帧 `-ss {ts} -i {video} -frames:v 1 -q:v 2 -y {out}` |
| 间隔截图 | `ffmpeg -i {video} -vf "fps={1/interval}" -frames:v {max} -q:v 2 -y frame_%04d.png` |
| 引导截图 | 加载 JSON 时间戳 → 逐帧 `-ss {ts} -i {video} -frames:v 1 -q:v 2 -y guided_NNNN_{slug}.png` |
| 输出格式 | `output_dir/frames/keyframes.json` + `*.png`，结构与 Python 脚本完全一致 |
| 参数默认值 | `scene_threshold=0.25`, `min_gap=3`, `max_gap=120`, `max_frames=40`（与 Python 脚本一致） |
| `extract_keyframes_guided` | 改为调用原生实现，不再 `run_python_script`；签名不变 |
| Tauri 命令 `extract_keyframes` | 从 python.rs 迁移到 pipeline.rs，调用原生实现 |
| 结构体迁移 | `KeyframeResult`/`KeyframeData`/`KeyframeInfo` 从 python.rs 迁移到 pipeline.rs |
| 错误处理 | FFmpeg 不存在 → `AppError::MissingDependency`；FFmpeg 执行失败 → `AppError::Other`（含 stderr 摘要）；视频不存在 → `AppError::Other` |
| 日志 | `log::debug!("[keyframes] phase=...")` |

### 6.3 影响范围

| 文件 | 改动类型 |
|------|----------|
| `commands/ai_douyin.rs` | **新增** — AI Douyin tasks API（reqwest） |
| `commands/mod.rs` | **修改** — 增加 `pub mod ai_douyin;` |
| `commands/pipeline.rs` | **修改** — 新增原生关键帧提取 + 迁入 `extract_keyframes` 命令 |
| `commands/python.rs` | **修改** — 移除 `list_ai_douyin_tasks`、`extract_keyframes`、`AiDouyinTaskList`、`KeyframeResult/Data/Info` |
| `commands/tools/handlers/acquire.rs` | **修改** — import 改为 `ai_douyin::list_ai_douyin_tasks` |
| `lib.rs` | **修改** — import 来源调整（`ai_douyin::list_ai_douyin_tasks`、`pipeline::extract_keyframes`） |
| `scripts/list_ai_douyin_tasks.py` | **删除** |
| `scripts/extract_keyframes.py` | **删除** |
| `scripts/README.md` | **修改** — 脚本清单从 6 减到 4 |
| `tauri.conf.json` | **无需修改** — `scripts/*.py` glob 自动适配剩余 4 个脚本 |

### 6.4 回滚策略

1. **Git 回滚**：本次改动均为工作区未提交变更，`git checkout -- <file>` 即可恢复。
2. **分步回滚**：两个脚本迁移互相独立，可单独回滚其中一个。
3. **Python 脚本恢复**：被删除的 `.py` 文件可从 git 历史或上游 `D:/Project/MyClaude/myriad-mind/scripts/` 恢复。
4. **IPC 契约**：Tauri 命令签名保持不变，前端 `api.ts` 无需改动，回滚不涉及前端。

### 6.5 验证清单

- [x] `cargo check` 零错误
- [ ] `list_ai_douyin_tasks` — 需真实 AI Douyin API Key 端到端验证（环境无 key，逻辑单测保证）
- [ ] `extract_keyframes` — 需本地视频文件 + FFmpeg 端到端验证（环境依赖，逻辑对照保证）
- [x] 前端 `api.ts` IPC 契约不变（`listAiDouyinTasks` 参数名一致）
- [x] Agent 工具链路（`QueryAiDouyinHandler` / `ExtractKeyframesHandler`）import 正确

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

## 六、本次实施方案（2026-06-24 实施记录）

> 本节为「设计先行」落地稿：在编码前先确认方案、影响范围与回滚策略。实施目标覆盖迁移顺序的 **第 1（`list_ai_douyin_tasks`）+ 第 2（`extract_keyframes`）** 共 2 个可迁脚本。

### 6.1 目标与范围

| 项 | 内容 |
|----|------|
| 迁移对象 | `scripts/list_ai_douyin_tasks.py`、`scripts/extract_keyframes.py` |
| 落点模块 | 新建 `commands/ai_douyin.rs`（HTTP 直连）、新建 `commands/media.rs`（FFmpeg 直调） |
| 移除项 | 上述 2 个 `.py` 脚本、`python.rs` 内对应的 2 个 `#[tauri::command]` 与类型 |
| 不迁项 | `transcribe_faster_whisper.py`、`download_youtube_subtitles.py`、`download_video_candidates.py`（whisper / yt-dlp 硬依赖，保留） |
| 验收 | `cargo check` 0 错误；既有 `redact_secrets` 单测通过；行为（参数/输出/错误语义）不退化 |

### 6.2 模块职责（新增）

- **`commands/ai_douyin.rs`** — 唯一职责：直连 AI Douyin `/tasks` 接口。
  - 移植 `build_tasks_endpoint`（去尾 `/`，按后缀 `/api/v1` / `/api` / 其它三种拼接规则）。
  - 构造 query（`page`/`pageSize` 必带，`status`/`search` 非空才带）、`X-API-Key` 头、30s 超时。
  - 返回上游 JSON 对象（包成 `AiDouyinTaskList { data: serde_json::Value }`，与原 `#[serde(flatten)]` 等价）。
- **`commands/media.rs`** — FFmpeg 关键帧抽取（与 `pipeline.rs::extract_audio_ffmpeg` 同类「Rust 直调 FFmpeg」操作）。
  - 忠实移植 `extract_keyframes`：`interval` / `scene` / `both` 三模式 + 可选引导时间戳 `guided`。
  - 两遍 scene 检测：Pass1 `select=gt(scene\,N),showinfo` 输出到 null、解析 stderr 的 `pts_time:`；Pass2 按时间点 `-ss` 精准截单帧。
  - 去重（按文件名）+ 按时间戳排序 + `[:max_frames]` 截断。
  - 落盘 `output_dir/frames/keyframes.json`（**含 `trigger` 字段**）。
  - 局部 `resolve_ffmpeg_binary` / `apply_windows_no_window`（与 `pipeline.rs` 同款，PATH + WinGet 兜底）。

### 6.3 接口签名变化（IPC 契约）

迁移后移除仅为「定位 Python 中转」而存在的 `python_path` 参数（属中转层管道，非功能性参数；移除即「Rust 直连」的本意）。**功能性参数全部原样保留**：

```rust
// ai_douyin.rs —— 移除首参 python_path，其余不变
#[tauri::command]
pub async fn list_ai_douyin_tasks(
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
) -> Result<AiDouyinTaskList, AppError>

// media.rs —— 移除首参 python_path，其余不变
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError>
```

调用方同步更新（仅 2 处）：
- 前端 `apps/desktop/src/api.ts::listAiDouyinTasks(pythonPath, apiKey, opts)` → 去掉 `pythonPath`，invoke payload 同步去掉 `pythonPath`。
- Agent 工具 `commands/tools/handlers/acquire.rs::QueryAiDouyinHandler` → 不再传 `ctx.python_path`。
- `extract_keyframes` Tauri 命令前端无调用方（仅注册），签名变更无前端影响。
- `pipeline.rs::extract_keyframes_guided` 改为调用 `media::extract_keyframes_impl`，**签名同步去掉 `python_path`**；`analyze.rs` 调用处去掉 `ctx.python_path`。

> Tauri IPC 反序列化对「前端多传的字段」默认忽略，故即使前端某处残留 `pythonPath` 也不会报错；但本次会一并清理，不留死参。

### 6.4 行为一致性要点（逐项对齐 Python 脚本）

**`list_ai_douyin_tasks`：**
- 端点默认 `https://ai-douyin.top9.cc`（脚本 `DEFAULT_API_BASE`）。
- 三段式后缀拼接（`/api/v1`→`/tasks`；`/api`→`/v1/tasks`；其它→`/api/v1/tasks`）。
- query 顺序与键名（`page`/`pageSize`/`status`/`search`）与脚本一致；空串不带。
- 非 2xx → `AppError::Other("AI Douyin tasks request failed: HTTP {code} {body}")`，**保留状态码与响应体**（比原 Python 仅给 `HTTPError.code` 更利于诊断，且 acquire.rs 上层已统一脱敏）。
- 传输层失败 → `AppError::Http(#[from] reqwest::Error)`。

**`extract_keyframes`：**
- FFmpeg argv 与脚本逐字一致：guided/scene 单帧 `-ss {ts:.3f} -i {video} -frames:v 1 -q:v 2 -y {out}`；interval `-vf fps={1/interval:.6f} -frames:v {max} -q:v 2 -y frame_%04d.png`；scene Pass1 `-vf select=gt(scene\,{th}),showinfo -vsync vfr -f null -y NUL|/dev/null`。
- `frame_%04d.png` 文件名解析回时间戳 `(num-1)*interval`（与脚本一致）。
- `timestamp_label`：`HHhMMmSSs`（≥1h）/ `MMmSSs`（脚本格式）。
- scene 时间戳去重：`min_gap` 内跳过（`last_ts` 初值 `-min_gap-1`）。
- 输出目录结构 `output_dir/frames/`（脚本 `frames_dir = output_dir/"frames"`）不变，`keyframes.json` 落此。
- **`trigger` 字段**：`keyframes.json` 必含（`guided:{reason}`/`interval`/`scene`）——`ai/vision.rs:149` 依赖它；Tauri 命令返回的 `KeyframeInfo` 仍不含 `trigger`（与原 `serde` 反序列化丢弃字段的行为一致）。
- `scene` 模式 `scene_max = max_frames`；`both` 模式 `scene_max = max(max_frames//3, 5)`（脚本逻辑）。
- `scene_threshold`/`min_gap`/`max_gap` 默认 `0.25`/`3`/`120`（脚本 argparse 默认，对齐内部 `extract_keyframes_guided` 实参）。
- 简化项（已评估、低风险）：CLI 时代的 `KF_*` 环境变量回退（`env_or_default`）不再迁移——Rust 直连后唯一功能性调用方 `extract_keyframes_guided` 始终显式传参，`extract_keyframes` Tauri 命令无前端调用方；Tauri 命令路径改用与脚本 argparse 相同的硬编码默认值。该环境变量是 CLI 便利项，桌面 App 运行时不经命令行触发。

### 6.5 注册与资源清单同步

- `commands/mod.rs` 增 `pub mod ai_douyin;` `pub mod media;`。
- `lib.rs` import：`extract_keyframes` 从 `python::` 迁到 `media::`，`list_ai_douyin_tasks` 从 `python::` 迁到 `ai_douyin::`；`generate_handler!` 名称不变。
- `tauri.conf.json` 的 `bundle.resources` 用 glob `../../../scripts/*.py`——删文件后自动收敛，**无需手改**（已确认）。
- `scripts/README.md` 脚本清单 6→4；`docs/架构与结构.md` §八 同步。

### 6.6 风险与兼容性

| 风险 | 评估 | 缓解 |
|------|------|------|
| scene Pass1 stderr 解析差异 | 中：依赖 FFmpeg `showinfo` 输出格式 | 正则 `pts_time:([\d.]+)` 与脚本逐字一致；保留 stderr 全文便于诊断 |
| `keyframes.json` 缺 `trigger` | 高：vision 审查会断 | impl 落盘时显式写 `trigger`，并加单测断言 |
| FFmpeg 未安装 | 低：与原脚本同样依赖 FFmpeg | 复用 `resolve_ffmpeg_binary`，缺失返回 `MissingDependency`（同 `extract_audio_ffmpeg`） |
| IPC 签名去 `python_path` | 低：Tauri 忽略多余字段 | 同步清理 2 处调用方，无残留 |
| 旧日志脱敏 `redact_secrets` | 无：仍服务于剩余 4 个 Python 脚本（`download_video` 等仍可能经 argv 回显 key） | 保留不动 |

### 6.7 回滚策略

纯工作区改动、未提交（CLAUDE.md §五禁止 VCS 写操作）。回滚 = `git checkout -- <改动文件>` + 恢复 2 个 `.py`：
```bash
git checkout -- apps/desktop/src-tauri/src/ apps/desktop/src/api.ts scripts/ docs/
git checkout HEAD -- scripts/list_ai_douyin_tasks.py scripts/extract_keyframes.py
```
本次不碰 `commands/` 模块树既有结构（仅新增 2 个叶子模块，符合本计划既定落点），不改动 `pipeline.rs` 的 4 条管线分流骨架（仅 `extract_keyframes_guided` 改调 Rust impl），不动 `ai/engine.rs` 路由。`download_douyin_video` 超时兜底属关联修复（见 §七）。

### 6.8 实施步骤

1. 写本设计稿（本节）。
2. 新建 `commands/ai_douyin.rs`，迁 `list_ai_douyin_tasks` + `AiDouyinTaskList`；删 `python.rs` 内对应代码；改 `acquire.rs` / `api.ts` / `mod.rs` / `lib.rs`。
3. 新建 `commands/media.rs`，移植 `extract_keyframes` 全逻辑 + 单测；迁 Tauri 命令与类型；`pipeline.rs::extract_keyframes_guided` 改调 Rust impl；`analyze.rs` 去掉 `python_path`；`mod.rs` / `lib.rs`。
4. 删 2 个 `.py`；更新 `scripts/README.md`、`docs/架构与结构.md` §八。
5. `cargo check` + `cargo test`；导出证据到 `exports/task2/`。

---

## 七、关联修复（2026-06-24）：下载子进程总超时兜底

> 迁移验证期间发现保留脚本 `download_video_candidates.py` 在 B 站直链下载阶段永久卡死（实测 17 分钟无响应）。
> 属中转层缺总超时的既有缺陷，与本次迁移无因果关联，但顺手在 Rust 封装层补齐（方案 A，已实施 + 编译通过 + 用户实测验证）。

### 现象与根因

`download_douyin_video` → `run_python_script` 用 `Command.output().await` **无限期等待**子进程；
脚本侧 `urllib.request.urlopen(timeout=30)` 的 timeout 是 **socket 级**（单次 read 30s），慢速 CDN 持续 dribble 时永不超时 → 子进程卡死 → Rust 卡死 → Agent 卡死。

### 修复

1. **`python.rs::run_python_script`**：子进程加 `kill_on_drop(true)`——封装层全局防孤儿；正常完成的脚本无副作用（child 已退出），仅 future 被提前 drop（超时/取消）时才 kill。
2. **`pipeline.rs::download_douyin_video`**：用 `tokio::time::timeout(300s)` 包裹下载。超时 → drop future → `kill_on_drop` 杀子进程 → 返回 `AppError::Other("视频下载超时…")`。
3. **`acquire.rs::DownloadVideoHandler`** 既有逻辑：B 站 AI Douyin 失败自动 fallback yt-dlp，超时亦触发该分支，不再永久卡死。

### 边界（不改项）

- 不动 `download_video_candidates.py`（Python 脚本黑盒，CLAUDE.md §3）。
- 超时值 `300s` 硬编码常量（后续若提 `config.json` 需走配置 schema 变更确认流程）。
- 抖音/小红书无 yt-dlp fallback，超时直接报错（但不再卡死，可重试）。

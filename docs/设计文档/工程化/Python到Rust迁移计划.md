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

## 六、本次实施方案（2026-06-24 · 第1批：2个脚本）

> 实施范围：仅迁移前 2 个可迁脚本（list_ai_douyin_tasks + extract_keyframes），
> install_faster_whisper 延后（低频操作，非关键路径）。

### 6.1 影响范围

| 模块 | 变更类型 | 说明 |
|------|---------|------|
| `commands/python.rs` | **修改** | 移除 `list_ai_douyin_tasks` / `extract_keyframes` 两个 Tauri 命令及类型定义 |
| `commands/ai_douyin.rs` | **新增** | `list_ai_douyin_tasks` 的 Rust `reqwest` 直连实现 |
| `commands/media.rs` | **新增** | `extract_keyframes` 的 Rust `Command::new("ffmpeg")` 直连实现 |
| `commands/mod.rs` | **修改** | 新增 `pub mod ai_douyin; pub mod media;` |
| `lib.rs` | **修改** | 引入新模块的 Tauri 命令，移除旧 python 模块的对应 import |
| `commands/pipeline.rs` | **修改** | `extract_keyframes_guided` 改为调用 `media::extract_keyframes_direct` |
| `tauri.conf.json` | **修改** | 资源清单从 `../../../scripts/*.py` 改为逐个列出保留的 4 个脚本 |
| `scripts/` | **不删** | 保留 Python 文件作为参考实现（git 历史），Rust 不再调用 |
| `apps/desktop/src/api.ts` | **修改** | `listAiDouyinTasks` 不再传 `pythonPath`；新增 `extractKeyframes` 直接调用 |

### 6.2 详细方案

#### 6.2.1 `list_ai_douyin_tasks` → Rust `reqwest`

```
新 Tauri 命令签名（保持不变，仅实现改为 reqwest）:
  list_ai_douyin_tasks(
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
  ) -> Result<serde_json::Value, AppError>

变更点:
  - 不再需要 python_path 参数（Rust 直连 HTTP）
  - api_key 不出现在命令行参数中（安全提升：旧方案 api_key 作为 --api-key argv 传入
    可被进程列表泄漏，新方案仅在 Rust 内存中构造 HTTP Header）
  - 结果直接 serde_json::Value（AI Douyin API 返回完整 JSON）
  - 日志埋点: log::debug!("[douyin] phase=list_tasks page={} page_size={}", ...)
```

#### 6.2.2 `extract_keyframes` → Rust `std::process::Command`

```
新函数（非 Tauri 命令，pipeline 内部调用）:
  extract_keyframes_direct(
    video_path: &Path,
    output_dir: &Path,
    mode: &str,           // "interval" | "scene" | "both"
    interval: u32,        // 秒
    max_frames: u32,
    guided_timestamps: Option<&Path>,
  ) -> Result<KeyframeResult, AppError>

实现要点:
  - ffmpeg 查找：复用已有模式（PATH → winget → 常见路径），与 deps.rs::detect_ffmpeg 一致
  - interval 模式: ffmpeg -i video -vf "fps=1/N" -frames:v M -q:v 2 output/frame_%04d.png
  - scene 模式: 两遍法（同 Python 脚本）— 第1遍 detect pts_time via select+showinfo，第2遍逐帧截图
  - both 模式: interval + scene 合并去重
  - 引导时间点: 读取 JSON 数组，逐时间点 ffmpeg -ss T -i video -frames:v 1 截图
  - 输出 keyframes.json 索引文件
```

### 6.3 回滚策略

- 新模块有 bug 时，可恢复 `python.rs` 中被注释的命令实现重新注册
- Python 脚本保留在 `scripts/` 目录不删除，作为回退参考
- 使用 `cargo check` 验证编译后即可判断回滚必要性

### 6.4 验收标准

- [x] `cargo check` 0 错误 0 警告
- [ ] `list_ai_douyin_tasks` 命令参数与旧版兼容（前端无需改动）
- [ ] `extract_keyframes` 输出 keyframes.json 结构与旧版一致

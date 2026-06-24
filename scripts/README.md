# Python 脚本

> 这些脚本来自上游项目 [大衍决 Claude Code Skill](https://github.com/yuexingqin97/MyClaude/tree/main/myriad-mind/scripts)，以黑盒子子进程方式被 Rust 后端调度。

## 脚本清单

> 原有 6 个脚本中，`extract_keyframes.py`（关键帧截图）与 `list_ai_douyin_tasks.py`（AI Douyin 任务查询）
> 已迁移为 **Rust 直连实现**（`commands/media.rs` 调 FFmpeg、`commands/ai_douyin.rs` 调 reqwest），
> 不再走 Python 中转。详见 [`docs/设计文档/工程化/Python到Rust迁移计划.md`](../docs/设计文档/工程化/Python到Rust迁移计划.md)。
> 本目录仅保留 whisper / yt-dlp 硬依赖的 4 个脚本。

| 脚本 | 用途 | CLI 入口 |
|------|------|----------|
| `transcribe_faster_whisper.py` | 音频转写 (ASR) | `python transcribe_faster_whisper.py <audio_path> --output-dir <dir>` |
| `download_video_candidates.py` | 下载视频（多 URL 候选） | `python download_video_candidates.py --response-json <file> --output <path>` |
| `download_youtube_subtitles.py` | YouTube 字幕下载 | `python download_youtube_subtitles.py <url> --output-dir <dir>` |
| `install_faster_whisper.py` | 安装 faster-whisper venv | `python install_faster_whisper.py [--venv-dir <dir>]` |

### 已迁移为 Rust 直连（不再打包 .py）

| 原脚本 | 迁移落点 | 说明 |
|--------|----------|------|
| `extract_keyframes.py` | `apps/desktop/src-tauri/src/commands/media.rs` | Rust 直调 FFmpeg（interval/scene/guided，参数与脚本逐字一致） |
| `list_ai_douyin_tasks.py` | `apps/desktop/src-tauri/src/commands/ai_douyin.rs` | Rust 直连 reqwest（端点拼接 / X-API-Key / JSON 返回） |

## 约定

- 所有脚本 **stdout 输出 JSON**，**stderr 输出错误信息**
- 成功时 exit code = 0，失败时 exit code = 1
- Rust 后端通过 `std::process::Command` 调用，解析 JSON stdout

## 同步上游

```bash
# 从上游 MyClaude 仓库同步最新脚本
cp D:/Project/MyClaude/myriad-mind/scripts/*.py scripts/
```

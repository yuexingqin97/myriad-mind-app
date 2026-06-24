# Python 脚本

> 这些脚本来自上游项目 [大衍决 Claude Code Skill](https://github.com/yuexingqin97/MyClaude/tree/main/myriad-mind/scripts)，以黑盒子子进程方式被 Rust 后端调度。
>
> **迁移说明**：`list_ai_douyin_tasks.py`（→ `commands/ai_douyin.rs`，reqwest 直连）与 `extract_keyframes.py`（→ `commands/pipeline.rs`，原生 FFmpeg）已于 2026-06-24 迁移为 Rust 实现，脚本已删除。

## 脚本清单

| 脚本 | 用途 | CLI 入口 |
|------|------|----------|
| `transcribe_faster_whisper.py` | 音频转写 (ASR) | `python transcribe_faster_whisper.py <audio_path> --output-dir <dir>` |
| `download_video_candidates.py` | 下载视频（多 URL 候选） | `python download_video_candidates.py --response-json <file> --output <path>` |
| `download_youtube_subtitles.py` | YouTube 字幕下载 | `python download_youtube_subtitles.py <url> --output-dir <dir>` |
| `install_faster_whisper.py` | 安装 faster-whisper venv | `python install_faster_whisper.py [--venv-dir <dir>]` |

## 约定

- 所有脚本 **stdout 输出 JSON**，**stderr 输出错误信息**
- 成功时 exit code = 0，失败时 exit code = 1
- Rust 后端通过 `std::process::Command` 调用，解析 JSON stdout

## 同步上游

```bash
# 从上游 MyClaude 仓库同步最新脚本
cp D:/Project/MyClaude/myriad-mind/scripts/*.py scripts/
```

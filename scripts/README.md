# Python 脚本

> 这些脚本来自上游项目 [大衍决 Claude Code Skill](https://github.com/yuexingqin97/MyClaude/tree/main/myriad-mind/scripts)，以黑盒子子进程方式被 Rust 后端调度。
> 
> 其中 `list_ai_douyin_tasks.py` 与 `extract_keyframes.py` 已迁移为 Rust 直连实现并删除 Python 文件，详见 [`docs/设计文档/工程化/Python到Rust迁移计划.md`](../docs/设计文档/工程化/Python到Rust迁移计划.md)。

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
# 从上游 MyClaude 仓库同步最新脚本（注意：已迁移的脚本不要在 scripts/ 中恢复）
cp D:/Project/MyClaude/myriad-mind/scripts/download_video_candidates.py scripts/
cp D:/Project/MyClaude/myriad-mind/scripts/download_youtube_subtitles.py scripts/
cp D:/Project/MyClaude/myriad-mind/scripts/install_faster_whisper.py scripts/
cp D:/Project/MyClaude/myriad-mind/scripts/transcribe_faster_whisper.py scripts/
```

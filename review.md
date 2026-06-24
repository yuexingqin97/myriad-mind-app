# Token 用量统计 — Python → Rust 迁移（第 1 批）

> 统计日期：2026-06-24  
> 统计方法：基于会话中实际传输的文本量做字符级估算（DeepSeek V4 Pro 会话）

---

## 估算方法

估算公式（保守）：
- 中英文混合文本：~1.5 字符 ≈ 1 token
- 纯代码/JSON：~3 字符 ≈ 1 token
- 英文/标识符为主：~4 字符 ≈ 1 token

---

## 输入（Input）估算

| 来源 | 内容 | 约字符数 | 约 Token |
|------|------|---------|---------|
| 系统指令（instructions + skills + agents） | 内置 | ~60,000 | ~20,000 |
| AGENTS.md + CLAUDE.md（attachment） | 项目约束 | ~8,500 | ~3,000 |
| 用户问题 | 迁移任务 | ~800 | ~300 |
| Python 到 Rust 迁移计划.md | 设计文档 | ~3,500 | ~1,200 |
| 架构与结构.md | 架构文档 | ~12,000 | ~4,000 |
| python.rs 全量 | Rust 源码 | ~15,000 | ~5,000 |
| extract_keyframes.py 全量 | Python 源码 | ~13,000 | ~4,500 |
| list_ai_douyin_tasks.py 全量 | Python 源码 | ~4,500 | ~1,500 |
| pipeline.rs 相关段落 | Rust 源码 | ~3,000 | ~1,000 |
| lib.rs 全量 | Rust 源码 | ~4,000 | ~1,500 |
| api.ts 相关段落 | TS 源码 | ~2,000 | ~700 |
| tauri.conf.json | 配置 | ~800 | ~300 |
| acquire.rs 相关段落 | Rust 源码 | ~1,500 | ~500 |
| Cargo.toml | 配置 | ~600 | ~200 |
| **输入合计** | | **~129,200** | **~43,700** |

---

## 输出（Output）估算

| 来源 | 内容 | 约字符数 | 约 Token |
|------|------|---------|---------|
| ai_douyin.rs（新文件） | Rust 代码 | ~7,500 | ~2,500 |
| media.rs（新文件） | Rust 代码 | ~14,000 | ~4,700 |
| python.rs 修改 | diff | ~1,000 | ~350 |
| pipeline.rs 修改 | diff | ~1,500 | ~500 |
| lib.rs 修改 | diff | ~500 | ~170 |
| mod.rs 修改 | diff | ~50 | ~20 |
| acquire.rs 修改 | diff | ~600 | ~200 |
| tauri.conf.json 修改 | diff | ~200 | ~70 |
| api.ts 修改 | diff | ~300 | ~100 |
| 迁移计划.md 新增 | Markdown | ~3,000 | ~1,000 |
| codegraph-evidence.md | Markdown | ~4,500 | ~1,500 |
| token-stats.md（本文） | Markdown | ~1,500 | ~500 |
| **输出合计** | | **~34,650** | **~11,610** |

---

## 追问轮（Q&A）估算

| 来源 | 方向 | 约字符数 | 约 Token |
|------|------|---------|---------|
| Q1: 前端构建报错诊断 | Input | ~400 | ~150 |
| Q1: 诊断回复 | Output | ~600 | ~200 |
| Q2: 日志分析请求 | Input | ~150 | ~50 |
| Q2: 日志读取 (~200 行) | Input | ~8,000 | ~2,700 |
| Q2: 日志分析回复 | Output | ~3,000 | ~1,000 |
| 产物整理（本轮） | Output | ~3,000 | ~1,000 |
| **追问合计 Input** | | **~8,550** | **~2,900** |
| **追问合计 Output** | | **~6,600** | **~2,200** |

---

## 总计

| 指标 | 约 Token |
|------|---------|
| 主任务 Input | ~43,700 |
| 主任务 Output | ~11,610 |
| 追问轮 Input | ~2,900 |
| 追问轮 Output | ~2,200 |
| **Total** | **~60,410** |
| **现象** | `tsc && vite build` 时 TypeScript 找不到 workspace 依赖包 |
| **根因** | `packages/core/dist/` 和 `packages/ui/dist/` 不存在 — 首次构建需先编译依赖包 |
| **修复** | `pnpm --filter @myriad-mind/core build` → `pnpm --filter @myriad-mind/ui build` → `pnpm run build` |
| **结论** | monorepo 正常构建流程，非迁移 bug |

### Q2: 视频下载失败（端到端测试）

| 项目 | 内容 |
|------|------|
| **现象** | 炼化 B 站视频 `BV1BG41117xB`，最终笔记仅有元数据无实际视频内容 |
| **链路** | `query_ai_douyin` → AI Douyin HTTP 400 captcha → `download_video_candidates.py` 失败 → yt-dlp 412 + cookie 错误 → 降级生成 |
| **分析** | 详见下方完整链路追踪 |
| **结论** | 与本次迁移无关。`download_video_candidates.py` 是未迁移脚本（迁移计划 §4 "⚠️ 半迁"），失败来自 yt-dlp + B 站反爬。`list_ai_douyin_tasks` Rust 迁移工作正常（成功连接、返回正确错误信息、非崩溃） |

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
| yt-dlp B 站 cookies | ❌ 环境问题 | Chrome 锁库、B 站反爬 |

---

## 编译验证

```
cargo check: 0 errors, 33 warnings (均为预先存在的 dead_code/unused 警告)
pnpm run build (前端): 76 modules, built in 590ms ✅
```
                    &url,
                    &InputMode::Youtube,
                    &output,
                )?
            } else {
                // bilibili/douyin/xhs → AI Douyin API（异步）
                let mode = if lower.contains("bilibili") || lower.contains("b23.tv") {
                    InputMode::Bilibili
                } else {
                    InputMode::Douyin
                };
                // 先尝试 AI Douyin；B 站失败回退 yt-dlp（与 pipeline.rs 路由一致）
                match download_douyin_video(
                    &ctx.python_path,
                    &url,
                    &output,
                    &ctx.temp_dir,
                )
                .await
                {
                    Ok(t) => t,
                    Err(e) if matches!(mode, InputMode::Bilibili) => {
                        log::warn!(
                            target: "agent",
                            "[tool:download_video] ai_douyin failed for bilibili, fallback ytdlp: {e}"
                        );
                        download_video_ytdlp(&ctx.python_path, &url, &InputMode::Bilibili, &output)?
                    }
                    Err(e) => return Err(e),
                }
            };

            // 4. VideoFile artifact（视频文件本身不计 token）
            let art = ArtifactRef {
                id: "video.mp4".into(),
                path: output,
                kind: ArtifactKind::VideoFile,
                tokens_estimate: 0,
                summary: title.clone(),
            };

            Ok(ToolOutput::artifact(
                format!("已下载视频：{title}"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// extract_audio — 从视频提取音频 (FFmpeg)
// ------------------------------------------------------------

/// 用 FFmpeg 从视频文件提取 mp3 音频，输出 temp_dir/audio.mp3，返回 AudioFile artifact。
pub struct ExtractAudioHandler;

impl ToolHandler for ExtractAudioHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "extract_audio".into(),
            description: "用 FFmpeg 从本地视频文件提取音频（mp3），输出 temp_dir/audio.mp3。用于后续 ASR 转写。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "video_path": {
                        "type": "string",
                        "description": "视频文件绝对路径"
                    }
                },
                "required": ["video_path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let video_path = require_str(&params, "video_path")?;
            let video = PathBuf::from(&video_path);

            // 2. 提取音频（FFmpeg 同步）
            std::fs::create_dir_all(&ctx.temp_dir).map_err(AppError::Io)?;
            let audio = ctx.temp_dir.join("audio.mp3");
            extract_audio_ffmpeg(&video, &audio)?;

            // 3. AudioFile artifact
            let art = ArtifactRef {
                id: "audio.mp3".into(),
                path: audio,
                kind: ArtifactKind::AudioFile,
                tokens_estimate: 0,
                summary: format!("音频（来自 {}）", video.display()),
            };

            Ok(ToolOutput::artifact("音频提取完成", art))
        })
    }
}

// ------------------------------------------------------------
// transcribe_asr — 音频 ASR 转写 (faster-whisper)
// ------------------------------------------------------------

/// 调用 faster-whisper 转写音频，文本落盘 transcript.txt，返回 Transcript artifact。
pub struct TranscribeAsrHandler;

impl ToolHandler for TranscribeAsrHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "transcribe_asr".into(),
            description: "用 faster-whisper 把音频转写为文本（支持多语言自动检测）。落盘 transcript.txt，摘要含语言/段数。model_size 越大越准越慢（tiny/base/small/medium/large）。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "audio_path": {
                        "type": "string",
                        "description": "音频文件绝对路径"
                    },
                    "model_size": {
                        "type": "string",
                        "description": "whisper 模型大小，默认 small",
                        "default": "small"
                    },
                    "device": {
                        "type": "string",
                        "description": "推理设备 cpu/cuda，默认 cpu",
                        "default": "cpu"
                    }
                },
                "required": ["audio_path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数（model_size / device 可选，默认 small / cpu）
            let audio_path = require_str(&params, "audio_path")?;
            let model_size = opt_str(&params, "model_size").unwrap_or_else(|| "small".into());
            let device = opt_str(&params, "device").unwrap_or_else(|| "cpu".into());

            // 2. 转写（异步；output_dir 用 temp_dir，python 脚本写 text.txt/srt）
            let output_dir = ctx.temp_dir.to_string_lossy().to_string();
            let result = transcribe_audio(
                audio_path,
                output_dir.clone(),
                ctx.python_path.clone(),
                model_size,
                device,
            )
            .await?;

            // 3. 读取 text_path 全文，落盘到 artifacts_dir/transcript.txt
            let text_path = &result.result.text_path;
            let text = std::fs::read_to_string(text_path)
                .map_err(|e| AppError::Other(format!("读取转写文本失败 {}: {e}", text_path)))?;

            ctx.ensure_artifacts_dir()?;
            let art_path = ctx.artifacts_dir.join("transcript.txt");
            std::fs::write(&art_path, &text).map_err(AppError::Io)?;

            // 4. Transcript artifact + 摘要（语言/段数）
            let language = result.result.language.clone();
            let segment_count = result.result.segment_count;
            let art = ArtifactRef {
                id: "transcript.txt".into(),
                path: art_path,
                kind: ArtifactKind::Transcript,
                tokens_estimate: ArtifactRef::estimate_tokens(&text),
                summary: format!("{language} · {segment_count} 段"),
            };

            Ok(ToolOutput::artifact(
                format!("转写完成：{language}，{segment_count} 段"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// download_subtitles — YouTube 字幕下载
// ------------------------------------------------------------

/// 下载 YouTube 字幕。有文本则落盘 subtitle.txt 返回 Subtitle artifact；全无则文本提示。
pub struct DownloadSubtitlesHandler;

impl ToolHandler for DownloadSubtitlesHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "download_subtitles".into(),
            description: "下载 YouTube 视频字幕（自动字幕 + 人工字幕）。有可用字幕则落盘 subtitle.txt 返回 Subtitle artifact；无则返回提示文本。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "YouTube 视频 URL"
                    },
                    "languages": {
                        "type": "string",
                        "description": "可选，优先语言代码逗号分隔，如 \"zh,en\""
                    }
                },
                "required": ["url"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let url = require_str(&params, "url")?;
            let languages = opt_str(&params, "languages");

            // 2. 下载字幕（输出目录用 temp_dir）
            std::fs::create_dir_all(&ctx.temp_dir).map_err(AppError::Io)?;
            let output_dir = ctx.temp_dir.to_string_lossy().to_string();
            let subtitle = download_youtube_subtitles(
                url,
                output_dir,
                ctx.python_path.clone(),
                languages,
            )
            .await?;

            // 3. text_path 可能 None（视频无字幕）→ 返回纯文本提示
            let text_path = match &subtitle.result.text_path {
                Some(p) => p,
                None => return Ok(ToolOutput::text("该视频无可用字幕")),
            };

            // 4. 读全文落盘 artifacts_dir/subtitle.txt
            let text = std::fs::read_to_string(text_path).map_err(|e| {
                AppError::Other(format!("读取字幕文件失败 {}: {e}", text_path))
            })?;
            ctx.ensure_artifacts_dir()?;
            let art_path = ctx.artifacts_dir.join("subtitle.txt");
            std::fs::write(&art_path, &text).map_err(AppError::Io)?;

            let langs = subtitle.result.languages.join(",");
            let art = ArtifactRef {
                id: "subtitle.txt".into(),
                path: art_path,
                kind: ArtifactKind::Subtitle,
                tokens_estimate: ArtifactRef::estimate_tokens(&text),
                summary: format!("字幕语言：{langs}"),
            };

            Ok(ToolOutput::artifact(
                format!("字幕下载完成（语言 {langs}）"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// scan_code_project — 代码项目扫描 → Markdown artifact
// ------------------------------------------------------------

/// 扫描代码项目目录，按优先级读取关键文件并格式化为 Markdown，落盘 code_scan.md。
pub struct ScanCodeProjectHandler;

impl ToolHandler for ScanCodeProjectHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "scan_code_project".into(),
            description: "扫描本地代码项目目录：递归建目录树、按优先级（README/构建配置/入口/核心源码）读取关键文件、推断技术栈，格式化为 Markdown 落盘 code_scan.md。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "项目根目录绝对路径"
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            // 沙箱：限制在可信读取根（input_root/temp/artifacts/note_dir）内
            let root = ctx.resolve_readable(&path)?;

            // 2. 扫描（max_depth=5）+ 格式化
            let scan = scan_code_project(&root, 5)?;
            let tech_stack = scan.tech_stack.join(", ");
            let total_files = scan.total_files;
            let markdown = format_code_project_for_ai(&scan);

            // 3. 落盘 artifacts_dir/code_scan.md
            ctx.ensure_artifacts_dir()?;
            let art_path = ctx.artifacts_dir.join("code_scan.md");
            std::fs::write(&art_path, &markdown).map_err(AppError::Io)?;

            // 4. CodeScan artifact + 摘要（技术栈/文件数）
            let art = ArtifactRef {
                id: "code_scan.md".into(),
                path: art_path,
                kind: ArtifactKind::CodeScan,
                tokens_estimate: ArtifactRef::estimate_tokens(&markdown),
                summary: format!("{tech_stack} · {total_files} 文件"),
            };

            Ok(ToolOutput::artifact(
                format!("代码扫描完成：{tech_stack}，{total_files} 文件"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// read_file — 读小文本文件（< 8000 字符直接回，否则截断预览）
// ------------------------------------------------------------

/// 读取文本文件。小文件（< 8000 字符）直接返回全文；大文件返回前 2000 字符预览 + 提示。
pub struct ReadFileHandler;

impl ToolHandler for ReadFileHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: "读取本地文本文件（.md/.txt/.json/源码等）。小文件直接返回全文；大文件返回前 2000 字符预览并提示用 read_artifact 读取完整内容。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件绝对路径"
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            // 沙箱：限制在可信读取根内（防 prompt injection 读敏感文件）
            let resolved = ctx.resolve_readable(&path)?;

            // 2. 读全文（复用 fs::read_text_file 命令）
            let content = read_text_file(resolved.to_string_lossy().to_string()).await?;
            let char_count = content.chars().count();

            // 3. 小文件直接回；大文件截断前 2000 字符预览
            const SMALL_LIMIT: usize = 8_000;
            const PREVIEW: usize = 2_000;
            let output = if char_count <= SMALL_LIMIT {
                content
            } else {
                let head: String = content.chars().take(PREVIEW).collect();
                format!(
                    "{head}\n\n(共 {char_count} 字符，已截断预览。完整内容用 read_artifact 读取。)"
                )
            };

            Ok(ToolOutput::text(output))
        })
    }
}

// ------------------------------------------------------------
// scan_directory — 目录扫描 → 文本摘要（不落盘）
// ------------------------------------------------------------

/// 扫描目录下可处理文件（视频/音频/文本），格式化目录树为文本摘要返回。
pub struct ScanDirectoryHandler;

impl ToolHandler for ScanDirectoryHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "scan_directory".into(),
            description: "扫描目录下可处理文件（视频/音频/文本，递归最多 2 层），按类型分组返回文本摘要。不落盘 artifact。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录绝对路径"
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            // 沙箱：限制在可信读取根内
            let resolved = ctx.resolve_readable(&path)?;

            // 2. 扫描（复用 fs::scan_directory，内部递归 2 层 + 文件类型分类）
            let result = scan_directory(resolved.to_string_lossy().to_string()).await?;

            // 3. 格式化目录树为文本摘要（目录树通常不大，不落盘 artifact）
            let mut lines = Vec::with_capacity(result.files.len() + 4);
            lines.push(format!("目录：{}", result.path));
            lines.push(format!("共 {} 个可处理文件", result.total_count));
            // 按类型分组统计
            let mut groups: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for f in &result.files {
                *groups.entry(f.file_type.as_str()).or_default() += 1;
            }
            for (kind, n) in &groups {
                lines.push(format!("  - {kind}: {n}"));
            }
            lines.push(String::new());
            for f in &result.files {
                lines.push(format!("[{}] {} ({} B)", f.file_type, f.path, f.size_bytes));
            }
            let summary = lines.join("\n");

            Ok(ToolOutput::text(summary))
        })
    }
}

// ------------------------------------------------------------
// query_ai_douyin — 查询 AI Douyin 任务列表（付费）
// ------------------------------------------------------------

/// 查询 AI Douyin 任务列表。api_key 缺失返回配置错误；
/// 调用失败脱敏（stderr 可能含明文 key）。
pub struct QueryAiDouyinHandler;

impl ToolHandler for QueryAiDouyinHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "query_ai_douyin".into(),
            description: "查询 AI Douyin 平台已提交的视频解析任务列表。支持按 search/status 过滤。需要配置 ai_douyin_api_key（设置 → API 密钥）。调用失败时不回显 stderr（避免泄露密钥）。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "search": {
                        "type": "string",
                        "description": "可选，按关键词过滤任务标题/描述"
                    },
                    "status": {
                        "type": "string",
                        "description": "可选，按状态过滤（如 success/processing/failed）"
                    }
                },
                "required": []
            }),
            phase: Phase::Acquire,
            cost: Cost::Paid,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 取 api_key（缺失即配置错误，不脱敏，因为不涉及上游响应）
            let api_key = read_config_value("ai_douyin_api_key").ok_or_else(|| {
                AppError::Other("未配置 ai_douyin_api_key（请在设置 → API 密钥中配置）".into())
            })?;

            // 2. 解析可选过滤参数
            let search = opt_str(&params, "search");
            let status = opt_str(&params, "status");

            // 3. 查询（Rust reqwest 直连，api_key 仅在内存中，无 argv 泄漏风险）
            let list = list_ai_douyin_tasks(
                api_key,
                None, // api_base 用默认
                None, // page
                None, // page_size
                status,
                search,
            )
            .await
            .map_err(|e| {
                log::warn!(
                    target: "agent",
                    "[tool:query_ai_douyin] failed: {e}"
                );
                AppError::Other("AI Douyin 查询失败（详情见日志文件）".into())
            })?;

            // 4. 成功：data 是上游 API 返回的 JSON，格式化为文本摘要
            let data = &list;
            let summary = match data {
                serde_json::Value::Array(items) => {
                    let n = items.len();
                    let mut lines = Vec::with_capacity(n + 1);
                    lines.push(format!("AI Douyin 任务列表：共 {n} 条"));
                    for (i, item) in items.iter().take(50).enumerate() {
                        let title = item
                            .get("title")
                            .or_else(|| item.get("desc"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("(无标题)");
                        let st = item
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        lines.push(format!("{}. [{st}] {title}", i + 1));
                    }
                    if n > 50 {
                        lines.push(format!("…（其余 {} 条已省略）", n - 50));
                    }
                    lines.join("\n")
                }
                other => {
                    // 非 array（对象 / 单值）兜底：序列化为紧凑 JSON
                    format!("AI Douyin 响应：{}", other)
                }
            };

            Ok(ToolOutput::text(summary))
        })
    }
}
    output: &std::path::Path,
    temp_dir: &std::path::Path,
) -> Result<String, AppError> {
    // 读取 Key（resolve_via_ai_douyin 内部也会读，这里需要再读一次传给脚本）
    let douyin_key = crate::commands::config::read_config_value("ai_douyin_api_key").unwrap_or_default();

    let json_path = resolve_via_ai_douyin(video_url, temp_dir).await?;

    let output_str = output.to_string_lossy().to_string();
    let json_str = json_path.to_string_lossy().to_string();
    log::debug!(target: "agent","[douyin] download_video_candidates.py: {json_str} → {output_str}");

    let mut args: Vec<String> = vec![
        "--response-json".into(), json_str,
        "--output".into(), output_str,
    ];
    if !douyin_key.is_empty() {
        args.push("--api-key".into());
        args.push(douyin_key);
    }

    let result = crate::commands::python::run_python_script(
        python_path,
        "download_video_candidates.py",
        &args,
    )
    .await?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "download_video_candidates.py".into(),
            stderr: result.stderr,
        });
    }

    // Try to extract title from the API response (re-read from file)
    let title = if let Ok(json_str) = std::fs::read_to_string(&json_path) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_str) {
            data.get("title")
                .or_else(|| data.get("desc"))
                .and_then(|v| v.as_str())
                .unwrap_or("抖音视频")
                .to_string()
        } else {
            "抖音视频".to_string()
        }
    } else {
        "抖音视频".to_string()
    };

    log::debug!(target: "agent","[douyin] download complete: {title}");
    Ok(title)
}

/// Download video using yt-dlp
pub(crate) fn download_video_ytdlp(
    python_path: &str,
    url: &str,
    _mode: &InputMode,
    output: &std::path::Path,
) -> Result<String, AppError> {
    let output_str = output.to_string_lossy();
    log::debug!(target: "agent","[download] yt-dlp: {url}");
    let is_bilibili = matches!(_mode, InputMode::Bilibili);
    let (program, prefix_args) = ytdlp_command(python_path);
    let mut cmd = std::process::Command::new(&program);
    apply_windows_no_window(&mut cmd);
    cmd.args(&prefix_args);
    cmd.args([
        "-o",
        &output_str,
        "--print",
        "%(title)s",
        "--no-playlist",
        "--remote-components", "ejs:github",
        "--extractor-args", "youtube:player_client=android,web",
        "--sleep-requests", "3", "--sleep-interval", "5",
        "-f", "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best",
    ]);
    //  B 站 412 风控：缺 Referer 头会被拒。加上的话大部分视频不需要 Cookie 就能下。
    if is_bilibili {
        cmd.arg("--add-header").arg("Referer:https://www.bilibili.com");
    }
    cmd.arg(url)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");

    let mut r = cmd
        .output()
        .map_err(|e| AppError::Config(format!("yt-dlp 未安装：{e}")))?;

    //  裸跑被 412 挡了？试 cookies-from-browser（B 站受限内容需要登录态）。
    if !r.status.success() && is_bilibili {
        let stderr_first = String::from_utf8_lossy(&r.stderr);
        if stderr_first.contains("412") {
            log::warn!("[download] yt-dlp 裸跑 412，降级 --cookies-from-browser edge: {url}");
            let mut cmd2 = std::process::Command::new(&program);
            apply_windows_no_window(&mut cmd2);
            cmd2.args(&prefix_args);
            cmd2.args([
                "-o", &output_str,
                "--print", "%(title)s",
                "--no-playlist",
                "--remote-components", "ejs:github",
                "-f", "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best",
                "--add-header", "Referer:https://www.bilibili.com",
                "--cookies-from-browser", "edge",
                url,
            ])
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");
            r = cmd2.output().map_err(|e| AppError::Config(format!("yt-dlp 未安装：{e}")))?;
        }
    }

    if !r.status.success() {
        let stderr = String::from_utf8_lossy(&r.stderr);
        log::error!("[download] yt-dlp failed: {stderr}");
        return Err(AppError::Config(format!("yt-dlp 下载失败：{stderr}")));
    }
    let stdout = String::from_utf8_lossy(&r.stdout);
    let title = stdout
        .lines()
        .last()
        .unwrap_or("未知标题")
        .trim()
        .to_string();
    log::debug!(target: "agent","[download] title: {title}");

    // 校验文件是否真的存在（yt-dlp 可能合并失败但仍返回 0）
    if !media_file_ready(output) {
        log::error!(
            "[download] yt-dlp 返回成功但文件不存在：{} ({} bytes?)",
            output.display(),
            output.metadata().map(|m| m.len()).unwrap_or(0)
        );
        return Err(AppError::Config(
            "yt-dlp 返回成功但视频文件未生成，可能是音视频合并失败。请确认 FFmpeg 已安装并加入 PATH。".into(),
        ));
    }
    Ok(title)
}

pub(crate) fn media_file_ready(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub(crate) fn extract_audio_ffmpeg(video: &PathBuf, audio: &PathBuf) -> Result<(), AppError> {
    let ffmpeg = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;
    let status = std::process::Command::new(ffmpeg)
        .args([
            "-i",
            &video.to_string_lossy(),
            "-q:a",
            "0",
            "-map",
            "a",
            "-y",
            &audio.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    if !status.success() {
        return Err(AppError::Other("FFmpeg 音频提取失败".into()));
    }
    Ok(())
}

/// 调用 Rust FFmpeg 直连（替代原 extract_keyframes.py）
/// 只使用 scene 模式：字幕引导时间点 + 场景变化检测，不使用固定间隔
pub(crate) fn extract_keyframes_guided(
    _python_path: &str,
    video: &PathBuf,
    output_dir: &PathBuf,
    guided_timestamps: Option<&std::path::Path>,
) -> Result<PathBuf, AppError> {
    if !video.exists() {
        return Err(AppError::Other("视频文件不存在".into()));
    }

    if let Some(ts_path) = guided_timestamps {
        if ts_path.exists() {
            log::debug!(target: "agent",
                "[pipeline] keyframes extraction with guided timestamps: {}",
                ts_path.display()
            );
        }
    }

    crate::commands::media::extract_keyframes_direct(
        video,
        output_dir,
        "scene",   // 只使用 scene 模式
        30,        // interval (scene 模式不用)
        40,        // max_frames
        guided_timestamps,
    )?;

    Ok(output_dir.clone())
}

pub(crate) fn generate_temp_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}

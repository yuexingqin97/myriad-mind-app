# 架构设计详解

> 大衍决 App 的完整技术架构设计文档

---

## 1. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                    共享逻辑层 (packages/core)              │
│  配置管理 · Pipeline 定义 · Prompt 模板 · 笔记解析        │
│  修为面板计算 · 搜索 · 灵力预估 · 知识图谱                │
├───────────────────────┬─────────────────────────────────┤
│    桌面端 (apps/desktop) │     移动端 (apps/mobile)        │
│   Tauri 2.x (Rust+React)│   React Native (Expo)          │
│   完整功能 · 本地工具链   │   轻量功能 · 纯 TypeScript       │
└───────────────────────┴─────────────────────────────────┘
```

### 设计原则

1. **核心逻辑只写一次** — 配置/Prompt/搜索/面板/解析 → `packages/core/`
2. **平台差异隔离** — 进程调用/文件系统/HTTP 层 → 各自平台实现
3. **Python 脚本不动** — 现有 6 个脚本作为黑盒子子进程调用
4. **本地优先** — 无服务器、无云存储、数据完全用户控制

---

## 2. 桌面端架构（Tauri 2.x）

### 2.1 Rust 后端模块

```
src-tauri/src/
├── main.rs              # 入口，注册插件和命令
├── lib.rs               # Tauri Builder 配置
├── error.rs             # 统一错误类型（thiserror）
├── commands/
│   ├── mod.rs
│   ├── python.rs        # 调度 Python 脚本，解析 JSON stdout
│   ├── deps.rs          # 系统依赖检测（Python/FFmpeg/yt-dlp/CUDA）
│   ├── claude.rs        # Claude API 客户端（reqwest SSE 流式）
│   ├── config.rs        # 配置读写 + OS 密钥链集成
│   ├── fs.rs            # 文件系统操作（读笔记/写笔记/扫描目录）
│   └── pipeline.rs      # 多步骤管线编排（进度事件推送）
├── python/
│   ├── mod.rs
│   ├── venv.rs          # 创建/管理 faster-whisper venv
│   └── detect.rs        # 查找系统 Python 安装
└── deps/
    ├── mod.rs
    ├── ffmpeg.rs         # FFmpeg 检测（已知路径 + PATH）
    ├── ytdlp.rs          # yt-dlp 检测
    └── gpu.rs            # CUDA/GPU 检测
```

### 2.2 管线执行流程

```
pipeline.rs::execute(input, config) {

    // 步骤 0：识别输入类型（URL/文件/目录/代码项目）
    let mode = classify_input(&input);
    emit_event("mode_detected", mode);

    // 步骤 0.6：依赖检查
    let deps = check_dependencies(&mode).await?;
    emit_event("deps_checked", deps);

    // 步骤 0.7：灵力预估
    let estimate = estimate_cost(&mode, &config);
    emit_event("estimation", estimate);
    // ↑ 等待用户在前端确认

    // 步骤 1-4：Python 脚本管线
    for step in pipeline_steps(&mode) {
        emit_event("step_started", step.name);
        let result = run_python_script(&step.script, &step.args).await?;
        emit_event("step_completed", step.name, result);
    }

    // 步骤 5-7：Claude API 生成笔记
    let note = generate_note_streaming(&context, &config).await?;
    // ↑ 每个 token 通过 SSE 事件推送到前端

    // 步骤 8：清理临时文件
    if config.output.cleanup_temp {
        cleanup_temp_files(&mode.temp_dir)?;
    }

    // 步骤 9：后处理
    if config.post_process.auto_update_panel {
        update_dashboard(&config.output.note_dir).await?;
    }

    emit_event("completed", note);
}
```

### 2.3 Python 脚本调度

每个脚本封装为类型化函数：

```rust
// commands/python.rs

pub struct TranscriptionResult {
    pub model_size: String,
    pub device: String,
    pub compute_type: String,
    pub language: String,
    pub language_probability: f64,
    pub segment_count: usize,
    pub srt_path: PathBuf,
    pub text_path: PathBuf,
}

pub async fn transcribe_audio(
    audio_path: &Path,
    output_dir: &Path,
    python_path: &Path,
    config: &FwConfig,
) -> Result<TranscriptionResult> {
    let output = Command::new(python_path)
        .arg(scripts_dir().join("transcribe_faster_whisper.py"))
        .arg(audio_path)
        .arg("--output-dir").arg(output_dir)
        .arg("--model-size").arg(&config.model_size)
        .arg("--device").arg(&config.device)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        return Err(AppError::PythonScript {
            script: "transcribe_faster_whisper",
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(serde_json::from_slice(&output.stdout)?)
}
```

所有脚本遵循相同的模式：`Command::new → .output() → 检查 exit code → 解析 JSON stdout`。

### 2.4 依赖检测

```
App 启动
  ├── detect_python()  → 扫描 PATH 找 python3/python
  │   └── 版本 >= 3.9? → 返回路径 + 版本
  │
  ├── detect_ffmpeg()  → PATH + 已知安装位置
  │   ├── Windows: %LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*
  │   ├── macOS: /usr/local/bin/ffmpeg, /opt/homebrew/bin/ffmpeg
  │   └── Linux: /usr/bin/ffmpeg
  │
  ├── detect_ytdlp()   → PATH + pip list
  │
  ├── detect_whisper_venv() → 检查 ~/.cache/myriad-mind/faster-whisper-venv/
  │   └── 不存在? → 运行 install_faster_whisper.py（显示进度）
  │
  └── detect_gpu()     → nvidia-smi / system_profiler
      └── 有 CUDA? → 自动设 FW_DEVICE=cuda
```

每个检查返回 `CheckResult { name, found, path, version, suggestion }`，前端汇总显示。

### 2.5 Claude API 集成

```rust
// commands/claude.rs

pub async fn stream_note_generation(
    messages: Vec<Message>,
    system_prompt: String,
    api_key: String,
    app_handle: AppHandle,
) -> Result<String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 8192,
        "messages": messages,
        "system": system_prompt,
        "stream": true,
    });

    let mut stream = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?
        .bytes_stream();

    let mut full_text = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        // 解析 SSE: "data: {...}"
        if let Some(text_delta) = parse_sse_delta(&chunk) {
            full_text.push_str(&text_delta);
            // 推送增量到前端实时显示
            app_handle.emit("claude-stream-delta", text_delta)?;
        }
    }

    Ok(full_text)
}
```

---

## 3. 移动端架构（React Native Expo）

### 3.1 功能范围

移动端不跑任何本地重型工具，主打"消费已生成的笔记" + "轻量内容处理"：

| 功能 | 实现方式 |
|------|----------|
| 笔记浏览 | WebView 渲染 Markdown + Mermaid |
| 文章 URL 处理 | fetch HTML → 提取文本 → Claude API → 生成笔记 → 写入文件 |
| 修为面板 | 纯计算（`packages/core/dashboard/`） |
| 搜索 | 全文搜索 .md 文件 |
| 配置 | 同桌面端 Schema 校验 |
| 截图查看 | 从 `assets/` 目录加载本地图片 |

### 3.2 文章处理流程（移动端）

```
用户粘贴文章 URL
  │
  ├── React Native fetch(url)
  ├── 提取标题 + 正文（轻量 HTML→text）
  ├── packages/core/prompts/article-summarize.ts 构建 Prompt
  ├── fetch("https://api.anthropic.com/v1/messages", {...})
  │   └── SSE ReadableStream → 逐字显示
  ├── 组装 Markdown 笔记
  └── expo-file-system 写入 .md 到本地
```

### 3.3 Markdown 渲染（移动端）

```
react-native-webview
  └── 加载本地 HTML 页面
      ├── <script src="mermaid.min.js">
      ├── 注入 Markdown 内容 → marked.js 渲染
      └── mermaid.run() 渲染图表为 SVG
```

---

## 4. 共享代码设计（packages/core/）

### 4.1 配置 Schema (Zod)

```typescript
// packages/core/src/config/schema.ts

export const ConfigSchema = z.object({
  version: z.literal(1),
  asr: z.object({
    backend: z.enum(["faster-whisper", "volcengine"]),
    faster_whisper: z.object({
      model_size: z.enum(["tiny", "base", "small", "medium", "large-v3"]),
      device: z.enum(["auto", "cpu", "cuda"]),
      compute_type: z.string().optional(),
    }).optional(),
    volcengine: z.object({
      token_keyring_id: z.string(),  // 指向 OS 密钥链
      appid: z.string(),
    }).optional(),
  }),
  video: z.object({
    provider: z.enum(["ai-douyin", "tikhub"]),
  }),
  features: z.object({
    keyframes: z.boolean(),
    mermaid: z.boolean(),
    resources: z.boolean(),
    comments: z.boolean(),
    reading_info: z.boolean(),
    estimation: z.boolean(),
  }),
  keyframes: z.object({
    interval: z.number().min(5).max(300).default(30),
    max_frames: z.number().min(1).max(200).default(50),
    mode: z.enum(["interval", "scene", "both"]).default("interval"),
  }),
  output: z.object({
    note_dir: z.string().default(""),
    cleanup_temp: z.boolean().default(true),
    note_metadata: z.boolean().default(true),
    debug_metadata: z.boolean().default(false),
  }),
  post_process: z.object({
    auto_update_panel: z.boolean().default(true),
    auto_suggest_next: z.boolean().default(true),
  }),
});
```

### 4.2 Prompt 模板

所有 Claude 对话 prompt 从 `packages/core/src/prompts/` 导出为类型化函数：

```typescript
// packages/core/src/prompts/index.ts
export { buildSummarizePrompt } from './summarize';
export { buildTranslatePrompt } from './translate';
export { buildNoteGenerationPrompt } from './note-gen';
export { buildCodeAnalysisPrompt } from './code-analysis';
export { buildComparePrompt } from './compare';
```

每个 prompt 函数的参数和返回值完全复用现有 SKILL.md 中定义的 prompt 结构。

### 4.3 修为面板计算

```typescript
// packages/core/src/dashboard/calculator.ts

export function calculateCultivation(noteStats: NoteStats): CultivationLevel {
  const points =
    noteStats.totalNotes * 10 +
    noteStats.intermediateNotes * 5 +
    noteStats.advancedNotes * 10 +
    noteStats.techStacks * 8 +
    noteStats.totalHours * 2;

  // 映射到 7 个境界
  if (points < 50)  return { level: "炼气期", points, nextLevel: 50 };
  if (points < 120) return { level: "筑基期", points, nextLevel: 120 };
  if (points < 250) return { level: "金丹期", points, nextLevel: 250 };
  if (points < 500) return { level: "元婴期", points, nextLevel: 500 };
  if (points < 1000) return { level: "化神期", points, nextLevel: 1000 };
  if (points < 2000) return { level: "大乘期", points, nextLevel: 2000 };
  return { level: "渡劫飞升", points, nextLevel: null };
}

export function checkAchievements(noteStats: NoteStats): Achievement[] {
  // 6 项成就自动判定
  // 初入道途 / 博览群书 / 专精一道 / 融会贯通 / 持之以恒 / 神识外放
}
```

---

## 5. 数据流

### 5.1 桌面端完整数据流

```
用户输入
  │
  ▼
[模式识别] ──→ 8 种模式之一
  │
  ▼
[灵力预估] ──→ Token 预估 → 用户确认
  │
  ▼
[视频解析] ──→ AI Douyin/TikHub API ──→ download_url
  │
  ▼
[视频下载] ──→ download_video_candidates.py ──→ video.mp4
  │
  ▼
[音频提取] ──→ ffmpeg ──→ audio.mp3
  │
  ▼
[ASR 转写] ──→ transcribe_faster_whisper.py ──→ subtitle.srt + text.txt
  │
  ▼
[关键帧]   ──→ extract_keyframes.py ──→ frames/*.png + keyframes.json
  │
  ▼
[语言检测] ──→ 中文 ≥ 30%? → 跳过 / 否则 → 翻译
  │
  ▼
[笔记生成] ──→ Claude API (流式) ──→ learning_notes.md
  │           ├── 摘要 + 核心概念
  │           ├── 详细笔记（含截图 + Mermaid）
  │           ├── 术语表 + 扩展资源
  │           ├── 评论区精华
  │           ├── 知识关系图
  │           └── 元信息
  │
  ▼
[清理]     ──→ 删除 /tmp/video_analysis/{VIDEO_ID}/
  │
  ▼
[后处理]   ──→ 更新修为面板 + 学习建议
```

### 5.2 移动端文章数据流

```
用户粘贴文章 URL
  │
  ▼
[抓取]     ──→ fetch(url) ──→ HTML 文本
  │
  ▼
[提取]     ──→ 标题 + 正文
  │
  ▼
[生成]     ──→ Claude API (流式) ──→ article_note.md
  │
  ▼
[保存]     ──→ expo-file-system ──→ {NOTE_OUTPUT_DIR}/note.md
```

---

## 6. 构建与分发

### 6.1 桌面端

| 平台 | 产物 | 工具 |
|------|------|------|
| Windows | `.msi` 安装包 / `.exe` 便携版 | Tauri bundler |
| macOS | `.dmg` 磁盘映像 / `.app` 包 | Tauri bundler |
| 自动更新 | GitHub Releases | tauri-plugin-updater |

### 6.2 移动端

| 平台 | 产物 | 工具 |
|------|------|------|
| Android | `.aab` (Play Store) / `.apk` (直接下载) | EAS Build |
| iOS | `.ipa` (App Store / TestFlight) | EAS Build |
| 鸿蒙 | PWA 部署到 GitHub Pages / 后续 ArkTS 壳 | — |

### 6.3 CI/CD

```
GitHub Actions:
  push main     → Lint + Test + Build preview
  release tag   → Build all platforms → GitHub Releases
```

---

## 7. 关键依赖版本

| 依赖 | 版本 | 用途 |
|------|------|------|
| Rust | 1.93+ | 桌面后端 |
| Node.js | 24+ | 前端工具链 |
| TypeScript | 5.9+ | 类型安全 |
| React | 19+ | UI |
| Tauri | 2.x | 桌面框架 |
| Expo SDK | 54+ | 移动框架 |
| Python | 3.9+ (仅桌面) | 运行脚本 |
| FFmpeg | 8.x (仅桌面) | 音视频处理 |

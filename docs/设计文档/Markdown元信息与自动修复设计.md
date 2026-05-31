# Markdown 元信息与自动修复设计

> 参考来源：`D:\Project\MyClaude\myriad-mind` 验证版逻辑。  
> 设计目标：一篇 Markdown 就是一篇完整笔记实体。机器可读元信息放在文件末尾的“大衍决元信息块”中；如果用户删除了元信息，或导入旧笔记没有元信息，App 可以先规则补全，再调用 DeepSeek 生成/修复元信息。

---

## 一、设计结论

可以这样设计，而且很适合大衍决：

```text
Markdown 正文
  ├── 正文：用户可读学习笔记
  ├── 更新记录：版本增量历史
  ├── 问答记录：追问模式追加记录
  ├── 调试信息：仅设置开启时输出
  └── 大衍决元信息：机器可读元信息，固定文件末尾
```

不再额外依赖 `metadata.json`。

好处：

- 用户复制、移动、同步单个 `.md` 文件时不会丢元信息。
- 修为面板、搜索、追问模式都可以只扫描 Markdown。
- 用户删除元信息后也不致命，可以自动修复。
- 旧 Skill 生成的无元信息块笔记也能导入。

---

## 二、验证版可迁移逻辑

从 `D:\Project\MyClaude\myriad-mind` 里可以迁移这些经验：

| 验证版逻辑 | App 版迁移方式 |
|------------|----------------|
| `NOTE_METADATA` 控制文末生成元信息 | App 固定在文件末尾输出机器可读“大衍决元信息块” |
| `DEBUG_METADATA=false` 默认不输出调试信息 | App 设置项 `debug_metadata` 关闭时不写可见调试信息 |
| `DEBUG_METADATA=true` 时输出流水线耗时、工具调用、决策链路 | App 在文末追加 `## 调试信息`，仅面向开发/排错 |
| 笔记里有“原始资源”字段 | 元信息块中保留 `sources[]` |
| 问答模式读取文档元信息中的原始资源 | 追问模式先读元信息块的 `sources[]`，缺失时自动修复 |
| 修为面板提取标题、核心主题、难度、前置依赖、技术栈 | 这些字段直接进入元信息块 |
| 问答记录追加到文末 | 保留 `## 问答记录` 固定章节 |
| 面板版本号递增 | 笔记自身使用 `current_version` 和 `## 更新记录` |

---

## 三、大衍决元信息块结构

不要使用传统 YAML front matter，因为 front matter 按约定应位于文件开头，而这里明确要求机器可读元信息放在文件末尾。

App 使用固定起止标记：

````markdown
<!-- MYRIAD_MIND_METADATA_START -->
```yaml
...
```
<!-- MYRIAD_MIND_METADATA_END -->
````

读取时只解析最后一个完整的 `MYRIAD_MIND_METADATA` 块。这样即使正文中出现示例 YAML，也不会误判。

### 3.1 P0 必需字段

```yaml
schema: myriad-mind-note/v1
id: note_9f3a2c1b
title: Rust 异步编程
category: Rust
slug: rust-async
created_at: 2026-05-31T21:30:12+08:00
updated_at: 2026-05-31T22:45:08+08:00
current_version: 2
app:
  name: myriad-mind-app
  version: 0.1.0-alpha.1
  build: 20260531.1
  platform: windows
pipeline:
  schema_version: 1
  mode: note_generation
  intensity: standard
sources:
  - type: bilibili
    raw: https://www.bilibili.com/video/BVxxx
    canonical: bilibili:BVxxx:p1
    fingerprint: sha256:abc123
    title: Rust async 入门
    added_at: 2026-05-31T21:30:12+08:00
ai:
  provider: deepseek
  model: deepseek-v4-pro
  role: primary
  generated_at: 2026-05-31T22:45:08+08:00
  api_style: openai_compatible
  prompt_preset: note_generation/v1
  prompt_hooks:
    note_generation: true
```

P0 字段说明：

| 字段 | 用途 | 可否自动修复 |
|------|------|--------------|
| `schema` | 判断元信息版本 | 可以 |
| `id` | 笔记稳定 ID | 可以 |
| `title` | 展示、文件名、搜索 | 可以 |
| `category` | 输出目录分类 | 可以 |
| `slug` | 目录 / 文件安全名 | 可以 |
| `created_at` | 首次创建时间 | 可用文件创建时间兜底 |
| `updated_at` | 最近更新时间 | 可以 |
| `current_version` | 增量更新版本号 | 可以从更新记录推断 |
| `app` | 生成 / 更新该笔记的 App 版本 | 可以 |
| `pipeline` | 本次生成所属处理逻辑版本 | 可以 |
| `sources[]` | 输入来源和去重 | 部分可以 |
| `ai` | 生成模型、Provider、Prompt 记录 | 可以 |

### 3.2 P1 增强字段

```yaml
summary: 一句话摘要
topics:
  - Rust
  - async
  - Future
tags:
  - Rust
  - 异步编程
  - Future
difficulty: intermediate
reliability: reference
estimated_reading_minutes: 18
prerequisites:
  - Rust 所有权
  - trait 基础
content_type: video_note
language: zh-CN
features:
  mermaid: true
  keyframes: true
  comments: false
  resources: true
assets:
  transcript: assets/transcript.srt
  keyframes_dir: assets/keyframes
```

这些字段主要服务：

- 修为面板。
- 搜索筛选。
- 后续学习路线推荐。
- 追问模式补充上下文。

---

## 四、Markdown 正文固定章节

推荐结构：

````markdown
# Rust 异步编程

> 推荐阅读时长：18 分钟 | 难度：进阶 | 可靠性：参考
> 标签：#Rust #异步编程 #Future

## AI 摘要

## 核心要点

## 详细笔记

## Mermaid 图表

## 术语表

## 扩展资源

---

## 更新记录

### v1 · 2026-05-31 21:30 · 初次炼化

**来源：** https://www.bilibili.com/video/BVxxx

**本次内容：**
- 初次生成结构化笔记。

### v2 · 2026-05-31 22:45 · 增量更新

**来源：** https://youtube.com/...

**本次新增：**
- 补充 Future 状态机解释。

---

## 问答记录

### 2026-05-31 23:00 — async 和线程的区别

> **问：** async 和线程到底有什么区别？
>
> **答：** ...
>
> 📍 参考章节：详细笔记 / Future 与状态机

---

## 大衍决元信息

> 以下为应用读取用元信息。手动编辑可能影响去重、追问和修为统计。

<!-- MYRIAD_MIND_METADATA_START -->
```yaml
schema: myriad-mind-note/v1
id: note_9f3a2c1b
title: Rust 异步编程
category: Rust
current_version: 2
app:
  name: myriad-mind-app
  version: 0.1.0-alpha.1
  build: 20260531.1
pipeline:
  schema_version: 1
  mode: note_generation
sources:
  - type: bilibili
    raw: https://www.bilibili.com/video/BVxxx
    fingerprint: sha256:abc123
ai:
  provider: deepseek
  model: deepseek-v4-pro
  role: primary
  generated_at: 2026-05-31T22:45:08+08:00
  api_style: openai_compatible
  prompt_preset: note_generation/v1
```
<!-- MYRIAD_MIND_METADATA_END -->
````

规则：

- `## 更新记录` 和 `## 问答记录` 是固定章节名。
- App 写入时只追加这两个章节，不随意改用户正文。
- 如果用户删除章节，下次增量更新或追问时自动重建。
- `## 调试信息` 只有设置开启时才输出；设置关闭时，新笔记不包含该章节，旧笔记中的调试信息默认保留但不继续追加。
- `## 大衍决元信息` 固定放在文件最后；更新、追问、调试信息都必须插入到它之前。

---

## 五、可见元信息与调试信息

### 5.1 基础元信息

App 版把“机器必须读取”的元信息固定放在文件末尾的 `## 大衍决元信息` 章节，而不是文件开头。

这部分不受用户设置里的“显示调试元信息”影响：

- `schema`
- `id`
- `title`
- `category`
- `sources[]`
- `current_version`
- `app`
- `pipeline`
- `ai`
- `created_at`
- `updated_at`

原因：这些字段是去重、追问、增量更新、修为面板的基础。如果允许关闭，后续功能会不稳定。

注意：`## 大衍决元信息` 不受 `debug_metadata` 开关影响，始终输出。`debug_metadata` 只控制 `## 调试信息`。

### 5.2 版本与模型记录

元信息块必须记录两类版本：

| 字段 | 含义 | 示例 |
|------|------|------|
| `schema` | 元信息结构版本 | `myriad-mind-note/v1` |
| `app.version` | 生成或最近更新该笔记的 App 版本 | `0.1.0-alpha.1` |
| `app.build` | 构建号，便于排查同版本不同构建 | `20260531.1` |
| `pipeline.schema_version` | 处理管线逻辑版本 | `1` |
| `pipeline.mode` | 本次处理模式 | `note_generation` / `note_qa` / `metadata_repair` |
| `pipeline.intensity` | 炼化强度 | `quick` / `standard` / `deep` |
| `ai.provider` | AI Provider | `deepseek` |
| `ai.model` | 实际调用模型 | `deepseek-v4-pro` |
| `ai.role` | 模型角色 | `primary` / `fast` / `fallback` |
| `ai.api_style` | API 协议风格 | `openai_compatible` |
| `ai.prompt_preset` | Prompt 模板版本 | `note_generation/v1` |

这些字段用于：

- 排查“这篇笔记是哪个 App 版本生成的”。
- 对比不同模型生成质量。
- 未来升级 Prompt / Pipeline 时做兼容迁移。
- 修为面板统计模型使用分布。
- Debug 开关关闭时仍保留必要排错信息。

增量更新时：

- `app.version` 更新为当前 App 版本。
- `app.build` 更新为当前构建号。
- `pipeline.mode` 更新为本次操作模式。
- `pipeline.schema_version` 更新为当前管线版本。
- `ai.model` 更新为本次实际使用模型。
- 旧模型信息不在顶层累积；历史模型变化写入 `## 更新记录`，或在 `debug_metadata=true` 时写入 `## 调试信息`。

### 5.3 用户可见的生成摘要

可以在正文顶部保留一行用户可读摘要：

```markdown
> 推荐阅读时长：18 分钟 | 难度：进阶 | 可靠性：参考
> 标签：#Rust #异步编程 #Future
```

这不是机器元信息的唯一来源，只是方便用户阅读。

### 5.4 调试信息开关

设置页增加开关：

```text
笔记内容 / 高级
[ ] 在笔记末尾附加调试信息
```

对应配置：

```ts
debug_metadata: boolean // 默认 false
```

行为：

| 开关 | 新生成笔记 | 增量更新 | 追问模式 |
|------|------------|----------|----------|
| `false` | 不输出 `## 调试信息` | 不追加调试条目 | 不追加调试条目 |
| `true` | 输出 `## 调试信息` | 追加本次运行记录 | 追加本次追问记录 |

调试信息是“可见附录”，不是功能必需数据。App 读取笔记时不依赖它。

### 5.5 调试信息样式

当 `debug_metadata=true` 时，在文档末尾追加：

```markdown
---

## 调试信息

> 以下信息用于排查处理链路、Token 消耗和模型决策。普通阅读可忽略。

### v2 · 2026-05-31 22:45 · 增量更新

#### A. 流水线耗时

| 步骤 | 工具 / 模型 | 耗时 | Token | 说明 |
|------|-------------|------|-------|------|
| 输入识别 | classifier.ts | 20ms | - | 识别为 B 站视频 |
| 字幕转写 | faster-whisper | 02:31 | - | small / cuda |
| 关键帧 | ffmpeg | 00:18 | - | smart mode |
| AI 生成 | deepseek-v4-pro | 01:42 | 45K | note_generation |

#### B. 工具调用

| 工具 | 输入 | 输出 | 状态 |
|------|------|------|------|
| download_video | bilibili:BVxxx | video.mp4 | ok |
| transcribe_audio | audio.mp3 | subtitle.srt | ok |
| extract_keyframes | subtitle.srt | 28 frames | ok |

#### C. 运行环境

| 项目 | 值 |
|------|----|
| App 版本 | 0.1.0-alpha.1 |
| 配置目录 | ~/.myriad-mind-app |
| FFmpeg | bundled |
| ASR | faster-whisper |
| AI Provider | deepseek |
| AI Model | deepseek-v4-pro |

#### D. 决策链路

1. 输入命中 `bilibili`。
2. 输出目录命中已有分类 `Rust`。
3. fingerprint 命中已有来源，选择重新炼化。
4. 使用 `note_generation` Hook 合并本次要求。
5. 生成完成后更新 `current_version` 到 `2`。
```

### 5.6 调试信息位置

文末章节顺序固定为：

```text
## 更新记录
## 问答记录
## 调试信息
## 大衍决元信息
```

理由：

- 更新记录是用户关心的版本历史。
- 问答记录是用户学习过程的一部分。
- 调试信息最偏工程，放最后，避免干扰阅读。
- 大衍决元信息必须保持文件末尾，便于程序快速定位和整体替换。

如果旧 Skill 笔记里已经有 `> 🔧 调试信息 / Debug Trace` 这种旧格式，导入时不强制删除；下次保存时可以迁移成新的 `## 调试信息` 章节。

---

## 六、元信息读取流程

读取 Markdown 时按顺序处理：

```text
1. 从文件末尾向前查找 MYRIAD_MIND_METADATA 块
2. 校验 schema
3. 检查 P0 必需字段
4. 缺字段时先规则补全
5. 规则补不全时调用 DeepSeek 修复
6. 修复后的元信息块写回 Markdown 末尾
7. 继续执行炼化 / 追问 / 搜索 / 修为面板
```

### 6.1 规则补全

不需要 AI 的字段优先用规则生成：

| 字段 | 规则 |
|------|------|
| `schema` | 固定 `myriad-mind-note/v1` |
| `id` | `note_` + 文件路径 / 内容 hash 前 8 位 |
| `title` | H1 标题 > 文件名 > `未命名笔记` |
| `slug` | title slugify |
| `category` | 父目录名 > `未分类` |
| `created_at` | 文件创建时间 > 当前时间 |
| `updated_at` | 文件修改时间 > 当前时间 |
| `current_version` | 从 `## 更新记录` 中最大 `vN` 推断，缺失则 `1` |
| `app.name` | 固定 `myriad-mind-app` |
| `app.version` | 当前 App 版本 |
| `app.build` | 当前构建号，取不到则省略 |
| `pipeline.schema_version` | 当前管线版本 |
| `pipeline.mode` | 当前任务类型 |
| `ai.provider` | 当前配置的 Provider |
| `ai.model` | 当前实际使用模型 |
| `sources[].raw` | 从正文中的“来源/原始资源”或 URL 正则提取 |
| `sources[].fingerprint` | canonical 或 raw URL hash |

### 6.2 DeepSeek 修复

需要理解正文语义的字段再交给 DeepSeek：

| 字段 | 需要 AI 的原因 |
|------|----------------|
| `summary` | 需要概括正文 |
| `topics` | 需要抽取核心主题 |
| `tags` | 需要统一标签 |
| `difficulty` | 需要判断入门/进阶/深入 |
| `reliability` | 需要判断内容可信度 |
| `prerequisites` | 需要推断前置知识 |
| `content_type` | 需要根据正文判断视频笔记/文章/代码分析 |
| `category` | 规则不确定时由 AI 建议分类 |

---

## 七、DeepSeek 元信息修复 Prompt

### 7.1 输入

只给 DeepSeek 必要内容，避免浪费 1M 上下文：

```text
文件路径
父目录
文件名
已有元信息块（如果有）
H1/H2 目录
正文前 3000-8000 字
更新记录摘要
问答记录标题列表
```

### 7.2 输出必须是 JSON

```json
{
  "title": "Rust 异步编程",
  "category": "Rust",
  "summary": "解释 Rust async/Future 的核心机制与实践注意事项。",
  "topics": ["Rust", "async", "Future"],
  "tags": ["Rust", "异步编程", "Future"],
  "difficulty": "intermediate",
  "reliability": "reference",
  "prerequisites": ["Rust 所有权", "trait 基础"],
  "content_type": "video_note",
  "language": "zh-CN",
  "confidence": 0.86,
  "reason": "标题、章节和正文多次出现 Future、async/await、Pin 等概念。"
}
```

### 7.3 约束

- 只能输出 JSON。
- 不要改写正文。
- 不要编造来源链接。
- `sources[]` 只能来自已有元信息、正文链接或调用方传入的当前输入。
- `confidence < 0.6` 时，分类写入 `未分类`，并在 UI 中提示用户确认。

---

## 八、元信息修复策略

### 8.1 何时自动写回

可以自动写回：

- `schema`
- `id`
- `slug`
- `created_at`
- `updated_at`
- `current_version`
- `summary`
- `topics`
- `tags`
- `difficulty`
- `content_type`

需要用户确认：

- `category` 从一个已有目录迁移到另一个目录。
- DeepSeek 建议把文件移动到新目录。
- `sources[]` 中新增来自正文以外的来源。
- AI 修复置信度低于 `0.6`。

### 8.2 用户删除元信息块

如果用户删除了元信息：

```text
1. 不报错。
2. 读取 H1、文件名、父目录、更新记录。
3. 规则生成最小元信息块。
4. 调用 DeepSeek 补充语义字段。
5. 在文档末尾重新插入 `## 大衍决元信息`。
```

追加更新记录：

```markdown
### v3 · 2026-05-31 23:20 · 元信息修复

**原因：** 检测到文档元信息缺失，已根据正文和文件路径自动重建。
```

### 8.3 旧 Skill 笔记导入

旧笔记可能没有大衍决元信息块，但正文里有：

- 视频信息表。
- 原始资源。
- 推荐阅读时长。
- 难度。
- 可靠性。
- 标签。
- 文档元信息尾页。
- 调试信息尾页。

迁移策略：

```text
先正则提取旧格式字段
→ 映射到元信息块
→ DeepSeek 补充缺失语义字段
→ 保留旧正文不动
```

旧字段映射：

| 旧格式 | 新字段 |
|--------|--------|
| `原始资源` | `sources[].raw` |
| `来源平台` | `sources[].type` |
| `视频ID` | `sources[].canonical` |
| `推荐阅读时长` | `estimated_reading_minutes` |
| `难度` | `difficulty` |
| `可靠性` | `reliability` |
| `#标签` | `tags` |

旧调试信息映射：

| 旧格式 | 新字段 / 新章节 |
|--------|-----------------|
| `> 🔧 调试信息 / Debug Trace` | `## 调试信息` |
| `流水线耗时` | `## 调试信息` / `A. 流水线耗时` |
| `工具调用` | `## 调试信息` / `B. 工具调用` |
| `决策链路` | `## 调试信息` / `D. 决策链路` |
| `Token 消耗` | 元信息块 `ai.tokens` 或调试表格 |

---

## 九、重复炼化与增量更新

### 9.1 同一输入

如果新输入 fingerprint 已存在于 `sources[]`：

```text
current_version + 1
追加 vN · 重新炼化
默认更新同一文件
```

### 9.2 不同输入合并到同一文件

如果用户指定合并到已有文件：

```text
sources[] 追加新来源
current_version + 1
app / pipeline / ai 更新为本次实际运行信息
追加 vN · 增量更新
```

P0 可以只追加增量记录，不重写正文。

P1 再做 AI 合并正文：

```text
旧正文 + 新材料
→ DeepSeek 输出合并后正文 + 本次增量摘要
→ 程序替换正文区，保留更新记录和问答记录
```

---

## 十、追问模式如何使用元信息

追问模式读取顺序：

```text
1. 大衍决元信息块
2. 当前正文
3. sources[] 原始资源
4. assets transcript / keyframes
5. 问答记录
```

如果元信息块缺失：

```text
先修复元信息
再进入 note_qa
```

如果 `sources[]` 缺失：

- 仍可基于当前 Markdown 回答。
- UI 提示“缺少原始来源，只基于笔记回答”。

如果设置开启 `debug_metadata`：

- 追问完成后，在 `## 调试信息` 追加一条 `note_qa` 运行记录。
- 记录问题摘要、模型、耗时、Token、是否写回原文。
- 不记录用户 API Key、完整密钥、Cookie 或其他敏感凭据。

---

## 十一、修为面板如何使用元信息

修为面板扫描 `.md` 时：

1. 解析文件末尾的大衍决元信息块。
2. 缺失则触发轻量修复。
3. 读取：
   - `title`
   - `category`
   - `topics`
   - `tags`
   - `difficulty`
   - `estimated_reading_minutes`
   - `created_at`
   - `updated_at`
4. 生成统计、标签云、技能矩阵、知识图谱。

修为面板默认不读取 `## 调试信息`，避免工程附录污染学习统计。

大量文件扫描时不要逐篇调用 DeepSeek。

推荐策略：

| 场景 | 行为 |
|------|------|
| 单篇打开 / 追问 | 可立即修复 |
| 扫描 100 篇笔记 | 先规则修复，AI 修复进入后台队列 |
| 用户点击“整理元信息” | 批量调用 DeepSeek，但要展示预计 token |

---

## 十二、实现模块建议

```text
packages/core/src/notes/
├── note-metadata-block.ts # parse / stringify / replace tail metadata block
├── note-metadata.ts    # NoteMetadata 类型与校验
├── repair.ts           # 规则修复
├── fingerprint.ts      # 输入来源指纹
├── sections.ts         # 更新记录 / 问答记录 / 调试信息定位
├── debug-metadata.ts   # 调试信息渲染与追加
└── slug.ts             # 目录安全名

apps/desktop/src-tauri/src/commands/notes/
├── read_note.rs
├── write_note.rs
├── repair_metadata.rs
├── append_note_record.rs
└── append_debug_trace.rs
```

DeepSeek 修复通过 MindEngine：

```text
MindEngine task: metadata_repair
Model: deepseek-v4-flash
```

说明：

- 元信息修复多数是轻任务，用 Flash 即可。
- 只有正文特别长且需要更准分类时，再用 Pro。

---

## 十三、验收标准

- 新生成笔记末尾包含 `myriad-mind-note/v1` 大衍决元信息块。
- 元信息不再写入单独 `metadata.json`。
- 设置关闭 `debug_metadata` 时，新笔记文末不出现 `## 调试信息`。
- 设置开启 `debug_metadata` 时，新笔记文末出现规范化 `## 调试信息`。
- 增量更新和追问模式只在 `debug_metadata=true` 时追加调试条目。
- 调试信息不包含 API Key、Cookie、Token 原文等敏感信息。
- 元信息块包含 `app.version`、`app.build`、`pipeline.schema_version`、`pipeline.mode`。
- 元信息块包含 `ai.provider`、`ai.model`、`ai.api_style`、`ai.prompt_preset`。
- 用户删除元信息块后，App 能根据正文和路径恢复最小元信息。
- 旧 Skill 笔记无元信息块时，可以导入并补齐元信息。
- 旧 Skill 的 `> 🔧 调试信息 / Debug Trace` 可以保留或迁移到 `## 调试信息`。
- DeepSeek 修复只输出 JSON，程序负责写回 Markdown。
- 同一输入重复炼化时，能根据 `sources[].fingerprint` 识别。
- 用户指定合并到同一文件时，`sources[]` 追加来源，`current_version` 递增。
- 追问模式在元信息缺失时先修复，再回答。
- 修为面板可以从文件末尾元信息块快速读取标题、分类、标签、难度和阅读时长。

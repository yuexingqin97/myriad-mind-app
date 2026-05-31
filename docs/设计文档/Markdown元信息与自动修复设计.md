# Markdown 元信息与自动修复设计

> 参考来源：`D:\Project\MyClaude\myriad-mind` 验证版逻辑。  
> 设计目标：一篇 Markdown 就是一篇完整笔记实体。元信息优先放在 Markdown front matter 中；如果用户删除了元信息，或导入旧笔记没有元信息，App 可以先规则补全，再调用 DeepSeek 生成/修复元信息。

---

## 一、设计结论

可以这样设计，而且很适合大衍决：

```text
Markdown 正文
  ├── front matter：机器可读元信息
  ├── 正文：用户可读学习笔记
  ├── 更新记录：版本增量历史
  └── 问答记录：追问模式追加记录
```

不再额外依赖 `metadata.json`。

好处：

- 用户复制、移动、同步单个 `.md` 文件时不会丢元信息。
- 修为面板、搜索、追问模式都可以只扫描 Markdown。
- 用户删除元信息后也不致命，可以自动修复。
- 旧 Skill 生成的无 front matter 笔记也能导入。

---

## 二、验证版可迁移逻辑

从 `D:\Project\MyClaude\myriad-mind` 里可以迁移这些经验：

| 验证版逻辑 | App 版迁移方式 |
|------------|----------------|
| `NOTE_METADATA` 控制文末生成元信息 | App 固定使用 front matter，文末只保留更新/问答记录 |
| 笔记里有“原始资源”字段 | front matter 中保留 `sources[]` |
| 问答模式读取文档元信息中的原始资源 | 追问模式先读 front matter 的 `sources[]`，缺失时自动修复 |
| 修为面板提取标题、核心主题、难度、前置依赖、技术栈 | 这些字段直接进入 front matter |
| 问答记录追加到文末 | 保留 `## 问答记录` 固定章节 |
| 面板版本号递增 | 笔记自身使用 `current_version` 和 `## 更新记录` |

---

## 三、front matter 结构

### 3.1 P0 必需字段

```yaml
---
schema: myriad-mind-note/v1
id: note_9f3a2c1b
title: Rust 异步编程
category: Rust
slug: rust-async
created_at: 2026-05-31T21:30:12+08:00
updated_at: 2026-05-31T22:45:08+08:00
current_version: 2
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
  generated_at: 2026-05-31T22:45:08+08:00
---
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
| `sources[]` | 输入来源和去重 | 部分可以 |
| `ai` | 生成模型记录 | 可以 |

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

```markdown
---
schema: myriad-mind-note/v1
...
---

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
```

规则：

- `## 更新记录` 和 `## 问答记录` 是固定章节名。
- App 写入时只追加这两个章节，不随意改用户正文。
- 如果用户删除章节，下次增量更新或追问时自动重建。

---

## 五、元信息读取流程

读取 Markdown 时按顺序处理：

```text
1. 解析 front matter
2. 校验 schema
3. 检查 P0 必需字段
4. 缺字段时先规则补全
5. 规则补不全时调用 DeepSeek 修复
6. 修复后的 front matter 写回 Markdown
7. 继续执行炼化 / 追问 / 搜索 / 修为面板
```

### 5.1 规则补全

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
| `sources[].raw` | 从正文中的“来源/原始资源”或 URL 正则提取 |
| `sources[].fingerprint` | canonical 或 raw URL hash |

### 5.2 DeepSeek 修复

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

## 六、DeepSeek 元信息修复 Prompt

### 6.1 输入

只给 DeepSeek 必要内容，避免浪费 1M 上下文：

```text
文件路径
父目录
文件名
已有 front matter（如果有）
H1/H2 目录
正文前 3000-8000 字
更新记录摘要
问答记录标题列表
```

### 6.2 输出必须是 JSON

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

### 6.3 约束

- 只能输出 JSON。
- 不要改写正文。
- 不要编造来源链接。
- `sources[]` 只能来自已有元信息、正文链接或调用方传入的当前输入。
- `confidence < 0.6` 时，分类写入 `未分类`，并在 UI 中提示用户确认。

---

## 七、元信息修复策略

### 7.1 何时自动写回

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

### 7.2 用户删除 front matter

如果用户删除了元信息：

```text
1. 不报错。
2. 读取 H1、文件名、父目录、更新记录。
3. 规则生成最小 front matter。
4. 调用 DeepSeek 补充语义字段。
5. 在文档顶部重新插入 front matter。
```

追加更新记录：

```markdown
### v3 · 2026-05-31 23:20 · 元信息修复

**原因：** 检测到文档元信息缺失，已根据正文和文件路径自动重建。
```

### 7.3 旧 Skill 笔记导入

旧笔记可能没有 YAML front matter，但正文里有：

- 视频信息表。
- 原始资源。
- 推荐阅读时长。
- 难度。
- 可靠性。
- 标签。
- 文档元信息尾页。

迁移策略：

```text
先正则提取旧格式字段
→ 映射到 front matter
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

---

## 八、重复炼化与增量更新

### 8.1 同一输入

如果新输入 fingerprint 已存在于 `sources[]`：

```text
current_version + 1
追加 vN · 重新炼化
默认更新同一文件
```

### 8.2 不同输入合并到同一文件

如果用户指定合并到已有文件：

```text
sources[] 追加新来源
current_version + 1
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

## 九、追问模式如何使用元信息

追问模式读取顺序：

```text
1. front matter
2. 当前正文
3. sources[] 原始资源
4. assets transcript / keyframes
5. 问答记录
```

如果 front matter 缺失：

```text
先修复元信息
再进入 note_qa
```

如果 `sources[]` 缺失：

- 仍可基于当前 Markdown 回答。
- UI 提示“缺少原始来源，只基于笔记回答”。

---

## 十、修为面板如何使用元信息

修为面板扫描 `.md` 时：

1. 解析 front matter。
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

大量文件扫描时不要逐篇调用 DeepSeek。

推荐策略：

| 场景 | 行为 |
|------|------|
| 单篇打开 / 追问 | 可立即修复 |
| 扫描 100 篇笔记 | 先规则修复，AI 修复进入后台队列 |
| 用户点击“整理元信息” | 批量调用 DeepSeek，但要展示预计 token |

---

## 十一、实现模块建议

```text
packages/core/src/notes/
├── frontmatter.ts      # parse / stringify / merge
├── note-metadata.ts    # NoteMetadata 类型与校验
├── repair.ts           # 规则修复
├── fingerprint.ts      # 输入来源指纹
├── sections.ts         # 更新记录 / 问答记录定位
└── slug.ts             # 目录安全名

apps/desktop/src-tauri/src/commands/notes/
├── read_note.rs
├── write_note.rs
├── repair_metadata.rs
└── append_note_record.rs
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

## 十二、验收标准

- 新生成笔记顶部包含 `myriad-mind-note/v1` front matter。
- 元信息不再写入单独 `metadata.json`。
- 用户删除 front matter 后，App 能根据正文和路径恢复最小元信息。
- 旧 Skill 笔记无 front matter 时，可以导入并补齐元信息。
- DeepSeek 修复只输出 JSON，程序负责写回 Markdown。
- 同一输入重复炼化时，能根据 `sources[].fingerprint` 识别。
- 用户指定合并到同一文件时，`sources[]` 追加来源，`current_version` 递增。
- 追问模式在元信息缺失时先修复，再回答。
- 修为面板可以从 front matter 快速读取标题、分类、标签、难度和阅读时长。


# DeepSeek V4 视觉模式 · 补齐截图审查能力方案

> 编写日期：2026-05-31  
> 背景：Skill 原型中 Claude 的多模态视觉是截图审查/字幕分析的核心引擎，App 需要替代方案  
> 结论：DeepSeek V4 Pro/Flash 2026 年 4 月底上线视觉模式，单图仅 81 token，可以低成本补齐

---

## 一、问题回顾：Skill 原型靠 Claude 视觉做了什么

[Skill 原型视频流程](./Skill原型-视频处理流程.md) 中有三个步骤依赖 AI 视觉推理：

| 步骤 | 做什么 | 输入 | 输出 | 多模态依赖 |
|------|--------|------|------|-----------|
| **4.5 字幕分析** | 从字幕文本中识别"这段值得截图"的时刻 | `text.txt`（纯文本） | `guided_timestamps.json` | ❌ 不需要 |
| **4.7 精准截图** | 按引导时间点 + 场景检测 + 保底截取关键帧 | `video.mp4` + 引导时间点 | `frames/*.png` + `keyframes.json` | ❌ 不需要（ffmpeg 干活） |
| **7.1 截图审查** | 逐张看截图，判断类型/质量/去重，选出嵌入笔记的 | `frames/*.png`（图片） | 审查表（✅/⚖️/❌） | ✅ **必须** |

**步骤 7.1 是三阶段流程：**

```
第一阶段: 逐张审查
  → 前置校验: 字幕+画面交叉对照（查字幕 SRT 同时间段内容）
  → 类型标签: PPT_TITLE / CODE_BLOCK / ARCH_DIAGRAM / TALKING_HEAD / ...
  → 评分: 最终得分 = 基础分 × 上下文加成

第二阶段: 输出审查表
  | # | 时间 | 来源 | 字幕摘要 | 类型标签 | 分数 | 决策 | 嵌入位置 |

第三阶段: 自检清单
  □ 审查表行数 = 截图总数
  □ 每张 ❌ 有明确理由
  □ 每张 ✅ 标注嵌入位置
  □ 相似截图已去重
  □ 选中数量合理 (5-15张, ≤50%)
```

没有视觉 AI，这一步无法自动化——App 要么跳过截图审查（所有截图全嵌入 → 质量差），要么让用户手动选（体验差）。

---

## 二、DeepSeek V4 视觉能力评估

### 2.1 能力矩阵

| 能力 | 水平 | 对截图审查的意义 |
|------|------|-----------------|
| 图像内容理解 | 93% 准确率 | ✅ 能区分 PPT / 代码 / 架构图 / 人脸 |
| 代码阅读 | 强（文本侧强 + 视觉辅助） | ✅ 能识别代码截图并读内容 |
| 视觉推理 | 90% 精度 | ✅ 能判断"这张图有信息量吗" |
| 图表分析 | 支持 | ✅ 能识别 flowchart / 数据表 |
| 精细视觉 | 不稳定 | ⚠️ 但截图审查不需要精细识别 |
| 多图对话 | 支持 | ✅ 可以一次提交多张做对比去重 |
| 图像生成 | ❌ 不支持 | 不需要 |

### 2.2 成本估算

```
单张图片 = 81 tokens（DeepSeek 自研压缩）
每张审查 prompt ≈ 200 tokens（文本指令 + 字幕上下文）
每张返回 ≈ 300 tokens（JSON 审查结果）

单张总成本 ≈ (81 + 200 + 300) / 1,000,000 × 1元 ≈ 0.00058 元

一个 55 分钟视频 → 约 25 张候选截图：
  全部审查 ≈ 25 × 0.00058 ≈ 0.015 元

一个月炼化 100 个视频 → 约 1.5 元
```

**结论：成本可以忽略不计。**

### 2.3 原生视觉 vs OCR 方案

DeepSeek V4 是原生视觉理解架构，不是 OCR 转译：
- 能理解画面结构（"左边是代码，右边是运行结果"）
- 能识别 UI 元素（"这是一个 IDE 截图，光标在第 15 行"）
- 不依赖图片中的文字量（纯图表的架构图也能分析）

这意味着审查质量接近 Skill 原型中 Claude 的水平。

---

## 三、实施方案

### 3.1 整体架构

```
                    ┌─────────────────────────┐
                    │   DeepSeek V4 Vision API │
                    │   (单图 81 token)         │
                    └──────────┬──────────────┘
                               │
  ┌────────────────────────────┼────────────────────────────┐
  │                            │                            │
  ▼                            ▼                            ▼
步骤 4.5                     步骤 7.1                     步骤 7.4
字幕分析                     截图审查                     教程检测
(纯文本,                    (提交截图 +                   (提交截图,
不需要视觉)                  字幕上下文,                  判断内容类型)
                             返回审查表)
```

### 3.2 步骤 4.5：字幕分析（纯文本，无需视觉）

这一步本质是 NLP 任务 —— 从字幕文本中找"看这段代码""如图所示"等信号词。DeepSeek V4 文本能力完全胜任。

**输入：** `subtitle.srt` 全文  
**Prompt 要点：**
```
分析以下视频字幕，找出画面价值高的时刻。识别以下信号：
- 画面展示: "看这段代码"、"如图所示"、"注意这个表格"
- 操作演示: "点击这里"、"打开终端"、"运行一下"
- 代码相关: "这段代码的作用是"、"函数定义"
- PPT翻页: "下一页"、"第一个要点"、"这一章"
- 对比切换: "对比一下"、"前后的区别"
- 运行效果: "运行结果"、"输出是"、"报错了"

跳过：开场闲聊、片尾、过渡寒暄

输出 JSON 数组，每项 {ts, reason}，ts 精确到秒，8-25 个推荐。
```

**输出：** `guided_timestamps.json`

### 3.3 步骤 4.7：精准截图（已有基础）

当前 App 的 `extract_keyframes.py` 已支持基本参数。需要在 App 管线中增加：

1. 步骤 4.5 产出的 `guided_timestamps.json` 传给脚本的 `--timestamps` 参数
2. 配置参数从 `config.keyframes` 读取（`interval/max_frames/mode`）
3. 输出 `keyframes.json` 含 trigger 标记（guided/scene/gap）

**改动量：** 前端传参 + Rust pipeline 串步骤即可，核心逻辑在 Python 脚本里。

### 3.4 步骤 7.1：截图审查（核心新功能）

这是 DeepSeek V4 视觉模式的用武之地。

#### 方案 A：逐张审查（推荐，质量最高）

```rust
// Rust 伪代码
async fn review_keyframes(
    frames: &[Keyframe],
    subtitle_srt: &str,
    deepseek_api_key: &str,
) -> Result<Vec<ReviewedFrame>, AppError> {
    let mut results = vec![];
    for frame in frames {
        let prompt = build_review_prompt(frame, subtitle_srt);
        // 调用 DeepSeek Vision API
        let response = call_deepseek_vision(
            api_key,
            &prompt,
            &frame.image_path,  // base64 or URL
        ).await?;
        let review: FrameReview = serde_json::from_str(&response)?;
        results.push(ReviewedFrame { frame, review });
    }
    // 去重
    results = deduplicate_similar(results);
    // 按分数排序，选前 50%
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(results.len() / 2);
    Ok(results)
}
```

**单张 Prompt 模板：**
```
你是一个视频学习笔记的截图审查员。请分析这张视频截图，结合下方字幕上下文，给出审查结论。

## 截图时间戳：{timestamp}
## 对应字幕（前后 30 秒）：
{subtitle_context}

## 请判断：
1. 类型标签（选一个）：
   PPT_TITLE / CODE_BLOCK / ARCH_DIAGRAM / RUN_RESULT / TOOL_UI / 
   DATA_TABLE / SIMPLE_CHART / SPLIT_SCREEN / TALKING_HEAD / 
   PLAIN_TEXT / BLACK_SCREEN / STATIC_REPEAT / NO_INFO

2. 是否有信息增量？（画面+字幕组合是否提供了纯字幕之外的信息）
   - 实质性内容 + 信息画面 → 高
   - 实质性内容 + 静态人脸 → 无（字幕已覆盖）
   - 闲聊/过渡 → 低
   - 无字幕时段 → 无

3. 与前后截图的相似度（0-1，1=完全相同）
4. 推荐嵌入的笔记章节（如"三.核心概念"、"四.1 代码实现"；不适合的填"—"）

输出 JSON：
{"type_tag": "...", "info_score": 0-3, "similarity_vs_prev": 0-1, "embed_section": "...", "reason": "20字以内"}
```

**优点：** 质量最高，每张独立判断  
**缺点：** N 次 API 调用 = N 次网络延迟

#### 方案 B：批量审查（快，适合图不多的情况）

一次提交所有截图（最多 25 张），让 DeepSeek 一次返回全部审查结果。

**优点：** 1 次 API 调用  
**缺点：** 多图场景下注意力稀释（DeepSeek 可能有"只看前几张"的倾向）

#### 推荐：混合方案

```
if frames.len() <= 10:
    方案 B（批量审查）
else if frames.len() <= 30:
    方案 A（逐张审查，并发 3 张）
else:
    先批量粗筛（去掉 BLACK_SCREEN/TALKING_HEAD），再逐张精审
```

### 3.5 步骤 7.4：教程模式检测

在截图审查的同时，取前 5 张截图 + 视频标题，让 DeepSeek 判断：

**Prompt：**
```
根据视频标题和前 5 张关键帧截图，判断这是否为"操作型教程"。

操作型教程特征：
- 标题含"教程"/"入门"/"实战"/"配置"/"搭建"/"tutorial"/"how to"
- 画面中有大量 IDE/终端/操作界面截图
- 内容以"一步步跟着做"为主

输出 JSON：{"is_tutorial": true/false, "confidence": 0-1, "signals": ["标题含'教程'", "5张中有4张是操作界面"]}
```

命中 → 在 AI 生成笔记时注入教程模式 prompt，引导生成操作流程图。

---

## 四、与现有管线的集成

### 4.1 当前管线步骤

```
execute_pipeline
  → run_video_pipeline
    → download / prepare
    → extract_audio (ffmpeg)
    → transcribe (faster-whisper)     ← 产出 text.txt + subtitle.srt
    → extract_keyframes               ← 产出 frames/*.png
    → ai::generate_note               ← DeepSeek 纯文本生成笔记
    → save_note
```

### 4.2 改造后管线

```
execute_pipeline
  → run_video_pipeline
    → download / prepare
    → extract_audio (ffmpeg)
    → transcribe (faster-whisper)     ← 产出 text.txt + subtitle.srt
    → 🆕 analyze_subtitle             ← 步骤 4.5: DeepSeek 纯文本 → guided_timestamps.json
    → extract_keyframes               ← 步骤 4.7: 传入 guided_timestamps → frames/*.png + keyframes.json
    → 🆕 review_screenshots           ← 步骤 7.1: DeepSeek Vision → 审查表 + 选中截图列表
    → 🆕 detect_tutorial_mode         ← 步骤 7.4: DeepSeek Vision → is_tutorial
    → 🆕 copy_selected_frames         ← 复制选中截图到 assets/{VIDEO_ID}/
    → ai::generate_note               ← 注入审查结果 + 教程模式 flag → 更高质量笔记
    → save_note
```

### 4.3 新增 Rust 模块

```
apps/desktop/src-tauri/src/commands/
├── ai/
│   ├── mod.rs           ← 已有（文本生成）
│   ├── types.rs         ← 已有
│   └── vision.rs        ← 🆕 DeepSeek Vision API 封装
├── pipeline/
│   ├── mod.rs           ← 已有
│   ├── video.rs         ← 🆕 视频管线专用逻辑
│   ├── subtitle.rs      ← 🆕 字幕分析 (步骤 4.5)
│   └── review.rs        ← 🆕 截图审查 (步骤 7.1)
```

### 4.4 配置扩展

`packages/core/src/schema.ts` 需新增：

```typescript
// 截图审查配置
export const ScreenshotReviewSchema = z.object({
  enabled: z.boolean().default(true),      // 是否启用 AI 审查
  mode: z.enum(["batch", "single", "hybrid"]).default("hybrid"),
  max_review_frames: z.number().min(5).max(50).default(25),
  min_score: z.number().min(0).max(3).default(2),  // 低于此分跳过
  max_selected: z.number().min(3).max(20).default(15),
});

// 追加到 FeaturesSchema
export const FeaturesSchema = z.object({
  // ... 现有字段
  screenshot_review: ScreenshotReviewSchema.optional(),
  tutorial_detection: z.boolean().default(true),
});
```

---

## 五、实现优先级建议

| 优先级 | 任务 | 理由 | 预估工时 |
|--------|------|------|----------|
| **P0** | `ai/vision.rs` — DeepSeek Vision API 基础封装 | 所有视觉功能的基础 | 2h |
| **P1** | 步骤 7.1 截图审查（混合方案） | 对笔记质量提升最直接 | 3h |
| **P1** | 审查结果注入 `generate_note` prompt | 让 AI 知道哪些截图该嵌入 | 1h |
| **P2** | 步骤 4.5 字幕分析 → 引导截图 | 提高截图精准度 | 2h |
| **P2** | 步骤 7.4 教程模式检测 | 锦上添花 | 1h |
| **P3** | 截图自动复制到 assets/ | 文件操作，不依赖 AI | 1h |
| **P3** | 配置 Schema 扩展 + 设置页 | 用户可控制 | 1h |

**总计：约 11 小时**

---

## 六、风险与降级

| 风险 | 概率 | 影响 | 降级方案 |
|------|------|------|----------|
| DeepSeek Vision API 不稳定 | 低 | 截图审查中断 | 降级为不过滤，所有截图全嵌入 |
| 视觉识别误判（把 CODE_BLOCK 标成 TALKING_HEAD） | 中 | 漏掉该嵌入的截图 | 降低 min_score 阈值，宁可多选不可漏 |
| 单张审查太慢（N 次 API 调用） | 中 | 管线整体变慢 | 切换到批量模式（1 次调用） |
| DeepSeek 视觉能力未对 API Key 开放 | 低 | 功能不可用 | 检测 API 返回，自动跳回纯文本模式 |

核心原则：**截图审查是增强，不是阻塞**。如果视觉 API 挂了，管线仍然能跑完（所有截图嵌入），只是笔记质量略降。

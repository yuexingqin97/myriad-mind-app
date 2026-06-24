# 代码审查报告 — commit `e60595f5211c0b61c340e74f16e6ee731f56d0ff`

> **提交标题**：`feat: 改为流式传输，日志显示ai 推理过程`
> **作者 / 日期**：Yxqin / 2026-06-23
> **变更范围**：3 文件，+514 / −34
> - `apps/desktop/src-tauri/src/agent/runner.rs`（agent 主循环）
> - `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`（新增 `stream_chat_turn`，`chat_turn` 退役为死代码）
> - `docs/设计文档/AI与模型/Agent 开发计划.md`（文档同步）
>
> **审查方法**：先建立 codegraph 索引（1,134 节点 / 2,523 边），基于调用图定位变更影响面，再逐文件逐行审查，最后 `cargo check` 验证编译状态。执行证据见 `exports/task1/`。

---

## 0. codegraph 索引与影响面定位

### 索引建立

```
$ codegraph init
Indexed 86 files · 1,134 nodes, 2,523 edges in 641ms
```

### 变更符号的影响面（`codegraph impact` / `callers`）

| 变更符号 | 类型 | 位置 | 调用方 | 影响传播 |
|----------|------|------|--------|----------|
| `stream_chat_turn`（新增） | fn | `deepseek.rs:500` | `runner::run`（`runner.rs:60`） | → `pipeline::execute_pipeline`（`pipeline.rs:138`）→ 前端 `usePipeline.ts` 监听 `mind-stream` / `pipeline-progress` |
| `chat_turn`（退役） | fn | `deepseek.rs:312` | **0 调用方**（`codegraph callers` 确认） | 死代码，无影响传播 |
| `run`（修改函数体） | fn | `runner.rs:60` | `pipeline.rs:138`（`crate::agent::run`） | 唯一入口；改动其内部流式行为直接影响整条管线与前端流式 UI |

### 前端契约影响（基于调用图追踪）

`runner::run` → `emit("mind-stream", …)` / `emit_progress(...)` → 前端 `api.ts:134`（`pipeline-progress`）与 `api.ts:272`（`mind-stream`）→ `usePipeline.ts:259` 消费。

本提交改变了 `mind-stream` Delta 事件的**语义与发送时机**（从「循环结束后分块发最终笔记」改为「每轮流式发 `delta.content`」），前端消费侧未改动 —— 这是本次审查发现的最大影响面，详见 [H-1] 与 [M-1]。

---

## 1. 问题清单（按严重度分级）

> 分级标准：**Critical** = 数据损坏 / 安全漏洞 / 必然崩溃；**High** = 主路径高频触发 / 可致功能不可用；**Medium** = 有损质量或体验、边界条件触发；**Low** = 风格 / 微优化 / 鲁棒性细节。

### 🔴 Critical

**无 Critical 问题。**

### 🟠 High

#### [H-1] SSE 流按 chunk 独立切行，跨 chunk 的 `data:` 事件会被静默丢弃 — 稳定性 / 正确性

- **文件 / 行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:596-602`（`stream_chat_turn`）；同模式亦存在于 `stream_deepseek:187`（既有遗留，本次在主路径复制）
- **问题原因**：
  ```rust
  while let Some(chunk) = stream.next().await {
      // …
      for line in chunk.split(|&b| b == b'\n') {   // 每个 chunk 独立切行
          let line = String::from_utf8_lossy(line);
          let trimmed = line.trim();
          // …
          if let Ok(parsed) = serde_json::from_str::<Value>(data) { // 解析失败即丢弃
  ```
  `reqwest::bytes_stream()` 按 TCP 段边界产出 chunk，**不保证 SSE 事件边界**。一条 `data: {"choices":[0]…}\n` 可能被拆到两个 chunk：第一段 `data: {"choi`（`strip_prefix` 命中但 JSON 解析失败 → `if let Ok` 静默丢弃）、第二段 `ces":[0]…}\n`（无 `data: ` 前缀 → 整条丢弃）。
- **风险说明**：被丢弃的事件可能是：
  - `delta.content` 片段 → 最终笔记**缺字**（且无报错，前端 Done 用后端 `final_content` 兜底，但 `full_text` 本身已缺）。
  - `delta.tool_calls.function.arguments` 片段 → 拼接出的 arguments JSON **不完整** → runner.rs:257 `serde_json::from_str` 失败 → 该工具被跳过 + 回喂诊断 → 至少浪费一轮（MAX_STEPS=25 下放大耗损）。
  - `usage` / `finish_reason` → 统计缺失。
  agent loop 每轮都走此路径，25 轮累积丢事件概率不可忽略；长输出 / 网络抖动下更易触发。
- **修复建议**：引入跨 chunk 的行缓冲，按 `\n` 边界累积完整行后再解析：
  ```rust
  let mut buf = String::new();
  while let Some(chunk) = stream.next().await {
      let chunk = chunk.map_err(|e| AppError::Ai { kind: "network".into(), message: format!("流读取失败: {e}") })?;
      buf.push_str(&String::from_utf8_lossy(&chunk));
      // 按行消费，最后一段不完整行留在 buf
      while let Some(nl) = buf.find('\n') {
          let line = buf[..nl].trim();
          buf.drain(..=nl);
          if line.is_empty() { continue; }
          if let Some(data) = line.strip_prefix("data: ") {
              if data.trim() == "[DONE]" { continue; }
              if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                  // …原解析逻辑…
              }
          }
      }
  }
  ```
  同步修复 `stream_deepseek:181-261`（消除既有遗留）。

#### [H-2] 流式请求无超时，SSE 半挂会无限阻塞 agent 主循环 — 稳定性

- **文件 / 行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:508`（`let client = Client::new();`）；同问题见 `:85` / `:320` / `:807`
- **问题原因**：`reqwest::Client::new()` **默认无超时**。`stream_chat_turn` 用 `max_tokens: 131072` + thinking mode，单轮可能很长；若服务端建立连接后中途停止发送数据（网关超时 / 代理挂起），`stream.next().await`（deepseek.rs:590）将**永久阻塞**，没有任何读超时兜底。
- **风险说明**：`runner::run` 在 `pipeline::execute_pipeline`（pipeline.rs:138）中以 `await` 串行调用，一个挂死的轮会**冻结整条管线**，用户只能强杀进程。取消标志 `AtomicBool` 只在**轮顶部**检查（runner.rs:136），流式接收中途无法响应取消。对比 `fetch.rs:98-111` 已正确用 `Client::builder().timeout(30s)`，AI 临界路径反而裸建客户端。
- **修复建议**：用 builder 设置连接 + 整体超时（流式场景建议用 `connect_timeout` + 周期性 `read_timeout`，或外层 `tokio::time::timeout` 包裹 `stream.next()`）：
  ```rust
  let client = Client::builder()
      .connect_timeout(std::time::Duration::from_secs(15))
      .timeout(std::time::Duration::from_secs(180)) // 整轮上限
      .build()
      .map_err(|e| AppError::Other(format!("reqwest build: {e}")))?;
  // 或对流式读取逐 chunk 加超时：
  use tokio::time::timeout;
  while let Some(chunk) = timeout(Duration::from_secs(30), stream.next()).await
      .map_err(|_| AppError::Ai { kind: "timeout".into(), message: "SSE 读取超时".into() })? { … }
  ```
  顺带把 `client` 提到 `run` 外层复用（见 [L-1]）。

### 🟡 Medium

#### [M-1] 工具轮的中间 `content` 经 `Delta` 涌入笔记预览区 / 日志，造成 UI 污染与日志重复 — 正确性 / 可维护性（回归）

- **文件 / 行号**：`deepseek.rs:613-621`（`stream_chat_turn` 对每轮 `delta.content` 发 `MindStreamEvent::Delta`）；前端 `apps/desktop/src/hooks/usePipeline.ts:269-287` 与 `:299-307`
- **问题原因**：旧实现只在 loop 结束后把 `final_content` 分块发 `Delta`（runner.rs 旧 305-321，本次被删）。新实现中 `stream_chat_turn` 在**每一轮**（含工具轮）都把 `delta.content` 发 `Delta`。但前端 `usePipeline.ts` 的 `Delta` 处理把**所有** Delta 累积进同一个 `streamAccumRef`（:270）当作笔记正文，并 `pushLog("output", chunk)`（:275）。
  - 工具轮里 `content` 通常是 agent 的中间口头语（如「我先去取字幕」），并非笔记内容。
  - 这些片段被累积进 `streamingText`（笔记预览区）显示成「笔记」，并作为 `output` 写进日志。
  - 终止轮 `Done.text = final_content`（仅最后一轮）覆盖预览，但日志里已混入中间片段 + 随后 `pushLog("output", finalText)`（:306）又把完整笔记再记一次 → **日志重复**。
- **风险说明**：最终落盘笔记正确（`persist_note` 用 `agent_result.note_content`），但流式过程中预览区被中间话语污染、日志重复，是相对旧实现的**用户可见回归**。语义上 `Delta` 事件被重载为「笔记正文」与「agent 推理旁白」两种含义，前端无法区分。
- **修复建议**（任选其一）：
  1. **后端区分**：工具轮的 `content` 改用 `ReasoningDelta`（或新增 `AgentChat` 事件）发出，仅终止轮 `content` 走 `Delta`。需要 `stream_chat_turn` 能预知是否终止轮 —— 可在 runner 拿到 `tool_calls.is_empty()` 后再决定是否补发 `Delta`，或拆分「流式累积 + 由 runner 控制何时 emit」。
  2. **前端区分**：在 `mind-stream` 事件里加 `round` / `final` 标记，`usePipeline.ts` 仅在 `final` 时累积进 `streamAccumRef`。
  3. **最小改动**：runner 在每轮开始前 emit 一个 `mind-stream::Reset`（或复用 `Start`）让前端清空 `streamAccumRef`，并在 `done` 时只信任 `event.text`（当前已信任，但日志重复仍需前端在 `done` 时不再 `pushLog("output", finalText)` 若已分块记过）。

#### [M-2] 终止轮 `content` 为空时回退 `reasoning_content` 作为最终笔记 — 正确性 / 质量

- **文件 / 行号**：`apps/desktop/src-tauri/src/agent/runner.rs:199-211`
- **问题原因**：
  ```rust
  let fell_back_to_reasoning = raw_content.trim().is_empty()
      && turn.reasoning_content.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
  if fell_back_to_reasoning {
      raw_content = turn.reasoning_content.clone().unwrap_or_default();
  }
  ```
  thinking mode 下 `reasoning_content` 是模型**内部思维链**（「我应当写一篇关于 X 的笔记……」的元叙述），并非打磨过的 Markdown 正文。直接当作 `final_content` 落盘，可能产出非合法 Markdown / 含半成品 / 含自我指令的笔记。
- **风险说明**：仅在 `content` 为空时触发，属兜底；但一旦命中，笔记质量不可控，且 `Done.text` 会把它当作权威正文推给前端。注释虽已 `log::warn!` 标记，但无人工介入点。
- **修复建议**：命中回退时**重试一轮**（在 charter 里强化「必须把正文放进 content，不要放进 reasoning」），或至少对 `reasoning_content` 做最小清洗（截取最后一个 Markdown 标题之后的正文）。若确需保留兜底，应在 `AgentResult` 里带 `fell_back: bool` 标记供 UI 提示用户复核：
  ```rust
  // 命中回退 → 再给模型一轮机会，明确要求 content 输出
  messages.push(turn.message);
  messages.push(serde_json::json!({ "role": "user", "content": "请把最终笔记 Markdown 直接放在 content 字段返回，不要放在 reasoning。" }));
  continue; // 不 break，进入下一轮
  ```

#### [M-3] `chat_turn` 成为 178 行死代码且文档注释已陈旧误导 — 可维护性

- **文件 / 行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:307-489`
- **问题原因**：`codegraph callers chat_turn` 返回「No callers found」；`cargo check` 明确告警 `warning: function 'chat_turn' is never used`（见 `exports/task1/cargo-check.txt`）。开发计划文档亦承认「`chat_turn` 保留为死代码待后续清理」。但其 doc 注释（:307-311）仍写「用非流式是因为 tool_use 的流式累积……复杂且本项目未验证」，与本次已实证流式的事实**矛盾**，会误导后续维护者以为流式未实现而重新造轮子。
- **风险说明**：死代码增加认知负担、漂移风险（`stream_chat_turn` 演进时 `chat_turn` 不会同步），且 `ToolCallFunction` import（:7）仅因 `chat_turn` 与 `stream_chat_turn` 共用而保留，删除 `chat_turn` 后需确认 import 仍被 `stream_chat_turn` 使用（实际仍使用，安全）。
- **修复建议**：直接删除 `chat_turn`（:307-489）；如需保留对照，加 `#[allow(dead_code)]` 并把注释改为「已退役，参考 `stream_chat_turn`」：
  ```rust
  #[allow(dead_code)] // 已退役，保留供对照；agent loop 改用 stream_chat_turn
  pub async fn chat_turn(…) { … }
  ```

### 🟢 Low

#### [L-1] 每轮 `Client::new()` 新建 HTTP 客户端，丢失连接复用 — 性能

- **文件 / 行号**：`deepseek.rs:508`（`stream_chat_turn`）；同见 `:85` / `:320` / `:807`
- **问题原因**：agent loop 最多 25 轮，每轮 `Client::new()`。`reqwest::Client` 设计为复用（内部连接池），反复新建会重复 DNS / TLS 握手、放弃 keep-alive。
- **风险说明**：单次开销数百 ms 量级，25 轮累积可观；非致命但可优化。
- **修复建议**：在 `runner::run` 顶部建一个 `Client`，通过参数或 `ToolContext` 传入 `stream_chat_turn`；或用 `OnceLock<Client>` 做进程级单例。

#### [L-2] `tc_delta["index"]` 缺失时默认 0，多工具并行无 index 会碰撞 — 正确性（鲁棒性）

- **文件 / 行号**：`deepseek.rs:637`（`let idx = tc_delta["index"].as_i64().unwrap_or(0);`）
- **问题原因**：DeepSeek（OpenAI 兼容）对并行 tool_calls 通常会发 `index`，但 `unwrap_or(0)` 在缺失时把所有调用映射到同一桶 → 后到的覆盖先到的 `id`/`name`（仅追加 arguments 不覆盖，但首 chunk 的 id/name 丢失会导致 :712 `name.is_empty()` 丢弃整条）。
- **风险说明**：当前 DeepSeek 行为下概率低；一旦上游协议变动或单工具不带 index，静默丢工具调用。
- **修复建议**：缺失 index 时按 `tc_buffer.len() as i64` 兜底递增，并在日志告警：
  ```rust
  let idx = tc_delta["index"].as_i64().unwrap_or_else(|| { log::warn!("[llm] tool_call delta missing index, fallback"); tc_buffer.len() as i64 });
  ```

---

## 2. 检查范围与「未发现」说明

为避免笼统结论，以下列出**已检查但未发现问题**的范围及判定依据：

| 维度 | 检查项 | 检查方式 | 结论 |
|------|--------|----------|------|
| **安全** | 工具参数经 `emit_progress(detail=arguments)` 暴露给用户面板是否会泄露密钥 | 逐一审查 `commands/tools/handlers/*.rs` 所有 `ToolSpec::input_schema` | **未发现泄露**。所有工具的密钥（如 `query_ai_douyin` 的 `ai_douyin_api_key`）均在 handler 内部 `read_config_value` 读取，**不作为 LLM 工具参数**；`input_schema` 仅含 `search`/`status`/`url`/`path` 等非敏感字段。runner.rs:252 发送的 `arguments` 不含密钥。 |
| **安全** | 工具结果 `to_llm_text()` 经 `emit_progress` 暴露是否会泄露密钥 | 审查 `to_llm_text`（`tools/mod.rs:248-263`）与各 handler 的 `summary` 构造；`query_ai_douyin` 失败路径 `acquire.rs:629-636` 已脱敏 | **未发现泄露**。成功结果只含摘要 / artifact 引用；失败路径 `query_ai_douyin` 明确「stderr 可能含明文 key → 不回原文，只给脱敏提示」。 |
| **正确性** | 是否存在重复 `Done` 事件（`stream_deepseek` 与 `runner` 各发一次） | 对比 `stream_deepseek:264`（发 Done）与 `stream_chat_turn`（**不发** Done）+ runner.rs:411（loop 后发一次 Done） | **未发现重复**。`stream_chat_turn` 故意不发 Done，仅 runner 在 loop 结束后发一次。 |
| **正确性** | `persist_note` 是否会持久化被污染的中间话语 | 追踪 `agent_result.note_content` 来源 = `final_content`（仅终止轮 / Draft 回收） | **未发现**。落盘内容仅 `final_content`，中间轮 Delta 不入 `final_content`。污染仅限前端预览/日志（见 [M-1]）。 |
| **稳定性** | `cancel` 标志在流式接收中途能否响应取消 | 审查 runner.rs:136（仅轮顶部检查） | **确认为已知限制**（非本次回归）：流式 `await` 中途不检查取消，需配合 [H-2] 的读超时才能在挂起时退出。归入 [H-2] 一并修复。 |
| **可维护性** | 文档与实现一致性 | 对比 `Agent 开发计划.md` 更新与代码 | 文档已同步流式改动并标注 `chat_turn` 退役，一致。 |

---

## 3. 覆盖维度核对

本次审查覆盖以下维度（≥3 项要求已满足）：

- ✅ **正确性**：[H-1] 跨 chunk 丢事件、[M-1] Delta 语义污染、[M-2] reasoning 回退、[L-2] index 碰撞
- ✅ **稳定性**：[H-1] SSE 解析、[H-2] 无超时挂死
- ✅ **性能**：[L-1] Client 复用
- ✅ **安全**：工具参数 / 结果泄露路径（已检查，未发现）
- ✅ **可维护性**：[M-3] 死代码与陈旧注释、[M-1] 事件语义重载

---

## 4. 修复优先级建议

| 优先级 | 项 | 理由 |
|--------|----|------|
| P0（上线前必修） | [H-1] [H-2] | 主路径下高频 / 可致功能不可用且无超时兜底 |
| P1（紧跟） | [M-1] [M-2] | 用户可见质量回归 + 笔记质量风险 |
| P2（顺手清理） | [M-3] [L-1] [L-2] | 降低维护负担 / 鲁棒性微优化 |

---

## 5. 结论

本次提交将 agent loop 由非流式改为 SSE 流式，思路正确、日志体系完善、文档同步到位，`cargo check` 0 错误。但流式 SSE 解析缺少跨 chunk 行缓冲（[H-1]）与请求超时（[H-2]）两项基础设施，在 agent 主路径上构成稳定性隐患；`Delta` 事件语义被重载导致前端预览/日志污染（[M-1]）是相对旧实现的可见回归。建议优先修 [H-1]/[H-2] 后合入。

**执行证据**：`exports/task1/`（commit.diff、commit-stat.txt、codegraph-status.txt、codegraph-impact-*.txt、codegraph-callers-*.txt、cargo-check.txt）。

# Code Review: `e60595f` — 流式传输 & AI 推理过程日志

> **审查范围**：提交 `e60595f5211c0b61c340e74f16e6ee731f56d0ff` 的全部变更  
> **变更文件**：`agent/runner.rs`、`commands/ai/deepseek.rs`、`docs/设计文档/AI与模型/Agent 开发计划.md`  
> **审查方法**：CodeGraph 索引 → 符号级影响分析 → 逐文件审查 → 前端/后端接口对照  
> **变更规模**：+514 / −34 行，2 个 Rust 源文件 + 1 个文档文件  

---

## 影响面分析（CodeGraph 索引结果）

| 符号 | 位置 | 调用者 | 影响 |
|------|------|--------|------|
| `stream_chat_turn` (新增) | `deepseek.rs:500` | `runner.rs:159` | agent loop 核心路径 |
| `chat_turn` (保留) | `deepseek.rs:312` | **无调用者（死代码）** | 仅日志增强，签名未变 |
| `run` | `runner.rs:60` | `pipeline.rs` (execute_pipeline) | agent 主循环 |
| `MindStreamEvent` | `types.rs:115` | `deepseek.rs` ×7, `runner.rs` ×2, `api.ts` ×1 | 跨层事件契约 |
| `emit_progress` | `pipeline.rs:265` | `runner.rs` ×8 | 用户面板日志通道 |

**CodeGraph 索引状态**：1,134 nodes / 2,523 edges / 86 files，索引通过，无 TLE。

---

## 问题清单

### CRITICAL

#### C1. 流式读取期间无法响应取消请求（稳定性 / 正确性）

**文件与行号**：
- `apps/desktop/src-tauri/src/agent/runner.rs:117` — `cancel: Arc<AtomicBool>` 定义
- `apps/desktop/src-tauri/src/agent/runner.rs:136` — 取消检查点（仅 loop 顶部）
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:500` — `stream_chat_turn()` 签名（未接收 cancel 引用）
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:590–701` — `while let Some(chunk)` 流式读取循环（无取消检查）

**原因**：
`runner.rs` 第 117 行创建的 `cancel: Arc<AtomicBool>` 仅在每次 agent loop 迭代顶部（第 136 行）检查一次。但 `stream_chat_turn()` 函数内部有一个 `while let Some(chunk) = stream.next().await` 无限循环（第 590 行），它会在流式读取期间阻塞整个 agent loop。如果用户在此时取消操作（`cancel` 被外部 Tauri 命令置位），取消信号不会被处理，直到当前 `stream_chat_turn` 完整返回为止。

`stream_chat_turn` 的 `max_tokens` 设置为 131,072（第 518 行），实际流式读取可能需要数分钟。用户在此期间会看到 UI 无响应，取消按钮无效。

**风险**：用户取消操作后 agent 仍持续消耗 API 配额和计算资源，最长可能浪费 131K token 的输出。Alpha 阶段用户频繁试用，反复取消的场景会很常见。

**修复建议**：
```patch
// deepseek.rs — stream_chat_turn 签名增加 cancel 引用
 pub async fn stream_chat_turn(
     app_handle: &AppHandle,
     system_prompt: &str,
     messages: &[serde_json::Value],
     tools: &[serde_json::Value],
     thinking: Option<&ThinkingConfig>,
     api_key: &str,
+    cancel: &Arc<AtomicBool>,
 ) -> Result<AgentTurnResult, AppError> {

     // ... body construction ...

     while let Some(chunk) = stream.next().await {
+        // 每 chunk 检查取消标记（流式读取可能持续数分钟）
+        if cancel.load(Ordering::Relaxed) {
+            return Err(AppError::Cancelled);
+        }
         let chunk = chunk.map_err(|e| AppError::Ai { ... })?;
         // ...
     }
```

```patch
// runner.rs — 调用处传入 cancel
 let mut turn =
-    stream_chat_turn(app, &system_prompt, &messages, &tools_for_llm, Some(&thinking), &api_key)
+    stream_chat_turn(app, &system_prompt, &messages, &tools_for_llm, Some(&thinking), &api_key, &cancel)
         .await?;
```

---

### HIGH

#### H1. SSE 行分割方案不可靠，存在跨 chunk 截断风险（正确性）

**文件与行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:596`

**原因**：
```rust
for line in chunk.split(|&b| b == b'\n') {
```
当前实现逐 chunk 以 `\n` 为分隔符切分行。但 SSE 协议基于 `\n\n` 事件边界，且 TCP 字节流不保证 chunk 边界与行边界对齐。当一个 SSE 行的中间落在 chunk 边界时，`line` 会被截断为两个不完整的片段：
- 第一个 chunk 会包含行的前缀部分  
- 第二个 chunk 会包含行的后缀部分

由于代码在每行内执行 `trimmed.strip_prefix("data: ")`，被截断的行由于缺少 `data: ` 前缀会被静默丢弃，导致丢帧。

对比 `stream_deepseek()`（第 187 行）也使用了完全相同的不安全分割方式，这是一个预存问题，但本次 commit 在 `stream_chat_turn` 中复制了同样的问题。

**风险**：SSE 帧丢失意味着部分 tool_calls delta fragment 被丢弃，导致累积的 arguments 不完整或无法 parse。在工具调用轮次中，这会直接导致工具执行失败或产生错误参数。

**修复建议**：使用跨 chunk 的行缓冲区，参考：
```rust
let mut line_buf = String::new();
while let Some(chunk) = stream.next().await {
    let chunk = chunk.map_err(...)?;
    let chunk_str = String::from_utf8_lossy(&chunk);
    line_buf.push_str(&chunk_str);

    // 按 \n\n 分割事件，但保留最后一个不完整行
    while let Some(pos) = line_buf.find("\n\n") {
        let event = line_buf[..pos].to_string();
        line_buf = line_buf[pos + 2..].to_string();
        // 处理 event 内的每一行
        for line in event.lines() {
            // ... 现有 line 处理逻辑
        }
    }
    // line_buf 保留未完成的事件/行，等待下一 chunk
}
```

> ⚠️ 此修复建议跨度较大，建议作为独立 patch 在 `stream_chat_turn` 和 `stream_deepseek` 中统一应用。

#### H2. 前端 `reasoning_delta` 事件已推送但未消费（正确性 / 功能完整性）

**文件与行号**：
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:625–631` — Rust 端发出 `ReasoningDelta`
- `apps/desktop/src/hooks/usePipeline.ts:289–291` — 前端消费端

**原因**：
`stream_chat_turn` 在第 626–630 行正常发出了 `MindStreamEvent::ReasoningDelta` 事件，但前端 `usePipeline.ts:289–291` 的处理逻辑是：
```typescript
case "reasoning_delta":
  // 思考过程单独累计，不进入正文
  break;
```
既没有展示给用户，也没有累积到任何变量中。

commit message 声称 "agent 推理过程实时经 mind-stream 推送前端"，但从用户视角看，思考过程既不在炼化日志面板中显示，也不在笔记预览区显示，完全不可见。**功能目标未完全达成。**

**风险**：中等。不影响笔记生成正确性，但浪费了计算和网络带宽（`reasoning_content` 可能长达数万 token），且对用户没有价值产出。

**修复建议**：二选一：
- **方案 A**：前端接入 reasoning 展示（例如在 LogPanel 中以可折叠区域展示），实现 commit 声称的功能
- **方案 B**：如果 reasoning 仅用于 dev 调试，则在 Rust 端用 `cfg!(debug_assertions)` 条件 emit，release 模式下不推送

---

### MEDIUM

#### M1. 流式传输连接无超时保护（稳定性）

**文件与行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:590–701`

**原因**：
`while let Some(chunk) = stream.next().await` 循环无超时机制。`reqwest` 的 `bytes_stream()` 在没有新数据到达时可能阻塞无限长时间（取决于底层 TCP 连接的 keep-alive 配置和服务器行为）。如果 DeepSeek 服务端连接挂死（不发送数据也不关闭连接），agent 将永久阻塞。

对比 `reqwest::Client` 的初始化（第 508 行）使用了默认配置，未设置 `timeout()`、`connect_timeout()` 或 `pool_idle_timeout()`。

**风险**：中等。通常 SSE 连接由服务端管理超时，但发生网络分区 / 代理故障 / 服务端 bug 时可能触发无限阻塞。

**修复建议**：
```patch
-    let client = Client::new();
+    let client = Client::builder()
+        .timeout(std::time::Duration::from_secs(300))
+        .connect_timeout(std::time::Duration::from_secs(30))
+        .build()
+        .map_err(|e| AppError::Ai { kind: "network".into(), message: e.to_string() })?;
```

#### M2. Tool calls delta 异常 index 值静默合并到 index 0（正确性）

**文件与行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:637`

**原因**：
```rust
let idx = tc_delta["index"].as_i64().unwrap_or(0);
```
如果某个 SSE chunk 中 `tool_calls[].index` 值为 `null`、非数字字符串，或 JSON 解析失败导致 `as_i64()` 返回 `None`，则默认回退为 `0`。此时，该 tool_call 的 arguments 会被错误地追加到 index 0 的条目，导致 tool call 参数混乱。

同时，如果模型返回的 index 为负数（异常情况），`i64` 允许负值但业务上不应有负索引。

**风险**：中等。依赖 DeepSeek API 行为，正常工作时不会触发。但一旦 API 升级或返回格式变更，可能产生隐蔽的参数拼接错误，最终导致工具执行失败但难以排查。

**修复建议**：
```patch
-let idx = tc_delta["index"].as_i64().unwrap_or(0);
+let Some(idx) = tc_delta["index"].as_i64() else {
+    log::warn!(target: "agent", "[llm] stream_chat_turn: tool_call delta 无 index，跳过");
+    continue;  // 跳过无 index 的异常 delta
+};
+// 防御：拒绝负 index
+if idx < 0 {
+    log::warn!(target: "agent", "[llm] stream_chat_turn: tool_call delta index={idx} 为负，跳过");
+    continue;
+}
```

#### M3. `chat_turn` 与 `stream_chat_turn` 严重代码重复（可维护性）

**文件与行号**：
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:307–489` — `chat_turn()`（死代码）
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:500–797` — `stream_chat_turn()`

**原因**：
两个函数共享约 90% 的逻辑：请求体构建（~80 行相同）、错误分类（~15 行相同）、token 使用解析（~20 行相同）、日志格式（~30 行相同）。差异仅在于：
1. `stream: true` vs `stream: false`
2. 响应解析方式：流式 SSE 解析 vs 非流式 JSON 解析

`chat_turn` 已被 `stream_chat_turn` 完全替代（codegraph_callers 确认无调用者），最终变为 ~200 行死代码。设计文档注明 `chat_turn` 保留为死代码待后续清理。

**风险**：中等。当需要修改模型路由、请求参数、token 解析等逻辑时，需要同时修改两份代码。如果只改了 `stream_chat_turn` 而忘记 `chat_turn`，不会影响运行（因为 `chat_turn` 已死），但会造成两个函数行为不一致的假象，增加后续维护者的困惑。

**修复建议**：
- **短期**：在 `chat_turn` 开头添加 `#[deprecated]` 标记，编译器会发出警告
- **中长期**：Plan 确认后删除 `chat_turn`；或将其重构为调用 `stream_chat_turn` 的一个 thin wrapper（只需在 `minijinja` 层面切换 `stream: false` 并解析 JSON 响应体）

#### M4. 每个 SSE chunk 触发多次 IPC 事件发射（性能）

**文件与行号**：
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:615–619` — `Delta` emit
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:625–631` — `ReasoningDelta` emit
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs:687–697` — `Usage` emit

**原因**：
SSE 流式输出中，每个 `data:` 行都会立即生成一次 `app_handle.emit()` 调用。在 DeepSeek API 的默认行为下，内容 delta 可能以 token 级别粒度的频率发送。这意味着：
- 每生成 1 个 token → 1 次 `mind-stream` Delta 事件 → 1 次 Tauri IPC 调用
- 如 thinking mode 启用，同时还有 ReasoningDelta 事件
- 一个完整的 agent turn（可能 4K–128K token）会产生数千次 IPC 调用

每次 `emit` 经过：Rust 序列化 → event loop → JS 反序列化 → React state 更新。高频事件会挤压 UI 线程，导致前端卡顿。

`stream_deepseek` 也存在同样问题（预存），但本次 commit 在 `stream_chat_turn` 中复制了该模式。

**风险**：中等。在生成大篇幅笔记时前端可能明显卡顿。实测需在 8K+ token 输出时验证。

**修复建议**：在 Rust 端对 Delta/ReasoningDelta 事件添加节流（如每 50ms 批量发射一次）：
```rust
// 简易 throttle：累积 delta 文本，定时或达到阈值后批量 emit
let mut delta_buf = String::new();
let mut last_emit = std::time::Instant::now();

// ... 在 delta["content"] 处理中:
delta_buf.push_str(content);
if last_emit.elapsed().as_millis() >= 50 {
    let _ = app_handle.emit("mind-stream", MindStreamEvent::Delta { delta: delta_buf.clone() });
    delta_buf.clear();
    last_emit = std::time::Instant::now();
}
// 循环结束后 flush 剩余
```

---

### LOW

#### L1. `String::from_utf8_lossy` 在每行上分配 String（性能）

**文件与行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:597`

**原因**：
```rust
let line = String::from_utf8_lossy(line);
```
在每次迭代（每个 SSE 行）中都调用 `from_utf8_lossy()`，如果输入是合法 UTF-8（绝大多数情况），这会产生不必要的 `Cow::Owned` 分配。对于整轮数万行的流式会话，累积分配开销不可忽略。

**修复建议**：先尝试 `std::str::from_utf8` 的快速路径：
```rust
let line = std::str::from_utf8(line).unwrap_or_else(|_| String::from_utf8_lossy(line).as_ref());
```

#### L2. `ReasoningEffort` match 缺少通配分支（可维护性）

**文件与行号**：`apps/desktop/src-tauri/src/commands/ai/deepseek.rs:527–529`

**原因**：
```rust
body["reasoning_effort"] = match effort {
    ReasoningEffort::High => serde_json::json!("high"),
    ReasoningEffort::Max => serde_json::json!("max"),
};
```
当前 `ReasoningEffort` 枚举只有 `High` 和 `Max` 两个变体，match 是穷尽的。但如果未来添加新变体（如 `Low`、`Medium`），编译器会在此处报错，这是好的。但 `stream_deepseek` 中的 `build_body` 函数（用户未展示但 codegraph 索引包含）也可能有类似 match 需要同步更新。当前无问题，仅标记供后续注意。

#### L3. `final_content` 在 Done 事件中被克隆（性能）

**文件与行号**：`apps/desktop/src-tauri/src/agent/runner.rs:414`

**原因**：
```rust
let _ = app.emit(
    "mind-stream",
    MindStreamEvent::Done {
        text: final_content.clone(),  // ← 克隆
```
`final_content` 可能包含完整笔记正文（可达 128K 字符）。此处在发送 Done 事件时 clone 了整个字符串。虽然这是最终一次操作，但对于大笔记不可忽略。

**修复建议**：在 `final_content` 不再被后续使用时，可改用 `std::mem::take` 或直接 move：
```patch
 let _ = app.emit(
     "mind-stream",
     MindStreamEvent::Done {
-        text: final_content.clone(),
+        text: final_content.clone(),  // 仍需 clone，因为后续 AgentResult 还要用
         finish_reason: Some("stop".into()),
     },
 );
```
（如果需要后续 `AgentResult { note_content: final_content, ... }` 也使用，则无法避免此 clone。可以在 Done 之后再构建 AgentResult 来减少一次 clone。）

---

## 各类别覆盖总结

| 类别 | 问题编号 | 最高级别 | 说明 |
|------|----------|----------|------|
| **正确性** | H1, H2, M2 | **High** | SSE 行截断导致丢帧；前端未消费 reasoning_delta；tool_calls index 异常合并 |
| **稳定性** | C1, M1 | **Critical** | 流式期间无法取消；无超时保护 |
| **性能** | M4, L1, L3 | **Medium** | 每 token 一次 IPC emit；from_utf8_lossy 分配；Done 时 clone 大字符串 |
| **安全性** | — | — | 本次变更未引入新的安全风险。log::debug 中的预览文本均为模型输出，不包含密钥。`api_key` 作为参数传入，不进入日志。 |
| **可维护性** | M3, L2 | **Medium** | `chat_turn` 死代码未清理；两函数大量重复 |

---

## 未发现问题说明

### 正确性 — content / reasoning_content 回退逻辑
`runner.rs:201–211` 的 thinking mode 下 `content` 为空时回退到 `reasoning_content` 的逻辑正确且完备。已考虑 `trim().is_empty()` 的边缘情况，并有 `log::warn!` 标记便于排查。**无问题**。

### 正确性 — Tool call arguments 增量累积
`deepseek.rs:635–661` 的 `tc_buffer` HashMap 按 `index` 分桶、累积 `arguments` 片段的算法正确。与 DeepSeek 官方流式 tool_calls 文档行为一致。**无逻辑问题**（M2 仅针对异常输入的防御性不足）。

### 安全性 — Key 泄漏
`api_key` 以 `&str` 传入，仅用于构造 `Authorization` header，不进入任何日志字符串。错误响应体不包含 Authorization header。**无密钥泄漏问题**。

### 安全性 — 错误回显脱敏
`runner.rs:342–343` 注释明确声明 `stderr` 已在 `python.rs` 源头通过 `redact_secrets` 脱敏，不含明文 api_key。日志中的工具调用参数（第 252 行 `tc.function.arguments`）可能包含用户本地路径但无密钥。**无泄漏风险**。

### 前后端契约兼容性
Rust `MindStreamEvent` 的 `#[serde(tag = "type")]` 序列化输出与前端 `api.ts:249` 的接口定义兼容（snake_case 字段）。`ReasoningDelta` 事件在前端类型联合中有定义，虽未消费但不会抛异常。**无契约断裂**。

---

## 审查结论

本次变更的核心目标（流式传输 + 日志可视化）已基本达成，`stream_chat_turn` 实现质量总体可用，`runner.rs` 的 reasoning_content 回退处理细致。

**必须在合入前修复**：
1. **C1** — 流式读取期间无法响应取消请求（加 `cancel` 参数 + 循环内检查，改动 < 10 行）
2. **H1** — SSE 行跨 chunk 截断风险（需行缓冲区，建议统一修复 `stream_chat_turn` 和 `stream_deepseek`）

**建议在下一迭代修复**：
3. **H2** — 前端接入 reasoning 展示或条件 emit
4. **M1** — 添加 `reqwest::Client` 超时配置
5. **M2** — 防御性处理 tool_calls index 异常值
6. **M3** — 删除或标记 `chat_turn` 为 deprecated

**可选改进**（低优先级）：
7. **M4** — Delta 事件节流
8. **L1** — `from_utf8` 快速路径

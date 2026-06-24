# Code Review: `e60595f` — feat: 改为流式传输，日志显示 AI 推理过程

> **审查日期**: 2026-06-24  
> **审查范围**: 3 文件，+514/−34 行  
> **审查方法**: codegraph 索引 + 全量 diff 逐行审查 + 影响面遍历（前端 `mind-stream` 消费端、`emit_progress` 事件链路、`ToolOutput::to_llm_text` 数据流）

---

## 变更摘要

| 文件 | 变更量 | 核心内容 |
|------|--------|---------|
| `apps/desktop/src-tauri/src/commands/ai/deepseek.rs` | +359/−4 | 新增 `stream_chat_turn`（~230 行流式 tool_calls SSE 解析）；为 `chat_turn` 增加工具调用日志 |
| `apps/desktop/src-tauri/src/agent/runner.rs` | +140/−30 | 切换为 `stream_chat_turn`；增加双层日志（dev 日志预览 + 用户面板完整输出）；`reasoning_content` 回退逻辑；终止轮 Done 事件重构 |
| `docs/设计文档/AI与模型/Agent 开发计划.md` | +49/−3 | 文档同步更新：Spike 流式实证、运行期调试增强记录 |

---

## 影响面分析（Codegraph 索引）

```
stream_chat_turn (deepseek.rs)
  ├─ 调用方: runner.rs::run() loop 第 159 行
  ├─ 上游: DeepSeek SSE API (POST /chat/completions, stream=true)
  ├─ 下游事件 (emit):
  │   ├─ "mind-stream" Delta ──→ 前端 usePipeline.ts streamAccumRef 累积
  │   ├─ "mind-stream" ReasoningDelta ──→ 前端 仅归档,不进正文
  │   └─ "mind-stream" Usage ──→ 前端 lastUsageRef 记录
  ├─ 返回: AgentTurnResult { message, content, tool_calls, reasoning_content, usage, finish_reason }
  └─ 注: 不自行 emit Done（由 runner.rs 在终止轮发出）

runner.rs::run()
  ├─ emit "mind-stream" Start ──→ 前端 reset aiStartRef / streamAccumRef
  ├─ loop { stream_chat_turn → emit_progress (双层日志) → tool dispatch → push messages }
  ├─ 终止轮: fallback reasoning_content → final_content
  └─ emit "mind-stream" Done ──→ 前端 以 Done.text 为权威正文
```

---

## 问题分级清单

### 🔴 Critical — 无

> 经完整审查，未发现会导致数据丢失、崩溃或安全漏洞的 Critical 级问题。`stream_chat_turn` 的 SSE 分片累积逻辑正确处理了 tool_calls delta 的 index 分桶与 arguments 拼接，`reasoning_content` 回退逻辑覆盖了 thinking mode 下 content 为空的边界情况。

---

### 🟠 High — 2 项

#### H1. SSE 流消费无超时保护，可能导致 agent loop 永久挂起

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: 585–650（`while let Some(chunk) = stream.next().await` 循环）
- **分类**: 稳定性
- **原因**: `stream_chat_turn` 的流消费循环没有任何超时机制。若 DeepSeek 服务端在发送部分数据后 TCP 连接挂起（不发送 FIN，也不发新数据），`stream.next().await` 会永远阻塞。`chat_turn` 的非流式版本受益于 reqwest 的默认超时（30s），但流式 body 的 `bytes_stream()` 不受此保护。
- **风险**: agent loop 单轮永久挂起 → 整个炼化任务卡死，无任何错误提示。用户只能强制关闭应用。
- **修复建议**:

```rust
// deepseek.rs stream_chat_turn() 中，将 stream 消费包装超时
use tokio::time::{timeout, Duration};

const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

// 替换：while let Some(chunk) = stream.next().await {
loop {
    let chunk = match timeout(STREAM_IDLE_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(chunk))) => chunk,
        Ok(Some(Err(e))) => return Err(AppError::Ai {
            kind: "network".into(),
            message: format!("流读取失败：{e}"),
        }),
        Ok(None) => break, // 流正常结束
        Err(_elapsed) => {
            log::warn!(target: "agent", "[llm] stream_idle_timeout after {}s", STREAM_IDLE_TIMEOUT.as_secs());
            return Err(AppError::Ai {
                kind: "timeout".into(),
                message: format!("SSE 流 {} 秒无新数据，已超时", STREAM_IDLE_TIMEOUT.as_secs()),
            });
        }
    };
    // ... 原有解析逻辑 ...
}
```

> ⚠️ 注意：超时应该是"距上次收到数据"的 idle timeout，而非"自请求开始"的总超时。需要每次收到 chunk 后 `reset` 计时器。上述片段为简化版，完整实现建议用 `tokio::time::Timeout` 在每次迭代重建或在外部用 `select!` + `tokio::time::interval`。

---

#### H2. 流异常中断时前端收到不完整事件序列，无 Error 事件收尾

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs` + `apps/desktop/src-tauri/src/agent/runner.rs`
- **行号**: `deepseek.rs` L585–735（流消费 + 无 Error emit）；`runner.rs` L159（调用方，错误向上传播）
- **分类**: 稳定性 / 正确性
- **原因**: `stream_chat_turn` 如果在流读取中途失败（网络断开、JSON 解析失败、H1 超时等），已通过 `mind-stream` emit 了若干 `Delta`/`ReasoningDelta`/`Usage` 事件。但函数在错误时直接 `return Err(...)`，**不 emit `MindStreamEvent::Error`**。`runner.rs` 收到错误后也直接 `?` 传播，同样不 emit Error 事件。
- **风险**: 前端 `usePipeline.ts` 的 `streamAccumRef` 已累积部分内容，但永远等不到 `done` 或 `error` 事件收尾。UI 可能停留在 "AI 生成中" 的中间状态，streaming text 悬空。
- **修复建议**:

```rust
// 方案 A: 在 stream_chat_turn 中 catch 错误并 emit Error
// 在 while 循环后、Err 返回前增加：
if let Err(ref e) = result {
    let _ = app_handle.emit("mind-stream", MindStreamEvent::Error {
        code: "stream_error".into(),
        message: e.to_string(),
        retryable: true,
    });
}

// 方案 B: 在 runner.rs 调用处 catch 并 emit（更上层，覆盖更全）
// runner.rs L159 附近：
let turn = match stream_chat_turn(...).await {
    Ok(t) => t,
    Err(e) => {
        let _ = app.emit("mind-stream", MindStreamEvent::Error {
            code: "agent_error".into(),
            message: format!("agent 调用失败：{e}"),
            retryable: matches!(e, AppError::Ai { kind, .. } if kind != "authentication"),
        });
        return Err(e);
    }
};
```

推荐 **方案 B**（在 runner.rs 统一 emit Error），这样同时覆盖 `stream_chat_turn` 失败和其他 runner 中的异常。

---

### 🟡 Medium — 5 项

#### M1. `tc_buffer` 无界增长风险

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: L593（`let mut tc_buffer: HashMap<i64, (String, String, String)> = HashMap::new();`）
- **分类**: 稳定性
- **原因**: 工具调用参数分片缓冲没有上限。虽然 DeepSeek 服务端不会恶意发送无限碎片，但若服务端 bug 导致不断发送新的 `tool_calls` delta（递增 index），内存会持续增长。`HashMap` 的 key 是 `i64`，理论上有 $2^{63}$ 个槽位。
- **风险**: 极端场景下 OOM。实际概率低（需要上游异常），但作为防御性编程应加限制。
- **修复建议**:

```rust
const MAX_TOOL_CALLS: usize = 64; // 单轮最多 64 个工具调用

// 在新建 entry 前：
if tc_buffer.len() >= MAX_TOOL_CALLS {
    log::warn!(target: "agent", "[llm] tool_calls 数量超限 {MAX_TOOL_CALLS}，丢弃");
    continue;
}
tc_buffer.insert(idx, (id, name, args));
```

---

#### M2. Tool call delta 首个分片缺失 `id` 时构造无效 ToolCall

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: L618–631（新建 tc_buffer entry）, L672–685（filter_map 构建 tool_calls）
- **分类**: 正确性
- **原因**: 代码假设 `id` 和 `name` 必然在首个分片中出现。若 DeepSeek 的服务端行为变更（首个分片只有 `index` + `function.arguments` 而 `id`/`name` 在后续分片补发），则 `tc_buffer` entry 的 `id` 会是空字符串 `""`。后续 `filter_map` 只过滤了 `name.is_empty()`，未过滤 `id.is_empty()`。空 `id` 的 `ToolCall` 会被用于构造 `tool_call_id` → 后续 tool result 消息的 `tool_call_id` 为空 → OpenAI API 拒收（400）。
- **风险**: 依赖 DeepSeek 服务端行为假设。若服务端变更，会导致后续轮 API 调用失败。概率低但影响大。
- **修复建议**:

```rust
// 在 filter_map 中同时检查 id:
.filter_map(|idx| {
    let (id, name, arguments) = tc_buffer.remove(&idx)?;
    if name.is_empty() || id.is_empty() {
        log::warn!(target: "agent", "[llm] tool_call idx={idx} 缺失 id/name，丢弃");
        return None;
    }
    Some(ToolCall { id, kind: Some("function".into()), function: ToolCallFunction { name, arguments } })
})
```

---

#### M3. 终止轮将完整笔记正文通过 `emit_progress` 重复推送

- **文件**: `apps/desktop/src-tauri/src/agent/runner.rs`
- **行号**: L163–179（`if let Some(ref text) = turn.content` 块）
- **分类**: 性能 / 可维护性
- **原因**: 每轮（包括终止轮）都将 `turn.content` 的**完整文本**通过 `emit_progress(detail=Some(text))` 序列化到 Tauri 事件。对于终止轮，`turn.content` 就是最终笔记全文（可能 10K–100K+ 字符）。这条数据同时通过两个通道发送：
  1. `mind-stream` Delta/Done 事件（正确路径，前端笔记预览区使用）
  2. `pipeline-progress` 事件的 `detail` 字段（冗余，用户日志面板使用）

  日志面板不需要展示完整笔记正文（文档自己也说："用户面板只提示总结中，不重复展示笔记正文"——L228），但 `reasoning` emit（L163–179）在终止判断（L196）之前执行，无法区分"推理文本"和"最终笔记"。
- **风险**: 大笔记（>50K 字符）序列化为 JSON 事件可能导致 UI 线程短暂卡顿；重复序列化浪费 CPU。
- **修复建议**:

```rust
// 在 emit_progress 前判断是否为终止轮（无 tool_calls），终止轮不推 detail
if let Some(ref text) = turn.content {
    if !text.trim().is_empty() {
        log::debug!(...); // dev 日志保留
        // 只在非终止轮推用户面板（终止轮正文由 mind-stream Done 承载）
        if !turn.tool_calls.is_empty() {
            emit_progress(app, "agent", "💭 agent 分析", ..., Some(text.as_str()));
        }
    }
}
```

或截断为合理长度：

```rust
let panel_preview: String = text.chars().take(2000).collect();
emit_progress(app, "agent", "💭 agent 分析", ..., Some(&panel_preview));
```

---

#### M4. `chat_turn` 与 `stream_chat_turn` 日志代码重复 ~40 行

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: `chat_turn` L446–483 vs `stream_chat_turn` L757–795（content_preview/reasoning_preview 计算 + log::debug! 格式完全一致）
- **分类**: 可维护性
- **原因**: 两个函数末尾的完成日志逻辑完全重复。后续修改日志格式需要改两处。
- **风险**: 代码腐化——改一处忘另一处，日志输出不一致，排查困难。
- **修复建议**: 提取为私有函数 `fn log_turn_done(phase: &str, ...)` 或直接用宏。注意 `chat_turn` 已被标注为死代码（待清理），如果确认移除则此项自动消除。

---

#### M5. 工具执行结果 (`to_llm_text()`) 消费后 `ToolOutput` 变为部分无效状态

- **文件**: `apps/desktop/src-tauri/src/agent/runner.rs`
- **行号**: L312（`let text = out.to_llm_text();`）
- **分类**: 可维护性
- **原因**: 注释明确标注 "提前捕获 artifact 摘要（`to_llm_text()` 会消费 out）"，但实际上 `to_llm_text()` 只是 `&self` 引用，并不消费 `out`。`out` 在后续 `emit_progress(..., Some(&text))` 中仍有效。注释与实现不一致，且 `text` 是 `String`（owned），`out` 作为 `&self` 引用并未被 move。
- **风险**: 误导后续维护者——如果将来 `to_llm_text()` 改为 `self` 消费，`artifact_summary` 的提前捕获逻辑是对的，但当前注释描述的实现与 Rust 所有权语义矛盾。代码实际行为正确，但注释可能让维护者误以为有什么 magic。
- **修复建议**: 修正注释：

```rust
// 提前捕获 artifact 摘要（与 to_llm_text() 顺序无关，二者都是 &self 不可变借用）
```

---

### 🟢 Low — 4 项

#### L1. `allow(dead_code)` 标记不再准确

- **文件**: `apps/desktop/src-tauri/src/commands/ai/types.rs`
- **行号**: L229（`#[allow(dead_code)] pub reasoning_content: Option<String>`）
- **分类**: 可维护性
- **原因**: 本次变更后，`reasoning_content` 在 `stream_chat_turn` 中被赋值、在 `runner.rs` 中被用于 fallback 逻辑（L203–210）。不再是 dead code。`#[allow(dead_code)]` 标记应移除。
- **修复建议**: 删除 L229 的 `#[allow(dead_code)]`。

---

#### L2. `full_text` 和 `reasoning_text` 不必要的 clone

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: L743–751
- **分类**: 性能
- **原因**:

```rust
let content: Option<String> = if full_text.is_empty() { None } else { Some(full_text.clone()) };
let reasoning_content: Option<String> = if reasoning_text.is_empty() { None } else { Some(reasoning_text) };
```

`full_text` 在此之后不再使用（Done 事件已在 `stream_chat_turn` 中不 emit，由 runner 发），可直接 move。`reasoning_text` 同理。

- **修复建议**:

```rust
let content: Option<String> = if full_text.is_empty() { None } else { Some(full_text) };
let reasoning_content: Option<String> = if reasoning_text.is_empty() { None } else { Some(reasoning_text) };
```

---

#### L3. SSE 行解析仅支持 `\n`，不兼容 `\r\n`

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: L601（`chunk.split(|&b| b == b'\n')`）
- **分类**: 正确性
- **原因**: SSE 规范（text/event-stream）允许 `\r\n` 作为行分隔符。当前实现按单个 `\n` 字节分割，若上游发送 `\r\n`，`\r` 会残留在行尾，导致 `strip_prefix("data: ")` 失败（因为行不是以 `data:` 开头，而是 `data: \r` 或需要 trim）。
- **风险**: DeepSeek 服务端当前使用 `\n`，实际不会触发。但若服务端变更或通过代理（某些代理会转换换行符），会导致流解析失败。
- **修复建议**: 使用 `std::str::lines()` 或先按 `\n` split 再 trim：

```rust
for line_bytes in chunk.split(|&b| b == b'\n') {
    let line = String::from_utf8_lossy(line_bytes);
    let trimmed = line.trim(); // 同时处理 \r 和空格
    // ...
}
```

当前代码已有 `let trimmed = line.trim();`，但 `trim()` 在 `strip_prefix` 之前调用，而 `strip_prefix` 作用在原始 `trimmed`（已去 `\r`）上。实际上当前对于 `data: value\r` 行可以正确工作，因为 `trim()` 会去掉 `\r`。所以 **此项实际不会触发 bug**，降为文档建议。

---

#### L4. `stream_chat_turn` 缺少请求阶段的 prompt_chars 日志中对 tool_calls message 的 content 计数

- **文件**: `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`
- **行号**: L530–540（prompt_chars 计算）
- **分类**: 可维护性（日志准确性）
- **原因**: `prompt_chars` 计算只统计 `messages[i].content` 的字符数，但 tool result 消息的 `content` 是字符串，assistant 消息含 `tool_calls` 时 `content` 可能为 null。当前代码用 `get("content").and_then(|v| v.as_str())` 处理，null 返回 0。这是正确的，但会低估实际 prompt 大小（tool_calls JSON 也占用 token）。不影响功能，仅影响日志准确性。
- **修复建议**: 将估算改为 `serde_json::to_string(&m).unwrap_or_default().chars().count()`，或在日志中注明"仅 content 文本"。

---

## 检查范围与未发现原因

### 已检查维度

| 维度 | 覆盖 | 说明 |
|------|------|------|
| **正确性** | ✅ | SSE 分片累积、tool_calls index 分桶、reasoning_content 回退、content null 归一化、finish_reason 传递 |
| **稳定性** | ✅ | 超时、流中断、错误传播、frontend 状态一致性、tc_buffer 边界 |
| **性能** | ✅ | 大字符串序列化、clone vs move、事件推送频率 |
| **安全** | ✅ | 密钥脱敏（redact_secrets）、日志不泄露 API Key、emit 内容审查 |
| **可维护性** | ✅ | 死代码、重复代码、注释准确性、`allow` 标记 |

### 未发现更高风险问题的原因

1. **SSE 工具调用分片累积逻辑** (`index` 分桶 + `arguments` 拼接) 经过运行期验证（文档标注"已通过真实 key 运行验证"），核心协议实现正确。
2. **`reasoning_content` 回退逻辑**在 runner.rs 的终止轮正确处理了 thinking mode 下 content 为空的情况，有 `log::warn!` 标记便于排查。
3. **前端事件契约** (`Delta → streamAccumRef` 累积 + `Done.text` 权威覆盖) 经代码走读确认匹配。
4. **密钥安全**：`redact_secrets` 在 python.rs 源头脱敏，`emit_progress` 不会泄露密钥。`Authorization` header 仅出现在 `format!("Bearer {api_key}")`，不在任何日志/事件中。
5. **取消机制**：`cancel: AtomicBool` 每轮顶部检查（`Ordering::Relaxed`），`stream_chat_turn` 内部不支持取消（已知限制，设计文档标 P1 延后）。

### 未覆盖维度说明

- **并发安全**: 当前为单任务串行执行（一个 `run()` 一个 agent loop），无并发竞态。若未来支持并行任务，`app.emit()` 的多事件通道需要关注。
- **流式 tool_calls 的分片 JSON 完整性**: 未对累积后的 `arguments` 字符串做 JSON 格式校验（`runner.rs` 中 `serde_json::from_str` 会做，失败则回喂诊断）。这是正确的分层设计。

---

## 总结

| 级别 | 数量 | 关键问题 |
|------|------|---------|
| Critical | 0 | — |
| High | 2 | SSE 无超时（H1）、流中断后缺 Error 事件（H2） |
| Medium | 5 | tc_buffer 无界（M1）、id 缺失（M2）、大笔记冗余推送（M3）、日志重复（M4）、注释不准确（M5） |
| Low | 4 | dead_code 标记、unnecessary clone、SSE 行尾兼容、日志精度 |

**建议优先修复 H1（SSE 超时）和 H2（Error 事件），二者在弱网/服务端异常场景下直接影响用户体验。** 其余 Medium/Low 项可在后续迭代中逐步清理。

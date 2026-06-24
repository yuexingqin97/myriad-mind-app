# codegraph 索引与变更影响面证据

> 提交：`e60595f5211c0b61c340e74f16e6ee731f56d0ff`（feat: 改为流式传输，日志显示ai 推理过程）
> 审查时间：2026-06-24
> 工具：codegraph CLI `init` + MCP（projectPath 指向仓库根）

## 1. 索引建立（硬性要求 #1）

`codegraph init .` 完成，扫描 86 文件，1,134 nodes / 2,523 edges。

```
$ codegraph init .
◆  Indexed 86 files
●  1,134 nodes, 2,523 edges in 590ms
└  Done
```

仓库为 `staticlib/cdylib/rlib` 混合 crate（`apps/desktop/src-tauri/Cargo.toml`）→ `pub fn chat_turn` 属公开 API 表面，**不会**触发 `dead_code` 编译告警，但实际无任何调用者（见下）。

## 2. 变更涉及符号与影响面

提交改动 2 个 Rust 文件 + 1 个设计文档：
- `apps/desktop/src-tauri/src/commands/ai/deepseek.rs`（+359 行：新增 `stream_chat_turn` ~297 行；`stream_deepseek`/`chat_turn` 日志增强）
- `apps/desktop/src-tauri/src/agent/runner.rs`（+140 行：loop 内 `chat_turn` → `stream_chat_turn`；新增推理/工具双层日志；移除结尾分块流式、改发 Done）

### `chat_turn`（退役函数）

```
codegraph_callers("chat_turn") → No callers found
```
→ 本提交后成为**死代码**（仅 `stream_chat_turn` 文档注释里提了一句）。设计文档自承：「`chat_turn` 保留为死代码待后续清理」。

### `stream_chat_turn`（本提交新增）

```
codegraph_callers("stream_chat_turn")
  → run (function) - apps/desktop/src-tauri/src/agent/runner.rs:60   // 唯一调用点 runner.rs:159
codegraph_impact("stream_chat_turn")
  → deepseek.rs:stream_chat_turn(500)
  → runner.rs:run(60)
```
→ 影响面收敛在 agent 主循环 `run`，无其它调用者，无测试覆盖（codegraph 标注 ⚠️ no covering tests）。

### `run`（agent 入口）

```
codegraph_callers("run") → lib.rs:40（execute_pipeline 调度）
```
→ 完整调用链：`execute_pipeline` → `agent::run` → `stream_chat_turn`（每轮）。

## 3. 前端契约（验证流式语义变化）

- `emit_progress`（`commands/pipeline.rs:265`）签名：`detail: Option<&str>` → 发 `pipeline-progress` 事件。
- 前端 `usePipeline.ts:259-322` 监听 `mind-stream`：`streamAccumRef` 仅在 `start`(264) / `done`(315) 时清空，`delta`(269-288) 持续累加并每 2000 字 push 一条 `output` 日志；`done`(299-321) 优先取后端权威 `event.text`。
- 前端 `api.ts:249-264` `MindStreamEvent` 类型：`done.text` 为权威正文。
- **关键**：设计文档声明「**前端无改动**…mind-stream Delta/Done 事件保持不变」——但事件*形状*不变，*语义*已变（旧版仅终止轮分块发 Delta；新版每轮都发 Delta）。

## 4. 安全核查点（密钥流向）

- `query_ai_douyin` 的 api_key 由 handler 内部 `read_config_value("ai_douyin_api_key")` 读取（`acquire.rs:610`），**不经过** `tc.function.arguments` → `emit_progress(detail=Some(&tc.function.arguments))`（runner.rs:252）不泄露密钥。
- `ToolOutput::to_llm_text()`（`tools/mod.rs:248-263`）只含 summary + artifact 引用清单，不含全文。
- 错误回喂 stderr 经 `python.rs::redact_secrets` 源头脱敏（runner.rs:342-343 注释）。

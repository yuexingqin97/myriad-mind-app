export const meta = {
  name: 'logging-scaffold',
  description: '落地日志与调试脚手架：勘察现状 → 核心接入 tauri-plugin-log → 埋点+前端 → 对抗审查（敏感信息/双通道/配置/编译）→ 修复',
  phases: [
    { title: '勘察', detail: 'Rust 日志现状 + 前端设置页现状（并行只读）' },
    { title: '核心接入', detail: 'Cargo/main/lib 三 Target + set_log_level/open_log_dir 命令' },
    { title: '埋点+前端', detail: 'Rust 关键模块埋点 + 前端设置页（并行）' },
    { title: '对抗审查', detail: '敏感信息/双通道/配置符合度/编译（4 维并行）' },
    { title: '修复', detail: '修复审查发现的问题' },
  ],
}

const BG = `项目：大衍决（D:/Project/myriad-mind-app），Tauri 2 + React 19 桌面 App。
任务：落地「日志与调试脚手架」。
设计文档（必读）：docs/设计文档/工程化/日志与调试脚手架设计.md
参考实现（tauri-plugin-log 范本）：D:/Project/Learn/cc-switch/src-tauri/src/lib.rs:303-338
编码约定：中文注释；Rust 遵现有风格（thiserror 统一错误、commands/ 模块）；不破坏现有业务逻辑。`

const SURVEY_SCHEMA = {
  type: 'object',
  properties: {
    area: { type: 'string' },
    findings: { type: 'array', items: { type: 'object', properties: { file: { type: 'string' }, line: { type: 'string' }, current: { type: 'string' }, action: { type: 'string' } }, required: ['file', 'action'] } },
    summary: { type: 'string' },
  },
  required: ['area', 'findings', 'summary'],
}
const IMPL_SCHEMA = {
  type: 'object',
  properties: {
    changes: { type: 'array', items: { type: 'object', properties: { file: { type: 'string' }, what: { type: 'string' } }, required: ['file', 'what'] } },
    selfCheck: { type: 'object', properties: { cargoCheck: { type: 'string' }, typecheck: { type: 'string' }, notes: { type: 'string' } } },
    concerns: { type: 'array', items: { type: 'string' } },
  },
  required: ['changes', 'selfCheck'],
}
const AUDIT_SCHEMA = {
  type: 'object',
  properties: {
    dimension: { type: 'string' },
    issues: { type: 'array', items: { type: 'object', properties: { severity: { type: 'string' }, file: { type: 'string' }, line: { type: 'string' }, problem: { type: 'string' }, suggestion: { type: 'string' } }, required: ['severity', 'problem'] } },
    summary: { type: 'string' },
  },
  required: ['dimension', 'issues', 'summary'],
}

// ---------- Phase 1: 勘察（并行只读）----------
phase('勘察')
const [rustSurvey, feSurvey] = await parallel([
  () => agent(`${BG}

你是 Rust 日志现状勘察员。只读，不改任何文件。
查清 apps/desktop/src-tauri/ 的日志现状，给出接入锚点：
1. main.rs 里 simple_logging 的用法（精确行号 + 上下文）
2. Cargo.toml 里 simple-logging / log 的依赖行
3. lib.rs run() 的结构：Tauri Builder 在哪、plugin 怎么注册（找 .plugin( 调用点）、generate_handler! 在哪
4. grep 全 src-tauri/src 的 log:: 调用（按文件汇总数量），重点标注 agent 调试关键模块：pipeline.rs / engine.rs / deepseek.rs / python.rs 现有 log:: 用法
5. 现有「打开目录」类命令（如 open_cache_dir）怎么用 tauri-plugin-opener 的，给精确锚点
返回结构化清单（findings 每条：file/line/current/action）。`, { label: 'survey:rust', phase: '勘察', schema: SURVEY_SCHEMA }),

  () => agent(`${BG}

你是前端现状勘察员。只读，不改任何文件。
查清 apps/desktop/src/ 的现状，给出前端改动锚点：
1. components/settings/SettingsView.tsx 结构：在哪加「日志级别」UI、现有设置项的组织方式
2. 现有「打开目录」按钮（open_cache_dir）在前端怎么调（api.ts 封装 + 组件用法）
3. api.ts 的 Tauri 命令封装模式（invoke 怎么包，有没有 isTauri 降级）
4. 配置持久化模式（如 useConfig 怎么存 locale 等，日志级别能否复用）
返回结构化清单。`, { label: 'survey:frontend', phase: '勘察', schema: SURVEY_SCHEMA }),
])

// ---------- Phase 2: 核心接入 ----------
phase('核心接入')
const coreImpl = await agent(`${BG}

【勘察结果-Rust】
${JSON.stringify(rustSurvey)}

【勘察结果-前端】
${JSON.stringify(feSurvey)}

你是核心接入实现者。读设计文档 §四/§八，执行核心接入（改文件）：
1. apps/desktop/src-tauri/Cargo.toml：加 tauri-plugin-log = "2"，删 simple-logging 依赖
2. apps/desktop/src-tauri/src/main.rs：删 simple_logging::log_to_file(...) 及相关 log_path 逻辑
3. apps/desktop/src-tauri/src/lib.rs run()：注册 tauri_plugin_log 插件，配置：
   - targets 三项：Stdout + Folder{ path: ~/.myriad-mind-app/logs/, file_name: "myriad-mind" } + Webview
   - rotation_strategy: RotationStrategy::KeepSome(2)
   - max_file_size: 50MB（50*1024*1024）
   - level: if cfg!(debug_assertions) { Trace } else { Info }
   - timezone_strategy: TimezoneStrategy::UseLocal
   - 参考 cc-switch lib.rs:303-338 的写法（注意 cc-switch 用 app.handle().plugin() 在 setup 里；你按本项目 lib.rs 现有结构选最自然的方式：能在 Builder 链里 .plugin() 就用，需要 app handle 的放 setup）
4. 加两个 #[tauri::command]：
   - set_log_level(level: String) -> Result<(), AppError>：校验 level ∈ {trace,debug,info,warn}，调 log::set_max_level(parse_level_filter(&level))；注册到 generate_handler!
   - open_log_dir(app: AppHandle) -> Result<(), AppError>：打开 ~/.myriad-mind-app/logs/ 目录（先 create_dir_all，再用 tauri-plugin-opener，参考现有 open_cache_dir）
5. tauri.conf.json 的 allowlist/capabilities：若 Webview target 或新命令需要权限，按 Tauri 2 capabilities 机制配置（参考 cc-switch）

改完跑 cargo check 自检（Bash，timeout 300000=5分钟，超时就报告 timeout 不阻塞）。
红线：日志绝不记录 API key / Authorization。返回改动清单 + selfCheck。`, { label: 'impl:core', phase: '核心接入', schema: IMPL_SCHEMA })

// ---------- Phase 3: 埋点 + 前端（并行）----------
phase('埋点+前端')
const [instrument, feImpl] = await parallel([
  () => agent(`${BG}

【核心接入结果】
${JSON.stringify(coreImpl)}

你是 Rust 埋点实现者。读设计文档 §五（Agent 调试埋点），给关键模块补结构化日志（只加 log::debug!/trace!/warn!，不改业务逻辑）。格式约定 [模块] key=value：
1. pipeline.rs：每个 emit_progress 旁加 log::debug!('[pipeline] step=X status=Y percent=N')
2. engine.rs：run_mind_task 的模型路由决策（选 Pro/Flash + reason）、read_deepseek_key 成功/失败（只记"找到/未找到"，绝不记 key 值）
3. deepseek.rs：每次 LLM 请求（model + prompt 摘要前 200 字 + 预估 token）、响应（finish_reason + usage + 耗时ms）、reasoning_content 分流
4. python.rs：每次子进程调度（脚本名 + exit_code + 耗时ms），不记 stdout 全文

红线（重要）：绝不 log API key / Authorization header / 完整 prompt 原文。prompt 只记摘要+长度。
改完跑 cargo check 自检（timeout 300000）。返回改动清单。`, { label: 'impl:instrument', phase: '埋点+前端', schema: IMPL_SCHEMA }),

  () => agent(`${BG}

【核心接入结果】
${JSON.stringify(coreImpl)}

【前端勘察】
${JSON.stringify(feSurvey)}

你是前端实现者。读设计文档 §六，改前端：
1. apps/desktop/src/api.ts：加 setLogLevel(level: 'trace'|'debug'|'info'|'warn') 和 openLogDir() 命令封装（invoke，套现有 isTauri 降级模式）
2. apps/desktop/src/components/settings/SettingsView.tsx：加「日志级别」下拉（4 档）+ 「打开日志目录」按钮；级别切换调 api.setLogLevel；中文 UI 文案（如「日志级别」「打开日志目录」）
3. 级别持久化：调 setLogLevel 即时生效即可（运行时 log::set_max_level），是否存 config 可选——若存，复用 useConfig 模式

不改其他页面。改完跑 pnpm typecheck（在 apps/desktop，timeout 120000）。返回改动清单。`, { label: 'impl:frontend', phase: '埋点+前端', schema: IMPL_SCHEMA }),
])

// ---------- Phase 4: 对抗审查（4 维并行）----------
phase('对抗审查')
const audits = await parallel([
  () => agent(`${BG}

【对抗审查：敏感信息红线】你是安全审查员。读所有改动的 Rust 代码（重点 engine.rs/deepseek.rs/config.rs/python.rs 的 log:: 调用）+ 现有日志相关代码。
对抗性查找：日志里有没有泄漏 API key / Authorization header / 完整 prompt 原文 / deepseek_api_key / ai_douyin_api_key / cookie / token。
任何 log:: 的参数只要含上述敏感信息就报 issue（severity=blocker）。也检查 prompt 摘要是否真的截断（前 200 字）而非全文。
返回 issues 清单。`, { label: 'audit:secrets', phase: '对抗审查', schema: AUDIT_SCHEMA }),

  () => agent(`${BG}

【对抗审查：双通道不混淆】你是架构一致性审查员。读设计文档 §4.3（双通道）。
查找：① 有没有人用 log::info! 往用户 LogPanel 塞内容（LogPanel 应只由 pipeline-progress 事件驱动）② pipeline-progress 事件与 log 宏职责是否清晰（用户日志=精选事件，开发者日志=log 宏全量）③ 有没有把本该给开发者看的技术细节暴露进 pipeline-progress（用户可见）
返回 issues。`, { label: 'audit:channels', phase: '对抗审查', schema: AUDIT_SCHEMA }),

  () => agent(`${BG}

【对抗审查：配置符合度】你是配置审查员。对照设计文档 §四，检查 lib.rs 的 tauri_plugin_log 实现：
① 三 Target 齐全吗（Stdout + Folder + Webview）？缺 Webview 报 issue（这是比 cc-switch 进一步的关键点）
② 轮转 KeepSome(2) + max_file_size 50MB 对吗
③ 分级对吗（dev Trace / release Info，用 cfg!(debug_assertions)）
④ 日志目录是 ~/.myriad-mind-app/logs/ 吗（不是旧的 app.log）
⑤ set_log_level / open_log_dir 命令注册到 generate_handler! 了吗
⑥ simple_logging 和旧 app.log 逻辑是否清理干净
返回 issues（与设计的每处偏差）。`, { label: 'audit:config', phase: '对抗审查', schema: AUDIT_SCHEMA }),

  () => agent(`${BG}

【对抗审查：编译验证】你是构建验证员。实跑编译：
1. cd apps/desktop/src-tauri && cargo check（timeout 300000=5分钟，超时报告 timeout）
2. cd apps/desktop && pnpm typecheck（timeout 120000）
3. 若有 pnpm -F @myriad-mind/core typecheck / @myriad-mind/ui typecheck 也跑（core/ui 被前端依赖）
报告所有编译/类型错误（file:line:error）为 issues（severity=blocker）。编译通过则 issues 空、summary 写 PASS。`, { label: 'audit:build', phase: '对抗审查', schema: AUDIT_SCHEMA }),
])

const issues = audits.filter(Boolean).flatMap(a => (a.issues || []).map(i => ({ ...i, dimension: a.dimension })))
log(`审查发现 ${issues.length} 个问题`)

// ---------- Phase 5: 修复 ----------
phase('修复')
let fix = null
if (issues.length === 0) {
  log('无问题，跳过修复')
} else {
  fix = await agent(`${BG}

【审查发现的问题】
${JSON.stringify(issues)}

你是修复者。逐一修复上述问题（blocker 级优先，尤其是编译错误和敏感信息泄漏）。
修复后跑 cargo check + pnpm typecheck 自检（各自 timeout 300000/120000）。
返回改动清单 + selfCheck + 仍未解决的问题（concerns）。`, { label: 'fix', phase: '修复', schema: IMPL_SCHEMA })
}

return {
  survey: { rust: rustSurvey?.summary, frontend: feSurvey?.summary },
  coreImpl: coreImpl?.changes,
  instrument: instrument?.changes,
  frontend: feImpl?.changes,
  auditIssues: issues,
  fix: fix ? { changes: fix.changes, remaining: fix.concerns } : 'no-issues',
}
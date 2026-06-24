# 追问与诊断日志 — Python → Rust 迁移（第1批）

> 日期: 2026-06-24  
> 关联: `exports/task2/codegraph-evidence.md`

---

## Q1: 前端构建报错

**用户**: 测试编译报错 `Cannot find module '@myriad-mind/ui'` / `'@myriad-mind/core'`，什么原因？

**诊断过程**:
1. 检查 `pnpm-workspace.yaml` — workspace 配置正确
2. 检查 `packages/core/package.json` — `"main": "./dist/index.js"`, `"types": "./dist/index.d.ts"`
3. 检查 `packages/ui/package.json` — 同上
4. 检查 `packages/core/dist/` 和 `packages/ui/dist/` — **目录不存在**

**根因**: monorepo 首次构建，workspace 依赖包未编译，`dist/` 目录缺失导致 TypeScript 无法解析 `@myriad-mind/*`。

**修复**:
```powershell
pnpm --filter @myriad-mind/core build   # tsc → dist/
pnpm --filter @myriad-mind/ui build     # tsc → dist/
pnpm run build                          # tsc && vite build → ✅ 76 modules
```

**结论**: 非迁移 bug，monorepo 正常构建流程。

---

## Q2: 视频下载端到端测试失败

**用户**: 炼化 B站视频失败，分析日志 `C:\Users\zhangxiaoyue\.myriad-mind-app\logs\myriad-mind.log`

**诊断过程**:
1. 搜索日志关键词 `error|fail|download_video|douyin|phase=failed`
2. 还原完整失败链路:

```
13:41:46  query_ai_douyin (Rust reqwest ✅)
           → AI Douyin API HTTP 400: "搜索前请完成人机验证"
           → 非迁移 bug，上游 captcha 限制

13:42:08  download_video 工具
           → resolve_via_ai_douyin ✅ 解析出下载候选 URL
           → download_video_candidates.py ❌ exit_code=1 (yt-dlp 下载候选失败)
           → yt-dlp 裸跑 ❌ HTTP 412 (B站反爬)
           → yt-dlp --cookies-from-browser edge ❌
             "Could not copy Chrome cookie database" (Chrome 运行中)

13:43:54  Agent 终止，笔记降级生成（仅元数据）
```

**责任归属**:

| 组件 | 归属 | 状态 |
|------|------|------|
| `list_ai_douyin_tasks` (Rust) | ✅ 本次迁移 | 正常，上游 captcha |
| `extract_keyframes` (Rust) | ✅ 本次迁移 | 未触发（前置步骤失败） |
| `download_video_candidates.py` | ⚠️ 未迁移 | yt-dlp 候选下载失败 |
| yt-dlp B站 cookies | ❌ 环境 | Chrome 锁库 + B站反爬 |

**结论**: 与本次迁移无关。两个已迁脚本均未直接参与失败链路（`list_ai_douyin_tasks` 工作正常；`extract_keyframes` 因视频未下载而未被调用）。

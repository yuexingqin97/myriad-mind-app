# 任务：搭建大衍决 App monorepo 脚手架

## 上下文
- 项目: myriad-mind-app (独立跨平台桌面+移动App)
- 位置: D:/Project/myriad-mind-app/
- 架构文档: docs/architecture.md, CLAUDE.md
- 设计原则: 核心逻辑只写一次 (packages/core/)，平台差异各自实现
- 环境: Windows 11, Node 24, Rust 1.93, Python 3.12

## Monorepo 目标结构
```
packages/core/     → 共享纯逻辑 (TypeScript)
packages/ui/       → 共享 React 组件
apps/desktop/      → Tauri 2.x + React
apps/mobile/       → React Native Expo
```

## 任务清单 (按顺序)

### 1. 环境准备
- 安装 pnpm: `npm install -g pnpm`
- 验证: `pnpm --version`

### 2. 创建 pnpm workspace
- 在根目录创建 pnpm-workspace.yaml (packages/*, apps/*)
- 创建根 package.json (name: myriad-mind-app, private: true)
- 配置 scripts: dev, build, lint

### 3. 创建 packages/core/
- package.json: name @myriad-mind/core
- tsconfig.json
- src/index.ts (导出入口)
- src/types.ts (配置类型 Config, 笔记类型 Note, 面板类型 Dashboard)
- src/schema.ts (配置 Schema Zod 验证 — 参考 docs/architecture.md 已有定义)
- src/note-parser.ts (笔记解析/统计)
- src/panel-calc.ts (修为面板计算 — 等级/成就/标签云)

### 4. 创建 packages/ui/
- package.json: name @myriad-mind/ui
- 暂时只建骨架，不写复杂组件
- src/index.ts

### 5. 创建 apps/desktop/ (Tauri)
- `pnpm create tauri-app apps/desktop --template react-ts` 或手动创建
- 如果脚手架工具不可用，手动创建:
  - package.json + vite.config.ts + tsconfig.json
  - src/main.tsx (React 入口)
  - src-tauri/Cargo.toml
  - src-tauri/tauri.conf.json
  - src-tauri/src/main.rs (空壳)
- 配置引用 @myriad-mind/core 和 @myriad-mind/ui

### 6. 创建 apps/mobile/ (React Native Expo)
- `npx create-expo-app apps/mobile --template blank-typescript` 或手动创建
- 如果脚手架工具不可用，手动创建:
  - package.json + tsconfig.json
  - app.json (Expo 配置)
  - App.tsx (入口)
- 配置引用 @myriad-mind/core 和 @myriad-mind/ui

### 7. 验证
- 根目录 `pnpm install`
- `pnpm -r run build` (或 tsc --noEmit)
- 确保没有跨包引用错误

## 规则
1. 每完成一个主要步骤 commit 一次 (git add -A && git commit -m "message")
2. 如果某个脚手架命令失败，手动创建文件
3. 不要删除任何已有文档
4. 配置文件名和目录结构参照 docs/architecture.md
5. 遇到依赖安装失败，跳过并记录在 commit message
6. 完成后输出 DONE 并列出创建的文件清单

## 成功标准
- pnpm workspace 可正常安装依赖
- packages/core/ 有完整的类型定义
- apps/desktop/ 可构建 (或至少 pnpm install 通过)
- apps/mobile/ 可构建 (或至少 pnpm install 通过)

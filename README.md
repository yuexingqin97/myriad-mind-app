# 大衍决 App (Myriad Mind App)

> 神识一扫，万物皆可为笔记 — 从 Claude Code Skill 进化为独立跨平台 App

## 项目定位

将大衍决从 Claude Code Skill 改造为**独立桌面+移动 App**，免费、开源、本地运行。用户丢入视频链接/文章 URL/本地文件，自动炼化为结构化学习笔记（AI 摘要 + Mermaid 图表 + 术语表 + 扩展资源 + 评论区精华）。

- **桌面端**（Windows / Mac）：完整功能，Python 脚本 + FFmpeg + yt-dlp + faster-whisper + Claude API
- **移动端**（Android / iOS / 鸿蒙）：轻量伴侣，笔记阅读 + 文章处理 + 修为面板 + 搜索
- **无服务器**：一切在本地运行，数据完全由用户控制
- **免费开源**：MIT License

---

## 架构设计

### 技术栈

| 平台 | 框架 | 后端语言 | 前端语言 |
|------|------|----------|----------|
| Windows / Mac | Tauri 2.x | Rust | React 19 + TypeScript |
| Android / iOS | React Native (Expo) | TypeScript | React Native |
| 鸿蒙 | PWA（Phase 1）/ ArkTS WebView（Phase 2） | — | React |

**选型理由**：
- Tauri 2.x 产物小（< 10MB），Rust 后端天然适合调度 Python 子进程和检测系统依赖
- React Native 复用 TypeScript 技能，`packages/core/` 纯逻辑代码跨平台共享
- PWA 零成本覆盖鸿蒙基础场景（读笔记、看面板、改配置）

### 项目结构

```
myriad-mind-app/
├── packages/
│   ├── core/                  # 共享纯逻辑 (TS)
│   │   ├── types/             #   配置 schema、笔记元数据、输入模式
│   │   ├── config/            #   配置管理（Zod 校验 + 迁移）
│   │   ├── pipeline/          #   处理管线定义（步骤/模式/灵力预估）
│   │   ├── prompts/           #   Claude API prompt 模板
│   │   ├── note-utils/        #   笔记解析/搜索/标签提取
│   │   └── dashboard/         #   修为面板计算（修为值/成就/知识图谱）
│   └── ui/                    # 共享 UI 组件 (React)
│       ├── ConfigWizard/      #   新手指引向导
│       ├── NoteRenderer/      #   Markdown 渲染（Mermaid + 截图 + 时间戳）
│       ├── Dashboard/         #   修为面板可视化
│       └── common/            #   通用组件
├── apps/
│   ├── desktop/               # Tauri 2.x 桌面 App
│   │   ├── src-tauri/         #   Rust 后端
│   │   │   ├── commands/      #     Tauri IPC 命令
│   │   │   │   ├── python.rs  #       调度 Python 脚本
│   │   │   │   ├── deps.rs    #       依赖检测（FFmpeg/yt-dlp/Python/CUDA）
│   │   │   │   ├── claude.rs  #       Claude API 客户端（SSE 流式）
│   │   │   │   ├── config.rs  #       配置读写（+ OS 密钥链）
│   │   │   │   └── pipeline.rs#       多步骤管线编排
│   │   │   └── python/        #     Python venv 管理
│   │   └── src/               #   React 前端
│   │       └── pages/         #     首页/处理/笔记库/笔记详情/面板/搜索/对比/设置
│   └── mobile/                # React Native 移动 App
│       └── src/
│           ├── screens/       #   首页/笔记阅读/面板/文章处理/搜索/设置
│           ├── components/    #   笔记卡片/Mermaid渲染/Markdown渲染
│           └── services/      #   Claude API 调用/本地存储
└── scripts/                   # 现有 Python 脚本（git submodule）
    ├── transcribe_faster_whisper.py
    ├── extract_keyframes.py
    ├── download_video_candidates.py
    ├── download_youtube_subtitles.py
    ├── install_faster_whisper.py
    └── list_ai_douyin_tasks.py
```

### 桌面端架构

```
用户输入 URL/文件
     │
     ▼
React 前端 ──Tauri IPC──▶ Rust 后端
                              │
                              ├── 步骤 0：判断输入类型（8 种模式）
                              ├── 步骤 0.6：检查依赖（Python/FFmpeg/yt-dlp/GPU）
                              ├── 步骤 0.7：灵力预估 → 用户确认
                              ├── 步骤 1-4：调度 Python 脚本
                              │   ├── download_video_candidates.py
                              │   ├── download_youtube_subtitles.py
                              │   ├── ffmpeg（提取音频）
                              │   ├── extract_keyframes.py
                              │   └── transcribe_faster_whisper.py
                              ├── 步骤 5-7：调用 Claude API（流式生成笔记）
                              ├── 步骤 8：清理临时文件
                              └── 步骤 9：更新修为面板 + 学习建议
                              │
                    解析 JSON stdout ◀── 每个 Python 脚本返回结构化结果
                    推送进度事件 ──▶  React 前端实时显示进度条
                    流式笔记内容 ──▶  React 前端逐字展示生成过程
```

### 移动端功能裁剪

| 功能 | 桌面 | 移动 | 说明 |
|------|------|------|------|
| 视频下载 + ASR + 截图 | ✅ 全功能 | ❌ | 需要 FFmpeg/yt-dlp/Python/GPU |
| 文章 URL 处理 | ✅ | ✅ 轻量 | fetch 网页 → 调 Claude API |
| 本地文档处理 | ✅ | ✅ 只读 | 读取 .md，不处理 .mp4 |
| 笔记浏览 / 阅读 | ✅ | ✅ | 核心移动场景 |
| 修为面板 | ✅ | ✅ | 纯计算，100% 支持 |
| 搜索 / 对比 | ✅ | ✅ | 本地全文搜索 |
| 配置管理 | ✅ | ✅ | 全功能设置 |

### 共享代码策略

**完全共享**（`packages/core/`）：
- 配置类型定义 + Zod 校验
- Claude API prompt 模板
- 笔记元数据解析（标签/难度/阅读时长）
- 修为面板计算（修为值/等级/成就判定）
- 知识图谱构建
- 全文搜索算法
- 灵力预估（Token/时间估算）

**不可共享**（平台特定实现）：
- 进程管理（Rust `std::process::Command` vs 无）
- 系统依赖检测（Rust 系统检查 vs 无）
- Claude API HTTP 层（Rust `reqwest` vs JS `fetch`）
- 文件系统（Tauri fs plugin vs expo-file-system）

---

## 新手指引向导

把现有 25+ 个环境变量变成 **6 步可视化配置向导**：

```
┌────────────────────────────────────────────┐
│  🏔️ 大衍决 — 初次踏入修炼之路              │
│                                            │
│  Step 1/6: 系统环境检查                     │
│  ┌──────────────────────────────────────┐  │
│  │ ✅ Python 3.12     ✅ FFmpeg 8.1     │  │
│  │ ✅ yt-dlp          ✅ GPU (RTX 4070)  │  │
│  │ ⚠️ faster-whisper  未安装 → [一键安装] │  │
│  └──────────────────────────────────────┘  │
│                        [下一步：ASR 设置]   │
└────────────────────────────────────────────┘
```

| 步骤 | 内容 |
|------|------|
| 1. **系统检查** | 自动检测 Python/FFmpeg/yt-dlp/GPU/CUDA，缺失项一键安装 |
| 2. **ASR 配置** | 选 faster-whisper（免费本地）或 火山引擎（云端），模型大小选择器 |
| 3. **视频解析** | AI Douyin API Key（附注册链接），可选 TikHub，或跳过 |
| 4. **功能开关** | 关键帧/Mermaid/资源推荐/评论区/阅读信息，可视化 toggle |
| 5. **Claude API Key** | 输入 Key → 存入 OS 密钥链，永不明文存储 |
| 6. **输出位置** | 笔记保存目录（推荐选云同步文件夹实现跨设备） |

---

## 配置管理

### 安全模型

```
myriad-mind-config.json     ← 所有非敏感配置（功能开关/输出路径/模型参数）
OS 密钥链                   ← API Key / Token（Claude Key、AI Douyin Key、TikHub Token）
```

- Windows：凭据管理器（Credential Manager）
- macOS：Keychain
- Linux：libsecret / gnome-keyring
- Android：Keystore
- iOS：Keychain

### 配置文件格式（JSON，有版本号支持迁移）

```jsonc
{
  "version": 1,
  "asr": {
    "backend": "faster-whisper",
    "faster_whisper": { "model_size": "small", "device": "auto" }
  },
  "video": {
    "provider": "ai-douyin"
    // api_key 在 OS 密钥链，不在此文件
  },
  "features": {
    "keyframes": true, "mermaid": true, "resources": true,
    "comments": true, "reading_info": true, "estimation": true
  },
  "keyframes": { "interval": 30, "max_frames": 50, "mode": "interval" },
  "output": { "note_dir": "", "cleanup_temp": true },
  "post_process": { "auto_update_panel": true, "auto_suggest_next": true }
}
```

---

## 笔记存储与同步

### 格式

纯 `.md` 文件 + `assets/{VIDEO_ID}/` 截图子目录，人可读、Git 友好、云同步友好：

```
{NOTE_OUTPUT_DIR}/
├── 修为面板.md
├── Bevy学习笔记/
│   ├── LearnEcs.md
│   ├── Learn2D.md
│   └── assets/BV14UzWBLEXD/
│       ├── frame_0005.png
│       └── frame_0010.png
└── Unreal学习笔记/
    └── ...
```

### 跨设备同步

**不做同步服务器**。用户把 `NOTE_OUTPUT_DIR` 选在云盘文件夹即可：
- iCloud Drive / Dropbox / Google Drive / OneDrive
- 冲突处理：last-write-wins + `.conflict` 备份文件

### 笔记索引

SQLite 轻量索引加速搜索和面板统计：
- 表：`notes`（路径/标题/标签/难度/可靠性/字数/来源 URL/平台）
- 全文搜索：FTS5 虚拟表
- 文件监听器自动增量更新（桌面端）
- 手动刷新（移动端）

---

## Markdown 渲染

| 元素 | 桌面端 | 移动端 |
|------|--------|--------|
| Markdown | react-markdown + remark-gfm | WebView 渲染 HTML |
| 代码高亮 | rehype-highlight + highlight.js | 同上 |
| Mermaid | mermaid.js 客户端渲染 SVG | mermaid.js in WebView |
| 截图 | `asset://` 本地协议加载 | `file://` URI 加载 |
| 时间戳链接 | 浏览器打开视频 | 浏览器打开视频 |

---

## AI 集成

直接调 Anthropic API，无中间服务器：

```
App (桌面/移动) ──HTTPS + SSE──▶ api.anthropic.com
                 Anthropic API Key
```

- 桌面端：Rust `reqwest` + SSE 流式（用户看到笔记逐字生成）
- 移动端：JS `fetch` + ReadableStream
- Prompt 模板：`packages/core/src/prompts/` 统一管理，完全复用现有 SKILL.md 的 prompt 设计
- 支持：prompt caching、自动重试、Token 计数预警

---

## 开发路线图

| Phase | 时间 | 目标 |
|-------|------|------|
| **1. 地基** | 2-3 周 | Monorepo 脚手架 + 桌面 App 文章处理 + 笔记浏览；移动端笔记阅读器 |
| **2. 视频** | 2-3 周 | 桌面端完整视频处理链路（下载→ASR→截图→AI→笔记） |
| **3. 面板** | 2-3 周 | 修为面板、搜索、对比、代码分析、目录处理 |
| **4. 移动增强** | 2 周 | 移动端文章处理、面板、搜索、配置向导 |
| **5. 发布** | 2 周 | 安装包（.msi/.dmg/.apk）、自动更新、文档、i18n（中/英） |

---

## 法律风险与免责

### 风险分析

| 风险 | 等级 | 说明 |
|------|------|------|
| 视频下载违反平台 ToS | 🟠 中 | yt-dlp 类工具处于灰色地带，但个人开源项目极少被追责 |
| 第三方解析服务 | 🟡 中低 | AI Douyin 依赖用户自行申请 Key |
| 网页爬取 | 🟡 中低 | 仅处理用户主动提供的 URL |
| 衍生作品版权 | 🟢 低 | 笔记默认私有，用户自行承担公开发布责任 |
| GitHub DMCA 下架 | 🟡 中低 | 同类项目（yt-dlp/you-get/BBDown）均存活，GitHub 有开发者辩护基金 |
| 开源许可证 | 🟢 无 | 上游 MIT，所有依赖均为宽松许可证 |

### 降低风险的措施

1. README 顶部免责声明：工具仅供个人学习/研究/教育
2. 不内置任何平台 API Key 或 Cookie
3. 不提供"一键分享笔记"功能
4. 不做商业化（收费/广告/增值）
5. 保留 MIT 许可证和上游版权声明

---

## 技术依赖

| 依赖 | 桌面前置条件 | 移动端 |
|------|-------------|--------|
| Python 3.9+ | 用户安装或 App 引导安装 | 不需要 |
| FFmpeg | 可内置捆绑（~80MB）或引导安装 | 不需要 |
| yt-dlp | 可内置捆绑或 pip 安装 | 不需要 |
| faster-whisper | App 内一键安装（venv + pip） | 不需要 |
| Claude API Key | 用户自己的 Key | 用户自己的 Key |
| AI Douyin API Key | 可选，用户自行申请 | 不需要 |

---

## 相关文档

- [大衍决 Skill 源码](../myriad-mind/)
- [大衍决 Skill 定义 (SKILL.md)](../myriad-mind/SKILL.md)
- [现有笔记输出](../大衍决残卷/)
- [上游项目](https://github.com/imlewc/myriad-mind-skill)

---

## License

MIT License — 同上游项目。

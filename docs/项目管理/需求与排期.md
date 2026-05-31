# 大衍决 App — 需求整理 & 任务拆解 & 时间评估

> 2026-05-31 整理，基于架构设计 + 依赖分析 + 产品讨论结论

---

## 一、核心设计决策（前几次讨论结论）

| 决策 | 结论 |
|------|------|
| 移动端框架 | React Native Expo，**不做** Tauri 移动端 — 阅读类 App 原生滚动体验 >> WebView |
| 代码复用 | `packages/core/` 共享全部纯逻辑，UI 各自最优 |
| Claude 可见性 | **零痕迹** — 全部修炼术语，MindEngine 抽象，后端可换模型 |
| API 调用模式 | **单轮** — 一次调用生成完整笔记，杜绝 Claude"提问打断" |
| 多模型支持 | MindEngine trait → Anthropic / DeepSeek / 自定义，三个适配器 |
| API Key 存储 | OS 密钥链，永不明文落盘 |
| Python 脚本 | `std::process::Command` 黑盒子调用，**不内嵌** Python 解释器 |
| CUDA | 不是依赖 — 检测到推荐、检测不到静默走 CPU |

---

## 二、功能需求（按优先级分层）

### P0 — 核心链路（必须可用）

```
用户丢视频链接 → 下载 → ASR → 截图 → AI 生成笔记 → 保存 .md
```

- 支持 B站 / YouTube 视频链接
- 本地视频/音频文件处理
- 文章 URL 处理（知乎/CSDN/掘金/微信公众号/通用网页）
- 笔记输出：Markdown + Mermaid + 截图 + 术语表 + 扩展资源
- 流式笔记生成（用户看到逐字输出）

### P0 — 配置与安全

- 新手指引向导（6 步：系统检查 → ASR → 视频解析 → 功能开关 → API Key → 输出目录）
- API Key 存 OS 密钥链
- 多模型选择（Anthropic / DeepSeek / 自定义 OpenAI 兼容）
- 功能开关：关键帧/Mermaid/资源推荐/评论区/阅读信息/灵力预估

### P1 — 笔记管理

- 笔记浏览 + Markdown 渲染（Mermaid + 截图 + 代码高亮）
- 搜索（FTS5 全文搜索）
- 笔记对比（两篇并排）
- 修为面板（统计 + 成就 + 知识图谱）
- 目录批量处理

### P2 — 增强功能

- 代码项目分析模式（GitHub URL / 本地代码目录）
- 批量模式（多个 URL 排队处理）
- AI Douyin 历史任务查询
- 火山引擎 VC 云端 ASR（替代 faster-whisper）
- TikHub 自托管方案

### P3 — 发布与运维

- 桌面安装包（Windows .msi / macOS .dmg）
- 移动 App 打包（.apk / .ipa）
- 自动更新（GitHub Releases）
- i18n（中文 / 英文）
- 鸿蒙 PWA（Phase 1）

---

## 三、任务拆解

### Phase 1：地基（3-4 周）

#### 1.1 Monorepo 脚手架 — 3 天

```
任务：
├── pnpm workspace 初始化（packages/core + packages/ui + apps/desktop + apps/mobile）
├── TypeScript tsconfig 共享配置
├── Rust Cargo workspace（如果有多个 crate）
├── ESLint + Prettier 共享配置
├── .gitignore / .env.example / .editorconfig
└── CI 骨架（GitHub Actions: lint + typecheck）

产出：空仓库能 pnpm install && cargo build
```

#### 1.2 共享核心包 `packages/core/` — 5 天

```
任务：
├── types/          — 配置 Schema/笔记元数据/输入模式/灵力预估/成就定义
├── config/         — Zod 校验 + 配置迁移（version 字段驱动）
├── prompts/        — 从 SKILL.md 提取所有 prompt 模板为 TS 函数
│   ├── note-gen.ts       — 笔记生成 prompt（含行为规则："禁止提问"）
│   ├── summarize.ts      — AI 摘要 prompt
│   ├── translate.ts      — 翻译 prompt
│   ├── code-analysis.ts  — 代码分析 prompt
│   ├── compare.ts        — 对比 prompt
│   └── resource-recommend.ts — 扩展资源推荐 prompt
├── note-utils/     — 笔记解析/标签提取/元数据读取
├── dashboard/      — 修为面板计算（7 境界映射 + 6 成就判定）
├── estimation/     — 灵力预估算法（时间 + Token 估算表）
└── 单元测试（vitest）

产出：packages/core/ 通过全部单测
```

#### 1.3 Tauri 桌面 App 骨架 — 5 天

```
任务：
├── Tauri 2 + React + Vite 初始化
├── Rust → 命令模组结构搭建
│   ├── commands/mod.rs
│   ├── commands/config.rs    — 读 JSON 配置（不含 Key）
│   ├── commands/mind.rs      — MindEngine trait + adapters
│   ├── commands/python.rs    — Python 脚本调度框架
│   ├── commands/deps.rs      — 系统依赖检测
│   └── error.rs              — 统一错误类型（thiserror + 修炼口吻）
├── React → 页面路由骨架
│   ├── 首页（输入区）/ 处理中 / 笔记库 / 笔记详情 / 面板 / 搜索 / 设置
├── Tauri IPC 通道打通（invoke → Rust → emit → React）
└── 热重载开发流程跑通

产出：桌面 App 能启动，React 页面能调 Rust 命令并收到事件
```

#### 1.4 配置向导 — 4 天

```
任务：
├── 6 步向导 UI（packages/ui/ConfigWizard/）
├── 系统依赖检测实现
│   ├── detect_python() → 扫描 PATH，读版本
│   ├── detect_ffmpeg() → PATH + WinGet 目录 + 已知路径
│   ├── detect_ytdlp() → PATH + pip list
│   ├── detect_whisper_venv() → 检查 ~/.cache/myriad-mind/faster-whisper-venv/
│   ├── detect_gpu() → nvidia-smi / DXGI
│   └── 缺失项一键安装（调 install_faster_whisper.py / pip install yt-dlp）
├── API Key 输入 → 写 OS 密钥链
├── 功能开关可视化 toggle
├── 输出目录选择器（Tauri dialog）
└── 配置持久化（myriad-mind-config.json + 密钥链）
```

#### 1.5 文章处理 + 笔记浏览 — 5 天

```
任务：
├── Rust: 文章抓取（reqwest GET → 提取 title + 正文）
├── Rust: MindEngine 调用（AnthropicAdapter 先实现）
├── Rust: SSE 流式解析 + Tauri events 推送
├── React: 输入区（粘贴 URL / 拖拽文件）
├── React: 丹炉动画 + 流式笔记逐字渲染
├── React: Markdown 笔记渲染（react-markdown + Mermaid + 代码高亮）
├── React: 笔记保存（.md 写入 NOTE_OUTPUT_DIR）
├── 缓存：临时文件写入 /tmp，完成后清理
└── curl 降级引导（知乎/公众号 → 提示用户保存 HTML 文件）

产出：用户粘贴文章 URL → 看到丹炉动画 → 得到一篇笔记 → 能浏览
```

#### 1.6 移动端笔记阅读器 — 3 天

```
任务：
├── React Native Expo 初始化
├── WebView 渲染 Markdown + Mermaid
├── 笔记列表（expo-file-system 扫描 NOTE_OUTPUT_DIR）
├── 笔记阅读（加载 .md → WebView 渲染）
├── expo-secure-store 存储 API Key
└── 跨设备同步引导（提示用户选云盘目录）

产出：移动 App 能看笔记
```

---

### Phase 2：视频（3-4 周）

#### 2.1 Python 脚本集成 — 4 天

```
任务：
├── git submodule 引入上游 scripts/
├── Rust python.rs 封装 6 个脚本的类型化调用
│   ├── transcribe_audio() → TranscriptionResult
│   ├── extract_keyframes() → KeyframeResult
│   ├── download_video_candidates() → DownloadResult
│   ├── download_youtube_subtitles() → SubtitleResult
│   ├── install_faster_whisper() → InstallResult
│   └── list_ai_douyin_tasks() → TaskListResult
├── JSON stdout 解析（serde_json）
├── stderr 错误提取 + 修炼口吻翻译
└── 集成测试（用本地测试视频）
```

#### 2.2 输入识别 + 管线编排 — 5 天

```
任务：
├── Rust: 输入类型识别（8 种模式 — URL/域名 → 平台判定）
├── Rust: Pipeline 编排器（10 步管线）
│   ├── 步骤 0:  模式识别
│   ├── 步骤 0.6: 依赖检查
│   ├── 步骤 0.7: 灵力预估 → Tauri event → 前端确认
│   ├── 步骤 1:  获取视频信息/下载直链（AI Douyin / TikHub）
│   ├── 步骤 2:  下载视频
│   ├── 步骤 3:  提取音频（ffmpeg）
│   ├── 步骤 3.5: 关键帧截图
│   ├── 步骤 4:  ASR 转写（faster-whisper / volcengine）
│   ├── 步骤 5-7: Claude API 生成笔记
│   ├── 步骤 8:  清理临时文件
│   └── 步骤 9:  更新修为面板
├── 每步进度事件推送（step_started / step_completed / percentage）
└── 错误恢复：某一环节失败不丢已完成的中间产物

产出：丢 B站链接 → 自动走完 10 步 → 拿到笔记
```

#### 2.3 关键帧 + 评论区 — 3 天

```
任务：
├── extract_keyframes.py 集成 + 截图渲染
├── 截图嵌入笔记（时间戳旁边 + 可点击跳转）
├── B站评论 API 调用 + 精选筛选
├── YouTube 评论（yt-dlp --write-comments）
└── 评论区精华格式化输出
```

#### 2.4 翻译 + 英文内容处理 — 2 天

```
任务：
├── Rust: 语言检测（中文字符占比 < 30% → 判定英文）
├── Prompt 注入翻译指令（中英对照输出）
└── 翻译后文本替换流程
```

#### 2.5 DeepSeek 适配器 — 2 天

```
任务：
├── Rust: DeepSeekAdapter 实现 MindEngine trait
├── OpenAI 兼容格式 SSE 解析
├── 模型选择 UI（Anthropic / DeepSeek / 自定义 endpoint）
└── 自定义 endpoint 配置（任何 OpenAI 兼容 API）
```

---

### Phase 3：面板与增强（2-3 周）

#### 3.1 修为面板 — 4 天

```
任务：
├── 面板计算器（遍历笔记目录 → 统计 → 7 境界映射）
├── 成就系统（6 项自动判定）
├── 知识图谱可视化（react-force-graph 或 Mermaid）
├── 面板 UI（Dashboard 组件 — 修为进度条 + 成就徽章 + 技术栈雷达）
└── 自动更新（AUTO_UPDATE_PANEL 配置）
```

#### 3.2 搜索 — 3 天

```
任务：
├── SQLite + FTS5 全文索引
├── 笔记增量同步（文件监听器 → 自动更新索引）
├── 搜索 UI（关键词 + 标签 + 难度筛选）
└── 搜索结果高亮
```

#### 3.3 笔记对比 — 2 天

```
任务：
├── 双栏对比 UI
├── Claude 对比 prompt（compare.ts）
├── 差异高亮 + 维度评分表
└── 选两篇笔记 → 一键对比
```

#### 3.4 目录批量处理 — 2 天

```
任务：
├── 目录扫描（递归 2 层，.md/.mp4/.mp3 等）
├── 批量排队处理（UI 队列面板）
├── 合并笔记输出（一篇含多个分段）
└── 失败文件标注 + 不阻断后续
```

#### 3.5 代码项目分析 — 3 天

```
任务：
├── GitHub URL clone + 本地目录扫描
├── 灵力评估（文件数 → 分级 → 确认/核心模块/快速概览）
├── 架构分析 prompt
├── Mermaid 架构图生成
└── 代码阅读指南输出
```

---

### Phase 4：移动增强（2 周）

#### 4.1 移动端文章处理 — 3 天

```
任务：
├── fetch URL → HTML → 提取文本
├── Claude API 流式调用（JS fetch + ReadableStream）
├── 笔记写入（expo-file-system）
└── 降级引导（反爬平台 → 提示复制粘贴）
```

#### 4.2 移动端面板 + 搜索 — 2 天

```
任务：
├── 面板计算（复用 packages/core/dashboard/）
├── 成就展示 + 境界进度条
└── 本地全文搜索（遍历 .md 文件）
```

#### 4.3 移动端配置向导 — 2 天

```
任务：
├── 精简版配置（API Key + 功能开关 + 输出目录）
├── expo-secure-store 存储 Key
└── 与桌面端配置 Schema 一致（packages/core/ 共享校验）
```

---

### Phase 5：发布（2 周）

#### 5.1 桌面端打包 — 3 天

```
任务：
├── Windows: .msi 安装包 + .exe 便携版
├── macOS: .dmg + .app（如需签名则配置 Developer ID）
├── FFmpeg 内嵌评估（~80MB，决定是捆绑还是检测+引导）
└── 安装包测试（清洁环境安装 → 走完完整流程）
```

#### 5.2 移动端打包 — 2 天

```
任务：
├── Android: EAS Build → .aab + .apk
├── iOS: EAS Build → .ipa（需 Apple Developer Account）
└── 鸿蒙: PWA manifest + GitHub Pages 部署
```

#### 5.3 自动更新 — 1 天

```
任务：
├── tauri-plugin-updater 配置
├── GitHub Releases 自动发布 CI
└── 版本号管理 + Changelog
```

#### 5.4 文档 + i18n — 2 天

```
任务：
├── README（中/英）
├── 用户手册（安装 + 配置 + 使用）
├── 免责声明（README 顶部）
├── i18n 框架（react-i18next）
└── 中文 → 英文 key mapping
```

---

## 四、总时间评估

| Phase | 内容 | 预估 | 累计 |
|-------|------|------|------|
| **1. 地基** | Monorepo + 核心包 + Tauri 骨架 + 配置向导 + 文章处理 + 移动端笔记阅读 | **3-4 周** | 4 周 |
| **2. 视频** | Python 集成 + 10 步管线 + 截图评论 + 翻译 + DeepSeek 适配 | **3-4 周** | 8 周 |
| **3. 面板增强** | 修为面板 + 搜索 + 对比 + 目录 + 代码分析 | **2-3 周** | 11 周 |
| **4. 移动增强** | 移动端文章处理 + 面板搜索 + 配置向导 | **2 周** | 13 周 |
| **5. 发布** | 打包 + 自动更新 + 文档 + i18n | **2 周** | **15 周** |

**总计：约 15 周（3.5 个月），单人全职。**

如有第二个开发者（前端 + 后端分工），可压缩到 10-12 周。最长的 Phase 1 和 2 各需要一个月，因为它们是从零搭建基础设施。

---

## 五、关键路径与风险

### 关键路径

```
Monorepo 脚手架 (3d) → 核心包 (5d) → Tauri 骨架 (5d)
  ↓
配置向导 (4d) ──┬── 文章处理 (5d) ──┐
               └── 移动阅读 (3d) ──┤
                                  ↓
                          视频管线 (5d) → Python 集成 (4d) → 截图评论 (3d)
                                  ↓
                          面板搜索 (7d) → 移动增强 (5d) → 发布 (5d)
```

### 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| faster-whisper 安装失败率高 | Phase 2 卡住 | 安装脚本在多个 Python 版本测试；降级方案：火山引擎云端 ASR |
| AI Douyin 服务不稳定 | 抖音/B站解析失败 | 支持 TikHub 备用；B站回退 yt-dlp |
| YouTube 反爬升级 | 字幕下载失败 | yt-dlp 社区通常快速适配，等待上游更新 |
| Tauri 2 移动端生态仍不成熟 | 原型推进慢 | 当前已选了 RN，Tauri 只做桌面，此风险已回避 |
| Claude API 涨价/限流 | 用户成本上升 | 已设计多模型抽象，DeepSeek 替代方案只需 ¥0.05/篇 |
| 单人体力上限 | Phase 3+ 质量下降 | Phase 3 面板/搜索可砍功能；Phase 4 移动增强可延后 |

---

## 六、MVP 定义（最小可发布版本）

如果时间极度紧张，**Phase 1 完成后即可发 Alpha 版**：

```
MVP 包含：
✅ 文章 URL → 笔记（桌面端）
✅ 笔记浏览（桌面 + 移动）
✅ 配置向导（6 步）
✅ 多模型支持（Anthropic + DeepSeek）
✅ Markdown 渲染（Mermaid + 代码高亮）

MVP 不包含：
❌ 视频处理（Phase 2）
❌ 修为面板（Phase 3）
❌ 搜索 / 对比（Phase 3）
❌ 代码项目分析（Phase 3）

MVP 时间：4 周，单人。
```

MVP 的价值：即使没有视频处理，一个"文章 → 结构化笔记"的工具已经对很多人有用。且此时所有基础设施（monorepo、核心包、Tauri 骨架、配置向导、MindEngine）都已就绪，后续 Phase 都是增量添加。

# 大衍决 App — 开发任务清单

> 最后更新：2026-05-31
>
> **v1.0 定位**：专注"输入 → 炼化 → 生成笔记"，修为面板展示列表 + 统计。不提供笔记阅读/编辑/搜索功能，用系统默认 .md 编辑器打开即可。

---

## 一、管道打通（让炼化流程真正跑起来）

### 1.0 内嵌 Python 环境（免安装）
- [ ] Tauri bundle 打包 Python 3.12 Embeddable（Windows 专用，~12MB）
- [ ] 首次启动自动解压到 `~/.cache/myriad-mind/python/`
- [ ] 自动安装 pip + faster-whisper + yt-dlp 到 embeddable Python
- [ ] macOS / Linux 探测系统自带 Python 3，无需打包
- [ ] Rust `deps.rs` 优先使用内嵌 Python → 回退系统 Python

### 1.1 接入 Python 脚本
- [ ] 将 `D:/Project/MyClaude/myriad-mind/scripts/` 作为 git submodule 引入到 `scripts/`
- [ ] 验证 6 个脚本可被 Rust `Command::new` 调用
- [ ] 实现 `install_faster_whisper.py` 的 venv 自动创建逻辑

### 1.2 前端对接 Rust 后端
- [ ] 用 `@tauri-apps/api` 的 `invoke()` 替换 App.tsx 中的 `setTimeout` 模拟
- [ ] 实现 SSE 事件监听 — `listen("claude-stream-delta", ...)` 实时笔记生成
- [ ] 进度条对接真实的 `PipelineProgress` 事件

### 1.3 管线编排实现
- [ ] Rust `pipeline.rs` 实现 `execute(input, config)` 主函数
- [ ] 串联 10 步：模式识别 → 依赖检查 → 灵力预估 → 下载 → 提取音频 → ASR → 关键帧 → AI 笔记 → 清理 → 面板更新
- [ ] 每个步骤通过 Tauri event 推进度到前端
- [ ] 支持用户中途取消

### 1.4 系统依赖检测
- [ ] Rust `deps.rs` 启动时自动探测 Python / FFmpeg / yt-dlp / CUDA
- [ ] 前端 `DepCheckPanel` 展示检测结果，缺失项给出安装指引

---

## 二、安全与配置

### 2.1 OS 密钥链
- [ ] Windows：接入 `windows-credentials` crate 读写 Credential Manager
- [ ] macOS：接入 `security-framework` crate 读写 Keychain
- [ ] Linux：接入 `libsecret` 或 `oo7` crate
- [ ] 替换 config.rs 中的 stub 实现
- [ ] 密钥链条目命名规范：`myriad-mind/{service}`

### 2.2 配置页签（与炼化/修为平级）
- [ ] 将配置从 Modal 弹窗改为独立顶栏页签（⚙️ 配置），与 📥 炼化 / 📊 修为 平级
- [ ] 配置表单覆盖所有 `.env` 参数，分 6 组：
  - 🎙️ 语音识别（ASR 后端 / faster-whisper 模型大小/设备 / 火山引擎 Token）
  - 📹 视频解析（AI Douyin / TikHub，API Key 从密钥链读取）
  - ⚙️ 功能开关（关键帧 / Mermaid / 资源推荐 / 评论区 / 阅读信息 / 灵力预估）
  - 🖼️ 关键帧（间隔 / 最大数量 / 模式）
  - 📂 输出设置（笔记目录 / 自动清理 / 元信息 / 调试信息）
  - ✨ 收尾设置（自动更新面板 / 学习推荐）
- [ ] 首次启动检测 — 无配置文件时默认显示配置页签（引导用户完成初始设置）
- [ ] 已配置时默认显示炼化页签
- [ ] 配置变更自动保存到 `%APPDATA%/myriad-mind/config.json`
- [ ] 敏感字段（API Key）仅显示脱敏形式（`sk-ant-...xxxx`），编辑时写入 OS 密钥链而非明文

### 2.3 配置存储层
- [ ] 配置读写到 `%APPDATA%/myriad-mind/config.json`
- [ ] 版本号 + 迁移逻辑（`version: 1` → 后续升级）
- [ ] 移动端配置存储（AsyncStorage）

### 2.4 API Key 管理
- [ ] Claude API Key — 从密钥链读取，前端只显示 `sk-ant-...xxxx`
- [ ] AI Douyin API Key — 同上
- [ ] TikHub Token — 同上
- [ ] 火山引擎 VC Token — 同上

---

## 三、桌面端功能完善

### 3.1 视频处理管线
- [ ] B 站视频：AI Douyin 解析 → 下载 → 提取音频 → ASR → 笔记
- [ ] YouTube：yt-dlp 优先字幕 → 无字幕则下载 + ASR
- [ ] 抖音/小红书：AI Douyin 解析 → 下载 → ASR → 笔记

### 3.2 文章处理管线
- [ ] WebFetch 抓取文章内容
- [ ] 平台反爬降级（知乎/公众号提示用户粘贴 HTML）
- [ ] Claude API 生成结构化学术笔记

### 3.3 本地文件处理
- [ ] 本地视频：提取音频 → ASR → 关键帧 → 笔记
- [ ] 本地音频：ASR → 笔记
- [ ] 本地文档：直接读 → Claude 分析 → 笔记
- [ ] 本地目录：扫描 → 逐个处理 → 合并笔记

### 3.4 代码项目分析
- [ ] GitHub URL → git clone → 扫描结构 → Claude 分析 → 架构笔记
- [ ] 本地代码目录同理
- [ ] 灵力评估（按文件数分四级，>300 文件必须确认）

### 3.5 笔记列表展示（修为面板中）
- [ ] 扫描 `note_dir` 下所有 .md，解析 front matter 获取元信息
- [ ] 在修为面板 Dashboard 中展示笔记列表（标题/日期/类型/难度/阅读时长）
- [ ] 点击笔记 → 用系统默认 .md 编辑器打开（`open` / `xdg-open`）
- [ ] ~~SQLite FTS5 全文搜索~~ → v2
- [ ] ~~内嵌 Mermaid 图表渲染~~ → v2
- [ ] ~~截图资产预览~~ → v2
- [ ] ~~内嵌笔记阅读器~~ → 砍掉，用系统编辑器

### 3.6 修为面板
- [ ] 扫描笔记目录统计（已有 `computeStats`）
- [ ] 境界进度 + 成就判定
- [ ] 连续学习天数计算
- [ ] 知识图谱（标签关系图）
- [ ] 面板持久化到 `修为面板.md`

---

## 四、移动端功能完善

### 4.1 文章处理
- [ ] fetch URL → 提取正文 → Claude API → 写 .md
- [ ] SSE 流式生成（expo 的 Text 逐字显示）

### 4.2 笔记列表展示
- [ ] 扫描笔记目录，展示列表（标题/日期/类型）
- [ ] ~~WebView Markdown 渲染~~ → 砍掉，移动端用系统文件管理器查看 .md
- [ ] ~~全文搜索~~ → v2

### 4.3 修为面板
- [ ] 纯计算（已有 `calculateCultivation` / `checkAchievements`）
- [ ] 与桌面端共享 `@myriad-mind/core`

---

## 五、构建与分发

### 5.1 桌面端打包
- [ ] Windows：`.msi` 安装包 + `.exe` 便携版
- [ ] macOS：`.dmg` + `.app` 签名公证
- [ ] 自动更新：`tauri-plugin-updater` + GitHub Releases

### 5.2 移动端打包
- [ ] Android：EAS Build → `.aab`（Play Store）/ `.apk`
- [ ] iOS：EAS Build → `.ipa`（TestFlight）
- [ ] 鸿蒙：PWA (Phase 1) → ArkTS WebView (Phase 2)

### 5.3 CI/CD
- [ ] GitHub Actions：push main → lint + test + build
- [ ] release tag → 全平台构建 → GitHub Releases 发布

---

## 六、品质与文档

### 6.1 测试
- [ ] `packages/core` 单元测试（vitest）
- [ ] `packages/ui` 组件测试
- [ ] Rust `error.rs` / `commands/` 单元测试
- [ ] E2E 测试（Tauri + Playwright？）

### 6.2 文档
- [ ] README 安装说明（各平台依赖要求）
- [ ] README 免责声明（已分析法律风险）
- [ ] 用户文档：如何申请各 API Key
- [ ] 贡献指南 CONTRIBUTING.md

### 6.3 法律合规
- [ ] README 顶部免责声明 ✅（已规划）
- [ ] MIT 许可证保留原作者版权
- [ ] 不内置任何 API Key / Token
- [ ] 不提供分享/发布功能

---

## 进度总览

| 阶段 | 状态 | 完成日期 |
|------|------|----------|
| 项目脚手架 | ✅ 完成 | 2026-05-31 |
| `@myriad-mind/core` 核心逻辑 | ✅ 完成 | 2026-05-31 |
| `@myriad-mind/ui` 组件库 | ✅ 完成 | 2026-05-31 |
| Desktop 前端页面 | ✅ 完成 | 2026-05-31 |
| Desktop Rust 后端 | ✅ 完成 | 2026-05-31 |
| CC Switch 暗色风格重构 | ✅ 完成 | 2026-05-31 |
| 一、管道打通 | ⏳ 待开始 | — |
| 二、安全与配置 | ⏳ 待开始 | — |
| 三、桌面端功能完善 | ⏳ 待开始 | — |
| 四、移动端功能完善 | ⏳ 待开始 | — |
| 五、构建与分发 | ⏳ 待开始 | — |
| 六、品质与文档 | ⏳ 待开始 | — |

---

## 当前里程碑

```
已完成的脚手架 ──────────────────────────── ✗
                                                   │
通往第一个可用版本 ─── 1.x 管线打通 ─── 安全配置 ─── ✗  ← 你在这里
                                                   │
功能完善 ─── 视频/文章/本地/代码 ─── 笔记管理 ────── ✗
                                                   │
发布就绪 ─── 测试 ─── 打包 ─── CI/CD ─── 文档 ──── ✗
```

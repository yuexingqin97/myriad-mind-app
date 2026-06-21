# YouTube 下载失败诊断（JS 挑战 → 429 → 格式限制 → 文件缺失）

> 诊断日期：2026-06-22
> 状态：**根因定位中，格式选择器待修**

---

## 问题现象

炼化 YouTube 视频（`cLQoSpsK5Ek`），下载阶段经历了多轮对抗，最终卡在"yt-dlp 返回成功但视频文件不存在"。

**完整失败链**：

1. 字幕优先 → yt-dlp 缺 JS runtime（`No supported JavaScript runtime`） → 字幕不可用
2. 回退 ASR → 下载视频 → yt-dlp 返回成功 → `video.mp4` 文件不存在（0 bytes）
3. 管线终止

---

## 已完成的修复（前 4 轮对抗）

| 轮次 | 问题 | 修复 | 结果 |
|------|------|------|------|
| 1 | yt-dlp 版本 2026.3.17 太旧 | `pip install -U yt-dlp` → 2026.6.9 | JS runtime 自动检测 Node.js |
| 2 | JS runtime 未指定 | Python 脚本加 `--js-runtimes node` | Node.js v24 可用 |
| 3 | JS 挑战未解 | `--remote-components ejs:github`（下载 JS 求解器） | 下载组件，但仍 429 |
| 4 | web 端 429 + SABR 实验 | iOS 客户端 | iOS 需要 PO Token |
| 5 | iOS 需 PO Token | `android,web` + `curl-cffi`（TLS 伪装） | **网络层通了，能拿到标题** |

第 5 轮是**关键突破**——`curl-cffi` 安装后，yt-dlp 能成功拿到视频元数据（标题、时长），说明 YouTube 反爬在 TLS 层被绕过了。

---

## 当前卡点：格式选择器不匹配

### 诊断发现

```bash
$ yt-dlp --extractor-args "youtube:player_client=android,web" \
    --remote-components ejs:github \
    --list-formats "https://www.youtube.com/watch?v=cLQoSpsK5Ek"

18  mp4   640x360   30  2 |  ~19MB   389k | 360p
```

**YouTube 对这个视频只暴露了格式 18**（360p 单文件 mp4，含视频+音频）。

### 我们的格式选择器

```
-f "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best"
```

- `bv*[ext=mp4]+ba[ext=m4a]` → 请求独立视频流(mp4) + 独立音频流(m4a) → **不存在**（只有合并的 18 号）
- 预期 fallback 到 `b[ext=mp4]` → 格式 18 → 应该匹配 → 但实际**没匹配上**
- `best` → 最终 fallback，但可能也只返回了 images/storyboards

### 核心矛盾

yt-dlp **exit code 0**（认为成功），但 `video.mp4` **文件不存在**（`media_file_ready` 返回 false）。说明 yt-dlp 没有真正下载到符合格式选择器的视频流，但认为任务"完成"了（可能只下载了元数据或 storyboard）。

### 其他发现

- FFmpeg 8.1.1 ✅ 正常（`ffmpeg -version` 确认）
- `media_file_ready` 逻辑正确（检查文件存在 + size > 0）
- 格式 18 是单文件 mp4，**不需要 FFmpeg 合并**——所以错误提示"音视频合并失败"是误导，实际是 yt-dlp 根本没下载到视频

---

## 待验证方案（明天继续）

### A. 简化格式选择器（最低风险）

```bash
# 当前
-f "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best"

# 改为
-f "best[ext=mp4]/best"    # 直接用 best mp4，格式 18 肯定匹配
```

### B. 不指定格式（让 yt-dlp 自己选）

```bash
# 完全去掉 -f，yt-dlp 默认行为：bestvideo+bestaudio/best
# 当前场景只有格式 18，yt-dlp 会直接用
```

### C. 指定格式 ID

```bash
# 明确用 18 号（仅限这个视频，不通用）
-f 18
```

推荐**先试 A 或 B**，因为当前问题是格式选择器太严格。

### D. 检查 yt-dlp verbose 输出

```bash
# 加 -v 看 yt-dlp 到底选了哪种格式、写到了哪个文件
yt-dlp -v --extractor-args "youtube:player_client=android,web" \
    --remote-components ejs:github \
    -f "best[ext=mp4]/best" \
    -o "video.mp4" "URL"
```

---

## 关联

- B站 412 诊断：[B站视频下载-412超时诊断.md](./B站视频下载-412超时诊断.md)
- 日志系统落地后，每次下载失败都有完整的 `[pipeline]` `[download]` 日志链，不再靠 `emit_progress` 猜根因
- `curl_cffi` 已装（TLS 伪装），为 B站 和 YouTube 后续都用得上

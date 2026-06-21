// ============================================================
// Tauri API 封装 — 类型安全的 invoke + 事件监听
// 浏览器 dev 模式下自动降级为 mock 实现
// ============================================================

// ---- 检测运行环境 ----
let tauriInvoke: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;
let tauriListen: ((event: string, handler: (payload: unknown) => void) => Promise<() => void>) | null = null;

async function ensureTauri() {
  if (tauriInvoke) return true;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const { listen } = await import("@tauri-apps/api/event");
    tauriInvoke = invoke;
    // Tauri 2.x wraps events in { payload, id }: unwrap before passing to handler
    tauriListen = (event: string, handler: (p: unknown) => void) =>
      listen<unknown>(event, (e) => handler(e.payload));
    return true;
  } catch {
    console.warn("[myriad-mind] Not running inside Tauri — using mock mode");
    return false;
  }
}

// ---- 依赖检测 ----

export interface DepResult {
  name: string;
  found: boolean;
  path?: string;
  version?: string;
  suggestion?: string;
}

export async function detectPython(): Promise<DepResult> {
  if (await ensureTauri()) return tauriInvoke!("detect_python") as Promise<DepResult>;
  return { name: "Python", found: false, suggestion: "未运行 Tauri 模式" };
}

export async function detectFfmpeg(): Promise<DepResult> {
  if (await ensureTauri()) return tauriInvoke!("detect_ffmpeg") as Promise<DepResult>;
  return { name: "FFmpeg", found: false, suggestion: "未运行 Tauri 模式" };
}

export async function detectYtdlp(): Promise<DepResult> {
  if (await ensureTauri()) return tauriInvoke!("detect_ytdlp") as Promise<DepResult>;
  return { name: "yt-dlp", found: false, suggestion: "未运行 Tauri 模式" };
}

export async function detectGpu(): Promise<DepResult> {
  if (await ensureTauri()) return tauriInvoke!("detect_gpu") as Promise<DepResult>;
  return { name: "GPU", found: false, suggestion: "未运行 Tauri 模式" };
}

export async function detectAllDeps(pythonPath?: string): Promise<Record<string, DepResult>> {
  if (await ensureTauri()) return tauriInvoke!("detect_all_deps", { pythonPath: pythonPath ?? null }) as Promise<Record<string, DepResult>>;
  return {
    python: { name: "Python", found: false },
    ffmpeg: { name: "FFmpeg", found: false },
    ytdlp: { name: "yt-dlp", found: false },
    gpu: { name: "GPU", found: false },
  };
}

// ---- 配置 ----

export interface ConfigInfo {
  path: string;
  exists: boolean;
  is_first_launch: boolean;
}

export async function getConfigInfo(): Promise<ConfigInfo> {
  if (await ensureTauri()) return tauriInvoke!("get_config_info") as Promise<ConfigInfo>;
  return { path: "", exists: false, is_first_launch: true };
}

export async function isFirstLaunch(): Promise<boolean> {
  if (await ensureTauri()) return tauriInvoke!("is_first_launch") as Promise<boolean>;
  return !localStorage.getItem("myriad-mind-configured");
}

export async function readConfig(): Promise<string> {
  if (await ensureTauri()) return tauriInvoke!("read_config") as Promise<string>;
  return localStorage.getItem("myriad-mind-config") ?? "{}";
}

export async function writeConfig(content: string): Promise<void> {
  if (await ensureTauri()) {
    await tauriInvoke!("write_config", { content });
    return;
  }
  localStorage.setItem("myriad-mind-config", content);
  localStorage.setItem("myriad-mind-configured", "true");
}

// ---- 文件系统 ----

export interface FileEntry {
  name: string;
  path: string;
  file_type: string;
  size_bytes: number;
}

export async function scanDirectory(dirPath: string): Promise<{ files: FileEntry[]; total_count: number }> {
  if (await ensureTauri()) return tauriInvoke!("scan_directory", { dirPath }) as Promise<{ files: FileEntry[]; total_count: number }>;
  return { files: [], total_count: 0 };
}

export async function readTextFile(filePath: string): Promise<string> {
  if (await ensureTauri()) return tauriInvoke!("read_text_file", { filePath }) as Promise<string>;
  return "";
}

// ---- 管线进度 ----

export interface PipelineProgressEvent {
  step: string;
  label: string;
  percent: number;
  status: "running" | "completed" | "failed";
  detail?: string;
}

export function listenPipelineProgress(
  onProgress: (event: PipelineProgressEvent) => void,
): () => void {
  let cancelled = false;

  ensureTauri().then((ok) => {
    if (!ok || cancelled) return;
    tauriListen!("pipeline-progress", (event: unknown) => {
      if (cancelled) return;
      onProgress(event as PipelineProgressEvent);
    });
  });

  return () => { cancelled = true; };
}

// ---- 管线 ----

export interface PipelineStep {
  name: string;
  label: string;
  percent: number;
  status: string;
}

export async function buildPipeline(mode: string): Promise<{ steps: PipelineStep[] }> {
  if (await ensureTauri()) return tauriInvoke!("build_pipeline", { mode }) as Promise<{ steps: PipelineStep[] }>;
  return { steps: [] };
}

// ---- Python 脚本 (6 个) ----

export async function checkPythonEnv(pythonPath: string): Promise<string> {
  if (await ensureTauri()) return tauriInvoke!("check_python_env", { pythonPath }) as Promise<string>;
  return "mock: Python 3.12.0";
}

export async function installFasterWhisper(
  pythonPath: string,
  venvDir?: string,
): Promise<{ venv_python: string; mirror: string }> {
  if (await ensureTauri()) return tauriInvoke!("install_faster_whisper", { pythonPath, venvDir: venvDir ?? null }) as Promise<{ venv_python: string; mirror: string }>;
  return { venv_python: "", mirror: "mock" };
}

export async function downloadYoutubeSubtitles(
  url: string,
  outputDir: string,
  pythonPath: string,
  languages?: string,
): Promise<unknown> {
  if (await ensureTauri()) return tauriInvoke!("download_youtube_subtitles", { url, outputDir, pythonPath, languages: languages ?? null });
  return null;
}

export async function downloadVideo(
  responseJsonPath: string,
  outputPath: string,
  pythonPath: string,
  timeout?: number,
): Promise<unknown> {
  if (await ensureTauri()) return tauriInvoke!("download_video", { responseJsonPath, outputPath, pythonPath, timeout: timeout ?? null });
  return null;
}

export async function listAiDouyinTasks(
  pythonPath: string,
  apiKey: string,
  opts?: { apiBase?: string; page?: number; pageSize?: number; status?: string; search?: string },
): Promise<unknown> {
  if (await ensureTauri()) return tauriInvoke!("list_ai_douyin_tasks", {
    pythonPath,
    apiKey,
    apiBase: opts?.apiBase ?? null,
    page: opts?.page ?? null,
    pageSize: opts?.pageSize ?? null,
    status: opts?.status ?? null,
    search: opts?.search ?? null,
  });
  return null;
}

// ---- 重置配置（删除 config.json，下次启动重新进入首启引导）----

export async function resetConfig(): Promise<void> {
  if (await ensureTauri()) {
    await tauriInvoke!("reset_config");
    return;
  }
  localStorage.removeItem("myriad-mind-config");
}

// ---- 打开外部链接（注册页等，调系统浏览器）----

export async function openExternalUrl(url: string): Promise<void> {
  if (await ensureTauri()) {
    try {
      // 官方插件 API（内部 invoke plugin:opener|open_url，已正确处理命令名/权限/scope）
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
      return;
    } catch (e) {
      console.warn("[myriad-mind] openUrl 失败，回退 window.open", e);
    }
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

// ---- 打开本地路径（输出目录等，调系统文件管理器）----

export async function openPath(path: string): Promise<void> {
  if (await ensureTauri()) {
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(path);
    } catch (e) {
      console.warn("[myriad-mind] openPath 失败", e);
    }
  }
}


export interface MindStreamEvent {
  type: "start" | "reasoning_delta" | "delta" | "usage" | "done" | "error";
  provider?: string;
  model?: string;
  delta?: string;
  text?: string;
  finish_reason?: string;
  // Tauri serializes Rust snake_case → JS receives snake_case
  input_tokens?: number;
  output_tokens?: number;
  reasoning_tokens?: number;
  total_tokens?: number;
  code?: string;
  message?: string;
  retryable?: boolean;
}

export function listenMindStream(
  onEvent: (event: MindStreamEvent) => void,
): () => void {
  let cancelled = false;
  ensureTauri().then((ok) => {
    if (!ok || cancelled) return;
    tauriListen!("mind-stream", (event: unknown) => {
      if (cancelled) return;
      onEvent(event as MindStreamEvent);
    });
  });
  return () => { cancelled = true; };
}

// ---- AI Tasks ----

export interface MindRequestPayload {
  task: string;
  messages: Array<{ role: string; content: string }>;
  system_prompt: string;
  model_override?: string;
  stream: boolean;
  max_tokens?: number;
  thinking?: { enabled: boolean; effort: "high" | "max" };
}

export async function runMindTask(request: MindRequestPayload): Promise<unknown> {
  if (await ensureTauri()) return tauriInvoke!("run_mind_task", { request }) as Promise<unknown>;
  await new Promise((r) => setTimeout(r, 2000));
  return { text: "# 模拟生成\n\n请在 Tauri 环境下运行。", provider: "deepseek", model: "mock" };
}

export async function pickFolder(): Promise<string | null> {
  if (await ensureTauri()) return tauriInvoke!("pick_folder") as Promise<string | null>;
  // Browser mock: return a fake path
  return "D:/Notes/MyriadMind";
}

export async function getCacheDir(): Promise<string> {
  if (await ensureTauri()) return tauriInvoke!("get_cache_dir") as Promise<string>;
  return "（浏览器 mock）";
}

export async function openCacheDir(): Promise<void> {
  if (await ensureTauri()) {
    await tauriInvoke!("open_cache_dir");
    return;
  }
  console.warn("[myriad-mind] open_cache_dir 仅在 Tauri 环境中可用");
}

export async function executeQa(notePath: string, question: string, writeBack: boolean): Promise<string> {
  if (await ensureTauri()) return tauriInvoke!("execute_qa", { notePath, question, writeBack }) as Promise<string>;
  return "（浏览器模拟）基于笔记内容的回答...";
}

export async function testDeepSeekConnection(): Promise<string> {
  if (await ensureTauri()) return tauriInvoke!("test_deepseek_connection") as Promise<string>;
  return "pong — deepseek-v4-flash (browser mock)";
}

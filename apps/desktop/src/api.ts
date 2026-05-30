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
    tauriListen = listen;
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

export async function detectAllDeps(): Promise<Record<string, DepResult>> {
  if (await ensureTauri()) return tauriInvoke!("detect_all_deps") as Promise<Record<string, DepResult>>;
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

// ---- Claude API (SSE 流式) ----

export interface ClaudeMessage {
  role: string;
  content: string;
}

export function listenClaudeStream(
  onDelta: (delta: string) => void,
  onDone: (fullText: string) => void,
): () => void {
  let cancelled = false;

  ensureTauri().then((ok) => {
    if (!ok || cancelled) return;
    // 注册 SSE 事件监听
    tauriListen!("claude-stream-delta", (event: unknown) => {
      if (cancelled) return;
      const payload = event as { delta: string };
      onDelta(payload.delta);
    });
  });

  return () => { cancelled = true; };
}

export async function streamNoteGeneration(
  messages: ClaudeMessage[],
  systemPrompt: string,
  apiKey: string,
): Promise<string> {
  if (await ensureTauri()) {
    return tauriInvoke!("stream_note_generation", {
      messages,
      systemPrompt,
      apiKey,
    }) as Promise<string>;
  }
  // Mock: simulate delay
  await new Promise((r) => setTimeout(r, 2000));
  return "模拟生成结果 — 请在 Tauri 环境下运行以体验完整功能";
}

import { useState, useCallback, useRef, useEffect } from "react";
import {
  type MyriadMindConfig,
  type SetupStatus,
  classifyInput,
  estimateCost,
} from "@myriad-mind/core";
import * as api from "@/api";
import { isTauri } from "@/lib/platform";
import type { LogEntry } from "@/components/log/LogPanel";

// ---- Types ----

interface UsePipelineOptions {
  config: MyriadMindConfig;
  setupStatus: SetupStatus;
}

interface UsePipelineResult {
  inputUrl: string;
  setInputUrl: (v: string) => void;
  noteCategory: string;
  setNoteCategory: (v: string) => void;
  taskPrompt: string;
  setTaskPrompt: (v: string) => void;
  status: string | null;
  progress: number;
  progressDetail: string | null;
  processing: boolean;
  logs: LogEntry[];
  streamingText: string;
  submit: () => Promise<void>;
}

// ---- Mock pipeline (browser dev) ----

async function runMockPipeline(
  inputUrl: string,
  pushLog: (type: LogEntry["type"], text: string) => void,
  setStatus: (s: string) => void,
  setProgress: (p: number) => void,
  setProgressDetail: (d: string | null) => void,
  setStreamingText: (fn: (prev: string) => string) => void,
  setProcessing: (v: boolean) => void,
) {
  const classify = classifyInput(inputUrl);

  const mockSteps = [
    { label: "识别输入模式", pct: 10 },
    { label: "检查环境依赖", pct: 25 },
    { label: "获取媒体内容", pct: 45 },
    { label: "AI 分析生成", pct: 70 },
    { label: "组装学习笔记", pct: 90 },
    { label: "更新修为面板", pct: 100 },
  ];

  for (const step of mockSteps) {
    await new Promise((r) => setTimeout(r, 350 + Math.random() * 250));
    setStatus(step.label);
    setProgress(step.pct);
    pushLog("step", `${step.label} … ${step.pct}%`);

    if (step.label === "识别输入模式") {
      pushLog("info", `  → 平台: ${classify.platform}, 模式: ${classify.mode}`);
    }
    if (step.label === "检查环境依赖") {
      pushLog("info", "  → Python 3.12.0 ✓   FFmpeg 8.1.1 ✓   yt-dlp 2025.5 ✓");
    }
    if (step.label === "获取媒体内容") {
      pushLog("info", `  → 正在下载 ${classify.platform} 内容 …`);
      await new Promise((r) => setTimeout(r, 400));
      pushLog("info", "  → 下载完成, 音频提取完成");
    }
    if (step.label === "AI 分析生成") {
      pushLog("info", "  → ASR 转写完成, 关键帧提取完成");
      pushLog("divider", "");
      pushLog("info", "  → 开始 AI 流式生成 …");

      const mockOutput = [
        "# 学习笔记\n\n",
        "## AI 摘要\n\n",
        "本视频深入讲解了 ",
        "现代前端框架的核心设计理念，",
        "包括响应式系统、虚拟 DOM diff 算法、\n",
        "以及编译时优化策略。\n\n",
        "## 核心概念\n\n",
        "1. **响应式原理** — Proxy + 依赖追踪\n",
        "2. **虚拟 DOM** — Tree diffing + Keyed patch\n",
        "3. **编译优化** — Static hoisting + Patch flags\n\n",
        "## Mermaid 知识关系图\n\n",
        "```mermaid\n",
        "graph TD\n",
        "  A[用户交互] --> B[响应式更新]\n",
        "  B --> C[虚拟 DOM Diff]\n",
        "  C --> D[最小化 DOM 操作]\n",
        "```\n",
      ];

      for (const chunk of mockOutput) {
        setStreamingText((prev) => prev + chunk);
        await new Promise((r) => setTimeout(r, 60 + Math.random() * 80));
      }

      pushLog("output", "… AI 输出完成（见上方流式内容）");
      setStreamingText(() => "");
      pushLog("divider", "");
    }
  }

  setProcessing(false);
  pushLog("divider", "");
  pushLog("success", `✅ 炼化完成 — 笔记已生成（浏览器模拟 · ${classify.platform}）`);
  setStatus("✅ 炼化完成 — 笔记已生成（浏览器模拟）");
  setProgressDetail(`识别为 ${classify.platform}，模拟耗时 ${(mockSteps.length * 0.5).toFixed(1)}s`);
}

function missingRuntimeDeps(
  mode: string,
  deps: Record<string, api.DepResult>,
): string[] {
  const missing: string[] = [];
  const needsMedia = ["bilibili", "youtube", "douyin", "xiaohongshu", "local_video", "local_audio"].includes(mode);
  const needsOnline = ["bilibili", "youtube", "douyin", "xiaohongshu"].includes(mode);
  const needsVideo = ["bilibili", "youtube", "douyin", "xiaohongshu", "local_video"].includes(mode);

  if (needsMedia && !deps.python?.found) missing.push("Python 3.9+");
  if (needsMedia && !deps["faster-whisper"]?.found) missing.push("faster-whisper");
  if (needsOnline && !deps["yt-dlp"]?.found) missing.push("yt-dlp");
  if (needsVideo && !deps.ffmpeg?.found) missing.push("FFmpeg");

  return missing;
}

// ---- Hook ----

export function usePipeline({ config, setupStatus }: UsePipelineOptions): UsePipelineResult {
  const [inputUrl, setInputUrl] = useState("");
  const [noteCategory, setNoteCategory] = useState("");
  const [taskPrompt, setTaskPrompt] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [progressDetail, setProgressDetail] = useState<string | null>(null);
  const [processing, setProcessing] = useState(false);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [streamingText, setStreamingText] = useState("");

  const logIdRef = useRef(0);
  const streamAccumRef = useRef("");
  const streamChunkedRef = useRef(0); // chars already pushed to logs
  const pipelineCancelRef = useRef<(() => void) | null>(null);
  const aiStartRef = useRef(0);
  const lastUsageRef = useRef<{ inputTokens?: number; outputTokens?: number; totalTokens?: number }>({});

  // Cleanup on unmount
  useEffect(() => {
    return () => { pipelineCancelRef.current?.(); };
  }, []);

  // ---- Log helpers ----

  const pushLog = useCallback((type: LogEntry["type"], text: string) => {
    logIdRef.current += 1;
    setLogs((prev) => [...prev, { id: logIdRef.current, type, text, timestamp: Date.now() }]);
  }, []);

  // ---- Submit ----

  const submit = useCallback(async () => {
    if (!inputUrl.trim() || processing) return;
    // 配置就绪守卫（浏览器 mock 模式放行，允许预览输入识别）
    if (await isTauri() && setupStatus !== "ready") {
      const msg = setupStatus === "needs_config"
        ? "需要先完成配置（DeepSeek API Key 等）。请到设置页配置。"
        : setupStatus === "invalid_config"
          ? "配置文件无效，请到设置页修复。"
          : "配置未就绪，请稍候或检查设置。";
      setStatus(`❌ ${msg}`);
      setProgressDetail(msg);
      pushLog("error", msg);
      return;
    }
    const classify = classifyInput(inputUrl.trim());
    const estimate = estimateCost(classify, config);

    if (await isTauri()) {
      const deps = await api.detectAllDeps(config.python_path || undefined);
      const missing = missingRuntimeDeps(classify.mode, deps);
      if (missing.length > 0) {
        const message = `运行环境未就绪，缺少：${missing.join("、")}。请到设置页修复依赖后重新检测。`;
        setStatus(`❌ ${message}`);
        setProgressDetail(message);
        setProcessing(false);
        logIdRef.current = 0;
        setLogs([]);
        pushLog("error", message);
        return;
      }
    }

    setStatus(
      `${classify.platform} · 预估 ${estimate.estimatedMinutes} 分钟 · ${Math.round((estimate.inputTokens + estimate.outputTokens) / 1000)}k tokens`
    );
    setProgressDetail(null);
    setProcessing(true);
    setProgress(0);
    setStreamingText("");
    streamAccumRef.current = "";
    streamChunkedRef.current = 0;

    // Reset logs for new run
    logIdRef.current = 0;
    setLogs([]);

    pushLog("info", `输入: ${inputUrl.trim()}`);
    pushLog("step", `模式识别 → ${classify.platform} (${classify.mode})`);
    pushLog("info", `预估灵力消耗: ~${estimate.estimatedMinutes} 分钟 · ${Math.round((estimate.inputTokens + estimate.outputTokens) / 1000)}k tokens`);
    pushLog("divider", "");

    if (await isTauri()) {
      // ---- Tauri real pipeline ----
      let pipelineDone = false;
      let unlistenStream: () => void = () => {};

      const finishPipeline = () => {
        if (pipelineDone) return;
        pipelineDone = true;
        setProcessing(false);
        unlisten();
        unlistenStream();
        pipelineCancelRef.current = null;
      };

      const unlisten = api.listenPipelineProgress((event) => {
        const pct = Math.round(event.percent);
        setProgress(isNaN(pct) ? 0 : pct);
        setStatus(event.label);
        if (event.detail) setProgressDetail(event.detail);

        if (event.status === "running") {
          pushLog("step", event.label);
        } else if (event.status === "completed") {
          pushLog("step", event.label);
        }
        if (event.detail && event.status !== "failed") {
          pushLog("info", event.detail);
        }

        // Failures: only stop on explicit failure
        if (event.status === "failed") {
          pushLog("error", `❌ ${event.label}`);
          setProgressDetail(`❌ ${event.label}`);
          finishPipeline();
        }
      });

      // Listen for mind-stream (DeepSeek unified events)
      const AI_CHUNK_SIZE = 2000; // 每积累 2000 字符推一条日志
      unlistenStream = api.listenMindStream((event) => {
        switch (event.type) {
          case "start":
            aiStartRef.current = Date.now();
            lastUsageRef.current = {};
            streamAccumRef.current = "";
            streamChunkedRef.current = 0;
            pushLog("step", `🤖 AI 开始生成 · ${event.model ?? ""}`);
            pushLog("divider", "");
            break;
          case "delta": {
            streamAccumRef.current += (event.delta ?? "");
            const unlogged = streamAccumRef.current.length - streamChunkedRef.current;
            // 积累足够字符后推一条日志归档
            if (unlogged >= AI_CHUNK_SIZE) {
              const chunk = streamAccumRef.current.slice(streamChunkedRef.current);
              pushLog("output", chunk);
              streamChunkedRef.current = streamAccumRef.current.length;
              // streamingText 显示当前未归档部分
              setStreamingText(
                streamAccumRef.current.slice(streamChunkedRef.current)
              );
            } else {
              // streamingText 只显示未归档的增量
              setStreamingText(
                streamAccumRef.current.slice(streamChunkedRef.current)
              );
            }
            break;
          }
          case "reasoning_delta":
            // 思考过程单独累计，不进入正文
            break;
          case "usage":
            lastUsageRef.current = {
              inputTokens: event.input_tokens,
              outputTokens: event.output_tokens,
              totalTokens: event.total_tokens,
            };
            break;
          case "done": {
            // 归档剩余未归档文本
            const remaining = streamAccumRef.current.slice(streamChunkedRef.current);
            if (remaining) {
              pushLog("output", remaining);
            }
            const totalChars = streamAccumRef.current.length;
            const elapsed = ((Date.now() - aiStartRef.current) / 1000).toFixed(1);
            const u = lastUsageRef.current;
            const summaryParts = [`共 ${totalChars} 字符`, `⏱️ ${elapsed}s`];
            if (u.totalTokens) summaryParts.push(`📊 ${(u.totalTokens / 1000).toFixed(1)}K tokens`);
            pushLog("divider", "");
            pushLog("success", `✅ 笔记生成完成 · ${summaryParts.join(" · ")}`);
            streamAccumRef.current = "";
            streamChunkedRef.current = 0;
            setStreamingText("");
            setProgress(100);
            finishPipeline();
            break;
            }
          case "error":
            pushLog("error", `❌ ${event.message ?? "未知错误"}`);
            finishPipeline();
            break;
        }
      });

      pipelineCancelRef.current = () => {
        unlisten();
        unlistenStream();
      };

      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke("execute_pipeline", {
          input: inputUrl.trim(),
          mode: classify.mode,
          pythonPath: config.python_path || null,
          noteDir: config.output.note_dir || "",
          noteCategory: noteCategory || null,
          taskPrompt: taskPrompt || null,
          debugMetadata: config.output.debug_metadata ?? false,
          cleanupTemp: config.output.cleanup_temp ?? true,
        }) as { success: boolean; mode: string; duration_seconds: number };
        if (result.success) {
          setProgress(100);
          setStatus(`✅ 炼化完成 — ${result.mode} · ${result.duration_seconds.toFixed(1)}s`);
        }
      } catch (e) {
        pushLog("error", `管线执行失败: ${e}`);
        setStatus(`❌ 管线执行失败: ${e}`);
        setProcessing(false);
        unlisten();
        unlistenStream();
      }
    } else {
      // ---- Mock pipeline (browser dev) ----
      await runMockPipeline(
        inputUrl.trim(),
        pushLog,
        setStatus,
        setProgress,
        setProgressDetail,
        setStreamingText,
        setProcessing,
      );
    }
  }, [inputUrl, config, setupStatus, processing, pushLog]);

  return {
    inputUrl,
    setInputUrl,
    noteCategory,
    setNoteCategory,
    taskPrompt,
    setTaskPrompt,
    status,
    progress,
    progressDetail,
    processing,
    logs,
    streamingText,
    submit,
  };
}

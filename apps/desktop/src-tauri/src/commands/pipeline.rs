// ============================================================
// 管线编排 — 多步骤管线控制
// 与 docs/architecture.md §2.2 对齐
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InputMode {
    #[serde(rename = "bilibili")]
    Bilibili,
    #[serde(rename = "youtube")]
    Youtube,
    #[serde(rename = "douyin")]
    Douyin,
    #[serde(rename = "xiaohongshu")]
    Xiaohongshu,
    #[serde(rename = "article_url")]
    ArticleUrl,
    #[serde(rename = "local_video")]
    LocalVideo,
    #[serde(rename = "local_audio")]
    LocalAudio,
    #[serde(rename = "local_text")]
    LocalText,
}

impl std::fmt::Display for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputMode::Bilibili => write!(f, "B 站视频"),
            InputMode::Youtube => write!(f, "YouTube 视频"),
            InputMode::Douyin => write!(f, "抖音/TikTok"),
            InputMode::Xiaohongshu => write!(f, "小红书"),
            InputMode::ArticleUrl => write!(f, "在线文章"),
            InputMode::LocalVideo => write!(f, "本地视频"),
            InputMode::LocalAudio => write!(f, "本地音频"),
            InputMode::LocalText => write!(f, "本地文档"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineStep {
    pub name: String,
    pub label: String,
    pub percent: f64,
    pub status: String, // "pending" | "running" | "completed" | "failed"
}

#[derive(Debug, Serialize)]
pub struct PipelineState {
    pub mode: InputMode,
    pub steps: Vec<PipelineStep>,
    pub current_step: usize,
    pub is_complete: bool,
    pub error: Option<String>,
}

/// 根据输入模式生成管线步骤列表
#[tauri::command]
pub async fn build_pipeline(mode: InputMode) -> Result<PipelineState, AppError> {
    let steps = match mode {
        InputMode::Bilibili
        | InputMode::Youtube
        | InputMode::Douyin
        | InputMode::Xiaohongshu => video_pipeline_steps(),
        InputMode::ArticleUrl => article_pipeline_steps(),
        InputMode::LocalVideo => local_video_pipeline_steps(),
        InputMode::LocalAudio => local_audio_pipeline_steps(),
        InputMode::LocalText => text_pipeline_steps(),
    };

    Ok(PipelineState {
        mode,
        steps,
        current_step: 0,
        is_complete: false,
        error: None,
    })
}

fn video_pipeline_steps() -> Vec<PipelineStep> {
    vec![
        PipelineStep {
            name: "mode_detected".into(),
            label: "识别输入".into(),
            percent: 0.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "deps_checked".into(),
            label: "环境检查".into(),
            percent: 5.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "estimation".into(),
            label: "灵力预估".into(),
            percent: 10.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "download_video".into(),
            label: "下载视频".into(),
            percent: 25.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "extract_audio".into(),
            label: "提取音频".into(),
            percent: 40.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "transcribe_audio".into(),
            label: "语音转写".into(),
            percent: 60.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "extract_keyframes".into(),
            label: "提取关键帧".into(),
            percent: 75.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "generate_note".into(),
            label: "AI 生成笔记".into(),
            percent: 90.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "cleanup".into(),
            label: "清理临时文件".into(),
            percent: 95.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "completed".into(),
            label: "完成".into(),
            percent: 100.0,
            status: "pending".into(),
        },
    ]
}

fn article_pipeline_steps() -> Vec<PipelineStep> {
    vec![
        PipelineStep {
            name: "mode_detected".into(),
            label: "识别输入".into(),
            percent: 0.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "fetch_content".into(),
            label: "抓取内容".into(),
            percent: 20.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "analyze".into(),
            label: "AI 分析".into(),
            percent: 50.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "generate_note".into(),
            label: "生成笔记".into(),
            percent: 80.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "completed".into(),
            label: "完成".into(),
            percent: 100.0,
            status: "pending".into(),
        },
    ]
}

fn local_video_pipeline_steps() -> Vec<PipelineStep> {
    // 类似 video_pipeline_steps 但跳过下载
    video_pipeline_steps()
        .into_iter()
        .filter(|s| s.name != "download_video")
        .map(|s| PipelineStep {
            percent: if s.name == "extract_audio" {
                15.0
            } else if s.name == "transcribe_audio" {
                35.0
            } else if s.name == "extract_keyframes" {
                55.0
            } else if s.name == "generate_note" {
                80.0
            } else if s.name == "completed" {
                100.0
            } else {
                s.percent
            },
            ..s
        })
        .collect()
}

fn local_audio_pipeline_steps() -> Vec<PipelineStep> {
    vec![
        PipelineStep {
            name: "mode_detected".into(),
            label: "识别输入".into(),
            percent: 0.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "transcribe_audio".into(),
            label: "语音转写".into(),
            percent: 30.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "generate_note".into(),
            label: "AI 生成笔记".into(),
            percent: 75.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "completed".into(),
            label: "完成".into(),
            percent: 100.0,
            status: "pending".into(),
        },
    ]
}

fn text_pipeline_steps() -> Vec<PipelineStep> {
    vec![
        PipelineStep {
            name: "mode_detected".into(),
            label: "识别输入".into(),
            percent: 0.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "read_content".into(),
            label: "读取内容".into(),
            percent: 10.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "analyze".into(),
            label: "AI 分析".into(),
            percent: 40.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "generate_note".into(),
            label: "生成笔记".into(),
            percent: 80.0,
            status: "pending".into(),
        },
        PipelineStep {
            name: "completed".into(),
            label: "完成".into(),
            percent: 100.0,
            status: "pending".into(),
        },
    ]
}

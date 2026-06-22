// ============================================================
// handlers — Agent 工具 handler 集合，按阶段分文件
// acquire（获取）/ analyze（理解）/ io（生成·自检）
// 每个 handler 是一个 unit struct，实现 super::ToolHandler，注册到 super::registry。
// ============================================================

pub mod acquire;
pub mod analyze;
pub mod io;

// 统一 re-export，供 registry 按名字注册。
pub use acquire::{
    DownloadSubtitlesHandler, DownloadVideoHandler, ExtractAudioHandler, FetchUrlHandler,
    QueryAiDouyinHandler, ReadFileHandler, ScanCodeProjectHandler, ScanDirectoryHandler,
    TranscribeAsrHandler,
};
pub use analyze::{ExtractKeyframesHandler, ReviewKeyframesHandler};
pub use io::{ReadArtifactHandler, WriteNoteHandler};

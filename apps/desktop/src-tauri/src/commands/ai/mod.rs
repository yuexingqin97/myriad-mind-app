pub mod deepseek;
pub mod engine;
pub mod types;
pub mod vision;

pub use engine::{generate_note, qa_note, run_mind_task, test_deepseek_connection};

pub mod types;
pub mod deepseek;
pub mod engine;

pub use engine::{generate_note, qa_note, run_mind_task, test_deepseek_connection};

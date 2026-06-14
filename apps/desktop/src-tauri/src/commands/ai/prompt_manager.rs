// ============================================================
// PromptManager — 运行时从 prompts/ 加载 .md 模板并用 minijinja 渲染
// 职责: 定位 prompts/ 目录 → 注册全部 .md 为模板 → render(key, vars)
// 路径解析复用 python.rs 的多策略探测（dev/打包通吃）
// ============================================================

use crate::error::AppError;
use minijinja::Environment;
use std::path::{Path, PathBuf};

/// 哨兵文件：用于确认找到的就是真正的 prompts/ 目录
const SENTINEL: &str = "note/system.md";

/// prompts/ 目录路径
///
/// 开发模式: 从 CWD (apps/desktop/src-tauri/) 找 prompts/
/// 生产模式: 从可执行文件同级 / Resources/ / resources/ 查找 prompts/
fn prompts_dir() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = find_prompts_dir_from(&cwd) {
            return found;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(found) = find_prompts_dir_from(exe_dir) {
                return found;
            }
        }
    }

    // 兜底：相对路径（让后续报错带上下文）
    PathBuf::from("prompts")
}

fn find_prompts_dir_from(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        for candidate in [
            dir.join("prompts"),
            dir.join("Resources").join("prompts"),
            dir.join("resources").join("prompts"),
        ] {
            if candidate.join(SENTINEL).exists() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 提示词模板管理器
///
/// 构造时扫描 prompts/ 注册全部 .md 文件，key = 相对路径去掉 .md 后缀
/// （如 `note/system`、`vision/review_system`）。模板内可用 minijinja 全部语法：
/// `{{ var }}` 变量、`{% if %}` 条件、`{% include %}` 引入子模板。
pub struct PromptManager {
    env: Environment<'static>,
}

impl PromptManager {
    /// 扫描 prompts/ 注册所有模板。目录缺失或模板语法错误时返回 AppError。
    pub fn new() -> Result<Self, AppError> {
        let dir = prompts_dir();
        let mut env = Environment::new();
        // trim_blocks: 去掉块标签({% %})后的换行；lstrip_blocks: 去掉块标签前的行首空白
        // 让 {% if %} 块单独成行时不污染渲染输出的换行结构
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        Self::register_dir(&mut env, &dir, "")?;
        Ok(Self { env })
    }

    fn register_dir(
        env: &mut Environment<'static>,
        base: &Path,
        prefix: &str,
    ) -> Result<(), AppError> {
        if !base.exists() {
            return Err(AppError::Other(format!(
                "prompts 目录不存在: {}（期望包含 {}）",
                base.display(),
                SENTINEL
            )));
        }
        let entries = std::fs::read_dir(base).map_err(|e| {
            AppError::Other(format!("读取 prompts 目录失败 {}: {e}", base.display()))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_key = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };

            if path.is_dir() {
                Self::register_dir(env, &path, &rel_key)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let source = std::fs::read_to_string(&path).map_err(|e| {
                    AppError::Other(format!("读取模板失败 {}: {e}", path.display()))
                })?;
                let key = rel_key.strip_suffix(".md").unwrap_or(&rel_key);
                env.add_template_owned(key.to_string(), source).map_err(|e| {
                    AppError::Other(format!("模板编译失败 '{key}': {e}"))
                })?;
            }
        }
        Ok(())
    }

    /// 按 key 渲染模板。`ctx` 接受任何 serde::Serialize，推荐用 `minijinja::context!` 宏构造。
    pub fn render<T: serde::Serialize>(&self, key: &str, ctx: T) -> Result<String, AppError> {
        let tmpl = self
            .env
            .get_template(key)
            .map_err(|e| AppError::Other(format!("模板 '{key}' 未找到: {e}")))?;
        tmpl.render(ctx)
            .map_err(|e| AppError::Other(format!("模板 '{key}' 渲染失败: {e}")))
    }
}

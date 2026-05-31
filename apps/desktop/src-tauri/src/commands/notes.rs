// ============================================================
// 笔记存储 — 自动分类 + 文末元信息块 + 版本追踪
// 元信息格式: <!-- MYRIAD_MIND_METADATA_START -->```yaml...```<!-- END -->
// ============================================================

use crate::error::AppError;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

// ---- 数据结构 ----

#[derive(Debug, Clone, Serialize)]
pub struct NoteSaveResult {
    pub path: String,
    pub title: String,
    pub category: String,
    pub version: u32,
}

// ---- 关键词 → 分类映射 ----

fn keyword_categories() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("Rust", vec!["rust", "cargo", "tokio", "serde", "bevy", "tauri", "ownership", "borrow", "lifetime", "async", "trait"]),
        ("AI", vec!["ai", "llm", "deepseek", "claude", "gpt", "openai", "transformer", "prompt", "embedding", "rag", "agent"]),
        ("前端", vec!["react", "vue", "javascript", "typescript", "css", "html", "frontend", "vite", "webpack"]),
        ("后端", vec!["api", "server", "database", "sql", "postgres", "redis", "grpc", "rest", "graphql"]),
        ("游戏开发", vec!["unreal", "unity", "ue5", "gdc", "gameplay", "shader", "rendering", "blueprint"]),
        ("DevOps", vec!["docker", "kubernetes", "ci", "cd", "git", "linux", "deploy", "nginx"]),
        ("Python", vec!["python", "django", "flask", "pytorch", "pandas", "numpy"]),
        ("CS基础", vec!["algorithm", "数据结构", "操作系统", "网络", "编译", "os", "compiler", "memory"]),
    ]
}

// ---- 主入口 ----

/// 保存 AI 生成的笔记。结构: base_dir/分类/标题.md + base_dir/分类/assets/
pub fn save_note(
    ai_output: &str,
    source_input: &str,
    source_type: &str,
    base_dir: &str,
    user_category: Option<&str>,
    debug_metadata: bool,
) -> Result<NoteSaveResult, AppError> {
    let title = extract_title(ai_output).unwrap_or_else(|| "未命名笔记".to_string());
    let safe_name = title_to_slug(&title);

    let category = if let Some(cat) = user_category {
        cat.to_string()
    } else {
        detect_category(ai_output, base_dir)
    };

    let cat_dir = PathBuf::from(base_dir).join(&category);
    std::fs::create_dir_all(&cat_dir)
        .map_err(|e| AppError::Config(format!("创建分类目录失败: {e}")))?;
    std::fs::create_dir_all(cat_dir.join("assets")).ok();

    let note_path = resolve_path(&cat_dir, &safe_name);

    let old_content = if note_path.exists() {
        std::fs::read_to_string(&note_path).unwrap_or_default()
    } else {
        String::new()
    };

    let version = if old_content.is_empty() { 1 } else { read_version(&old_content) + 1 };
    let is_new = version == 1;
    let fingerprint = simple_hash(source_input);

    // 提取旧内容中的用户数据
    let old_qa = extract_qa_section(&old_content);
    let old_update_rows = if !is_new { extract_update_rows(&old_content) } else { String::new() };

    // 构建更新记录表格行
    let now = timestamp_now();
    let new_row = if is_new {
        format!("| {now} | 初次炼化 · 来源: {source_input} |")
    } else {
        format!("| {now} | 重新炼化 · 来源: {source_input} |")
    };
    let update_rows = if old_update_rows.is_empty() {
        format!("| 更新时间 | 更新内容 |\n|----------|----------|\n{new_row}")
    } else {
        format!("| 更新时间 | 更新内容 |\n|----------|----------|\n{old_update_rows}\n{new_row}")
    };

    // 构建调试信息
    let debug_section = if debug_metadata {
        format!("\n### 调试信息\n\n> 以下信息用于排查处理链路。\n\n| 项目 | 值 |\n|------|----|\n| App 版本 | 0.1.0-alpha.1 |\n| AI Provider | deepseek |\n| AI Model | deepseek-v4-pro |\n| 输出分类 | {category} |\n| 输入类型 | {source_type} |\n| Fingerprint | {fingerprint} |\n| 输出目录 | {base_dir} |\n")
    } else {
        String::new()
    };

    // 生成文末元信息块
    let metadata_block = generate_metadata_block(
        &title, &category, version, source_type, source_input, &fingerprint,
    );

    // 组装最终内容: 正文 + ## 大衍决心得 (更新记录 + 问答 + 元信息 + 调试信息)
    let mut final_content = ai_output.to_string();
    final_content.push_str("\n\n---\n\n## 大衍决心得\n\n### 更新记录\n\n");
    final_content.push_str(&update_rows);

    if !old_qa.is_empty() {
        final_content.push_str(&format!("\n\n### 问答记录\n\n{old_qa}"));
    }
    final_content.push_str(&format!("\n\n### 元信息\n\n> 以下为应用读取用元信息。手动编辑可能影响去重、追问和修为统计。\n\n{metadata_block}"));
    if !debug_section.is_empty() {
        final_content.push_str(&debug_section);
    }

    std::fs::write(&note_path, &final_content)
        .map_err(|e| AppError::Config(format!("写入笔记失败: {e}")))?;

    log::info!("[notes] saved: {}/{safe_name}.md v{version}", cat_dir.display());

    Ok(NoteSaveResult {
        path: note_path.to_string_lossy().to_string(),
        title,
        category,
        version,
    })
}

fn resolve_path(cat_dir: &std::path::Path, base_name: &str) -> PathBuf {
    let primary = cat_dir.join(format!("{base_name}.md"));
    if !primary.exists() { return primary; }
    for i in 2u32.. {
        let alt = cat_dir.join(format!("{base_name}-{i}.md"));
        if !alt.exists() { return alt; }
    }
    primary
}

// ---- 辅助函数 ----

fn extract_title(markdown: &str) -> Option<String> {
    markdown.lines()
        .find(|l| l.trim().starts_with("# "))
        .map(|l| l.trim()[2..].trim().to_string())
}

fn title_to_slug(title: &str) -> String {
    let slug = title.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != ' ', "")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string();
    if slug.len() > 64 { slug[..64].to_string() } else { slug }
}

fn detect_category(content: &str, base_dir: &str) -> String {
    let content_lower = content.to_lowercase();
    let existing = scan_categories(base_dir);
    let mut scores: HashMap<String, u32> = HashMap::new();

    for (cat, keywords) in keyword_categories() {
        let score = keywords.iter().filter(|kw| content_lower.contains(*kw)).count() as u32;
        if score > 0 { scores.insert(cat.to_string(), score); }
    }

    if let Some(best) = scores.iter().max_by_key(|&(_, s)| s) {
        for existing_cat in &existing {
            if existing_cat.to_lowercase() == best.0.to_lowercase() {
                return existing_cat.clone();
            }
        }
        return best.0.clone();
    }

    if content_lower.contains("rust") { return "Rust".into(); }
    if content_lower.contains("ai") || content_lower.contains("deepseek") { return "AI".into(); }
    "未分类".into()
}

fn scan_categories(base_dir: &str) -> Vec<String> {
    let mut cats = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') { cats.push(name); }
            }
        }
    }
    cats
}

fn read_version(content: &str) -> u32 {
    if let Some(block) = extract_metadata_block(content) {
        for line in block.lines() {
            if line.trim().starts_with("current_version:") {
                return line.split(':').nth(1).and_then(|v| v.trim().parse().ok()).unwrap_or(1);
            }
        }
    }
    // Fallback: count update table rows
    if let Some(section) = content.find("### 更新记录") {
        let after = &content[section..];
        return after.matches("| ").count().saturating_sub(2).max(1) as u32;
    }
    1
}

/// 提取旧笔记中的问答记录
fn extract_qa_section(content: &str) -> String {
    if let Some(start) = content.find("### 问答记录") {
        let after = &content[start + "### 问答记录".len()..];
        if let Some(end) = after.find("\n### ") {
            return after[..end].trim().to_string();
        }
        if let Some(end) = after.find("\n<!-- MYRIAD_MIND_METADATA") {
            return after[..end].trim().to_string();
        }
        return after.trim().to_string();
    }
    String::new()
}

/// 提取旧笔记更新记录表格中的数据行（去掉表头）
fn extract_update_rows(content: &str) -> String {
    if let Some(section) = content.find("### 更新记录") {
        let after = &content[section + "### 更新记录".len()..];
        // Find table rows: lines starting with |
        let rows: Vec<&str> = after.lines()
            .filter(|l| l.trim().starts_with('|') && !l.contains("---") && !l.contains("更新时间"))
            .collect();
        return rows.join("\n");
    }
    String::new()
}

fn extract_metadata_block(content: &str) -> Option<String> {
    let start_marker = "<!-- MYRIAD_MIND_METADATA_START -->";
    let end_marker = "<!-- MYRIAD_MIND_METADATA_END -->";
    if let Some(start) = content.rfind(start_marker) {
        let after = &content[start + start_marker.len()..];
        if let Some(end) = after.find(end_marker) {
            let block = &after[..end];
            // Extract yaml from code fence
            if let Some(yaml_start) = block.find("```yaml") {
                let yaml = &block[yaml_start + 7..];
                if let Some(yaml_end) = yaml.find("```") {
                    return Some(yaml[..yaml_end].trim().to_string());
                }
            }
            return Some(block.trim().to_string());
        }
    }
    None
}

fn generate_metadata_block(
    title: &str, category: &str, version: u32,
    source_type: &str, source_raw: &str, fingerprint: &str,
) -> String {
    let now = timestamp_now();
    format!(
        "<!-- MYRIAD_MIND_METADATA_START -->\n\
         ```yaml\n\
         schema: myriad-mind-note/v1\n\
         id: note_{fingerprint}\n\
         title: {title}\n\
         category: {category}\n\
         slug: {slug}\n\
         created_at: {now}\n\
         updated_at: {now}\n\
         current_version: {version}\n\
         app:\n\
           name: myriad-mind-app\n\
           version: 0.1.0-alpha.1\n\
           build: 20260531.1\n\
           platform: windows\n\
         pipeline:\n\
           schema_version: 1\n\
           mode: note_generation\n\
           intensity: standard\n\
         sources:\n\
           - type: {source_type}\n\
             raw: {source_raw}\n\
             canonical: {source_type}:{fingerprint}\n\
             fingerprint: sha256:{fingerprint}\n\
             title: {title}\n\
             added_at: {now}\n\
         ai:\n\
           provider: deepseek\n\
           model: deepseek-v4-pro\n\
           role: primary\n\
           generated_at: {now}\n\
           api_style: openai_compatible\n\
           prompt_preset: note_generation/v1\n\
           prompt_hooks:\n\
             note_generation: true\n\
         ```\n\
         <!-- MYRIAD_MIND_METADATA_END -->",
        slug = title_to_slug(title),
    )
}

fn timestamp_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Convert to days since epoch, then to YMD
    let days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mon <= 2 { y + 1 } else { y };
    format!("{year:04}-{mon:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// ============================================================
// 笔记存储 — 自动分类 + 目录结构 + Front Matter + 版本追踪
// ============================================================

use crate::error::AppError;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

// ---- 数据结构 ----

#[derive(Debug, Clone, Serialize)]
pub struct NoteMeta {
    pub title: String,
    pub category: String,
    pub slug: String,
    pub target_dir: PathBuf,
    pub version: u32,
    pub is_new: bool,
}

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

/// 保存 AI 生成的笔记到分类目录
pub fn save_note(
    ai_output: &str,
    source_input: &str,
    source_type: &str, // "bilibili", "file", "article", etc.
    base_dir: &str,
    user_category: Option<&str>,
) -> Result<NoteSaveResult, AppError> {
    // 1. 提取标题
    let title = extract_title(ai_output).unwrap_or_else(|| "未命名笔记".to_string());
    let slug = title_to_slug(&title);

    // 2. 检测分类
    let category = if let Some(cat) = user_category {
        cat.to_string()
    } else {
        detect_category(ai_output, base_dir)
    };

    // 3. 创建目录结构
    let target_dir = PathBuf::from(base_dir).join(&category).join(&slug);
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| AppError::Config(format!("创建笔记目录失败: {e}")))?;

    // 4. 版本检测
    let index_path = target_dir.join("index.md");
    let version = if index_path.exists() {
        read_current_version(&index_path) + 1
    } else {
        1
    };
    let is_new = version == 1;

    // 5. 生成完整内容
    let now = chrono_now();
    let fingerprint = simple_hash(source_input);
    let front_matter = generate_frontmatter(
        &title, &category, version, source_type, source_input, &fingerprint, &now,
    );

    let version_entry = if is_new {
        format!("\n\n---\n\n## 更新记录\n\n### v1 · {now} · 初次炼化\n\n**来源：** {source_input}\n\n**本次内容：**\n- 初次生成结构化笔记。\n")
    } else {
        let existing = if index_path.exists() {
            std::fs::read_to_string(&index_path).unwrap_or_default()
        } else {
            String::new()
        };
        let body = strip_version_section(&existing);
        format!("\n\n---\n\n## 更新记录\n\n{body}\n### v{version} · {now} · 重新炼化\n\n**来源：** {source_input}\n\n**变化：**\n- 基于同一输入重新炼化，更新内容。\n")
    };

    let final_content = format!("{front_matter}\n\n{ai_output}{version_entry}");

    // 6. 写入
    std::fs::write(&index_path, &final_content)
        .map_err(|e| AppError::Config(format!("写入笔记失败: {e}")))?;

    // 7. 创建 assets 目录
    let _ = std::fs::create_dir_all(target_dir.join("assets"));

    log::info!(
        "[notes] saved: category={category}, slug={slug}, v{version}, path={}",
        index_path.display()
    );

    Ok(NoteSaveResult {
        path: index_path.to_string_lossy().to_string(),
        title,
        category,
        version,
    })
}

// ---- 辅助函数 ----

/// 从 Markdown 提取第一个 # 标题
fn extract_title(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].trim().to_string());
        }
    }
    None
}

/// 标题 → URL slug
fn title_to_slug(title: &str) -> String {
    let slug = title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != ' ', "")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string();
    if slug.len() > 60 { slug[..60].to_string() } else { slug }
}

/// 根据内容关键词 + 已有目录匹配分类
fn detect_category(content: &str, base_dir: &str) -> String {
    let content_lower = content.to_lowercase();

    // 扫描已有分类目录
    let existing = scan_categories(base_dir);

    // 关键词匹配并计分
    let mut scores: HashMap<String, u32> = HashMap::new();
    for (category, keywords) in keyword_categories() {
        let mut score = 0u32;
        for kw in &keywords {
            if content_lower.contains(kw) {
                score += 1;
            }
        }
        if score > 0 {
            scores.insert(category.to_string(), score);
        }
    }

    // 优先匹配已有目录
    if let Some(best) = scores.iter().max_by_key(|&(_, s)| s) {
        let cat_name = best.0;
        // 检查是否有精确匹配的已有目录
        for existing_cat in &existing {
            if existing_cat.to_lowercase() == cat_name.to_lowercase() {
                return existing_cat.clone(); // 保留已有目录的大小写
            }
        }
        return cat_name.clone();
    }

    // 标题关键词 fallback
    if content_lower.contains("rust") { return "Rust".into(); }
    if content_lower.contains("ai") || content_lower.contains("deepseek") { return "AI".into(); }

    "未分类".into()
}

/// 扫描输出目录下已有的一级分类目录
fn scan_categories(base_dir: &str) -> Vec<String> {
    let mut cats = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    cats.push(name);
                }
            }
        }
    }
    cats
}

/// 从现有 index.md 读取版本号
fn read_current_version(index_path: &std::path::Path) -> u32 {
    if let Ok(content) = std::fs::read_to_string(index_path) {
        for line in content.lines() {
            if line.starts_with("current_version:") {
                if let Some(v) = line.split(':').nth(1) {
                    return v.trim().parse::<u32>().unwrap_or(1);
                }
            }
        }
    }
    1
}

/// 从已有笔记中剥离版本记录段落（只保留 ## 更新记录 之前的部分）
fn strip_version_section(content: &str) -> String {
    if let Some(pos) = content.find("\n## 更新记录") {
        let after = &content[pos + "\n## 更新记录".len()..];
        // 提取已有版本条目
        after.trim().to_string()
    } else {
        String::new()
    }
}

/// 生成 YAML front matter
fn generate_frontmatter(
    title: &str,
    category: &str,
    version: u32,
    source_type: &str,
    source_raw: &str,
    fingerprint: &str,
    now: &str,
) -> String {
    format!(
        "---\n\
         id: note_{fingerprint}\n\
         title: {title}\n\
         category: {category}\n\
         slug: {slug}\n\
         source_type: {source_type}\n\
         source_raw: {source_raw}\n\
         source_fingerprint: sha256:{fingerprint}\n\
         current_version: {version}\n\
         created_at: {now}\n\
         updated_at: {now}\n\
         ai_provider: deepseek\n\
         ai_model: deepseek-v4-pro\n\
         ---",
        slug = title_to_slug(title),
    )
}

// ---- 工具 ----

fn chrono_now() -> String {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{ts}")
}

fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

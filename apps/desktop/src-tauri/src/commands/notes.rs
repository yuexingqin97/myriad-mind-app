use crate::error::AppError;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct NoteSaveResult {
    pub path: String,
    pub title: String,
    pub category: String,
    pub version: u32,
}

pub fn save_note(
    ai_output: &str,
    source_input: &str,
    source_type: &str,
    base_dir: &str,
    user_category: Option<&str>,
    debug_metadata: bool,
    target_path: Option<&str>,
) -> Result<NoteSaveResult, AppError> {
    let normalized_output = normalize_mermaid_blocks(ai_output);
    let title = extract_title(&normalized_output).unwrap_or_else(|| "Untitled Note".to_string());
    let safe_name = title_to_slug(&title);

    let category = if let Some(cat) = user_category.and_then(normalize_category_name) {
        cat
    } else {
        detect_category(&normalized_output, base_dir)
    };

    let cat_dir = PathBuf::from(base_dir).join(&category);
    std::fs::create_dir_all(&cat_dir)
        .map_err(|e| AppError::Config(format!("Failed to create category directory: {e}")))?;
    let _ = std::fs::create_dir_all(cat_dir.join("assets"));

    let note_path = if let Some(path) = target_path {
        PathBuf::from(base_dir).join(path)
    } else {
        resolve_path(&cat_dir, &safe_name)
    };

    let old_content = std::fs::read_to_string(&note_path).unwrap_or_default();
    let version = if old_content.is_empty() {
        1
    } else {
        read_version(&old_content) + 1
    };

    let fingerprint = simple_hash(source_input);
    let update_rows = build_update_rows(&old_content, version, source_input);
    let old_qa = extract_qa_section(&old_content);
    let metadata_block = generate_metadata_block(
        &title,
        &category,
        version,
        source_type,
        source_input,
        &fingerprint,
    );

    let mut final_content = normalized_output;
    final_content.push_str("\n\n---\n\n## Myriad Mind\n\n### Update Log\n\n");
    final_content.push_str(&update_rows);

    if !old_qa.is_empty() {
        final_content.push_str("\n\n### QA Log\n\n");
        final_content.push_str(&old_qa);
    }

    final_content.push_str("\n\n### Metadata\n\n");
    final_content.push_str("> Internal metadata for dedupe, library placement, and follow-up QA.\n\n");
    final_content.push_str(&metadata_block);

    if debug_metadata {
        final_content.push_str(&format!(
            "\n\n### Debug\n\n| Item | Value |\n| --- | --- |\n| App Version | 0.1.0-alpha.1 |\n| AI Provider | deepseek |\n| AI Model | deepseek-v4-pro |\n| Category | {category} |\n| Source Type | {source_type} |\n| Fingerprint | {fingerprint} |\n| Output Dir | {base_dir} |\n"
        ));
    }

    std::fs::write(&note_path, &final_content)
        .map_err(|e| AppError::Config(format!("Failed to write note: {e}")))?;

    log::info!("[notes] saved: {} v{version}", note_path.display());

    Ok(NoteSaveResult {
        path: note_path.to_string_lossy().to_string(),
        title,
        category,
        version,
    })
}

fn detect_category(content: &str, base_dir: &str) -> String {
    if let Some(category) = extract_ai_category(content).and_then(|c| normalize_category_name(&c)) {
        for existing in scan_categories(base_dir) {
            if existing.to_lowercase() == category.to_lowercase() {
                return existing;
            }
        }
        return category;
    }

    "Uncategorized".to_string()
}

fn extract_ai_category(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim().trim_start_matches('>').trim();
        let lower = trimmed.to_lowercase();
        if lower.contains("ai_category") || trimmed.contains("AI 建议分类") || trimmed.contains("建议分类") {
            if trimmed.starts_with('|') {
                let cells: Vec<&str> = trimmed
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect();
                if cells.len() >= 2 {
                    return Some(cells[1].to_string());
                }
            }
            if let Some((_, value)) = trimmed.split_once(':') {
                return Some(value.trim().to_string());
            }
            if let Some((_, value)) = trimmed.split_once('：') {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn normalize_category_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .trim()
        .trim_matches('|')
        .trim_matches('`')
        .trim_matches('*')
        .trim()
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_alphabetic()
                || c.is_numeric()
                || matches!(*c, '-' | '_' | ' ' | '.')
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('-').trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.chars().take(40).collect())
    }
}

fn scan_categories(base_dir: &str) -> Vec<String> {
    std::fs::read_dir(base_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| !name.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_path(cat_dir: &Path, base_name: &str) -> PathBuf {
    let primary = cat_dir.join(format!("{base_name}.md"));
    if !primary.exists() {
        return primary;
    }
    for i in 2u32.. {
        let candidate = cat_dir.join(format!("{base_name}-{i}.md"));
        if !candidate.exists() {
            return candidate;
        }
    }
    primary
}

fn extract_title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find(|line| line.trim().starts_with("# "))
        .map(|line| line.trim()[2..].trim().to_string())
        .and_then(|title| {
            if title.is_empty() {
                None
            } else {
                Some(title)
            }
        })
}

fn title_to_slug(title: &str) -> String {
    let slug: String = title
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_alphabetic() || c.is_numeric() || matches!(*c, '-' | '_' | ' '))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();
    let slug = slug.replace("--", "-").trim_matches('-').to_string();
    if slug.is_empty() {
        "untitled-note".to_string()
    } else if slug.chars().count() > 80 {
        slug.chars().take(80).collect()
    } else {
        slug
    }
}

fn build_update_rows(old_content: &str, version: u32, source_input: &str) -> String {
    let now = timestamp_now();
    let action = if version == 1 {
        "Initial refinement"
    } else {
        "Re-refinement"
    };
    let new_row = format!("| {now} | {action} · Source: {source_input} |");
    let old_rows = extract_update_rows(old_content);
    if old_rows.is_empty() {
        format!("| Updated At | Change |\n| --- | --- |\n{new_row}")
    } else {
        format!("| Updated At | Change |\n| --- | --- |\n{old_rows}\n{new_row}")
    }
}

fn extract_update_rows(content: &str) -> String {
    let Some(start) = content.find("### Update Log").or_else(|| content.find("### 更新记录")) else {
        return String::new();
    };
    content[start..]
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('|')
                && !trimmed.contains("---")
                && !trimmed.contains("Updated At")
                && !trimmed.contains("更新")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_qa_section(content: &str) -> String {
    let Some(start) = content.find("### QA Log").or_else(|| content.find("### 问答记录")) else {
        return String::new();
    };
    let after = &content[start..];
    if let Some(next) = after.find("\n### Metadata") {
        return after[..next].trim().to_string();
    }
    if let Some(next) = after.find("\n<!-- MYRIAD_MIND_METADATA") {
        return after[..next].trim().to_string();
    }
    after.trim().to_string()
}

fn read_version(content: &str) -> u32 {
    if let Some(block) = extract_metadata_block(content) {
        for line in block.lines() {
            if line.trim().starts_with("current_version:") {
                return line
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(1);
            }
        }
    }
    1
}

fn extract_metadata_block(content: &str) -> Option<String> {
    let start_marker = "<!-- MYRIAD_MIND_METADATA_START -->";
    let end_marker = "<!-- MYRIAD_MIND_METADATA_END -->";
    let start = content.rfind(start_marker)?;
    let after = &content[start + start_marker.len()..];
    let end = after.find(end_marker)?;
    let block = &after[..end];
    if let Some(yaml_start) = block.find("```yaml") {
        let yaml = &block[yaml_start + 7..];
        if let Some(yaml_end) = yaml.find("```") {
            return Some(yaml[..yaml_end].trim().to_string());
        }
    }
    Some(block.trim().to_string())
}

fn generate_metadata_block(
    title: &str,
    category: &str,
    version: u32,
    source_type: &str,
    source_raw: &str,
    fingerprint: &str,
) -> String {
    let now = timestamp_now();
    let slug = title_to_slug(title);
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
  platform: windows\n\
pipeline:\n\
  schema_version: 1\n\
  mode: note_generation\n\
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
```\n\
<!-- MYRIAD_MIND_METADATA_END -->"
    )
}

fn normalize_mermaid_blocks(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 4);
    let mut i = 0usize;
    let mut in_fence = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            out.push(line.to_string());
            i += 1;
            continue;
        }

        if !in_fence && trimmed.starts_with("%%{init:") && next_non_empty_starts_graph(&lines, i + 1) {
            out.push("```mermaid".into());
            out.push(line.to_string());
            i += 1;

            let mut skipped_misplaced_opening = false;
            while i < lines.len() {
                let current = lines[i];
                if current.trim().starts_with("```") {
                    if skipped_misplaced_opening {
                        out.push("```".into());
                        i += 1;
                        break;
                    }
                    skipped_misplaced_opening = true;
                    i += 1;
                    continue;
                }
                out.push(current.to_string());
                i += 1;
            }

            if !out.last().map(|line| line.trim() == "```").unwrap_or(false) {
                out.push("```".into());
            }
            continue;
        }

        out.push(line.to_string());
        i += 1;
    }

    let mut result = out.join("\n");
    if markdown.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn next_non_empty_starts_graph(lines: &[&str], start: usize) -> bool {
    lines
        .iter()
        .skip(start)
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("graph ")
                || trimmed.starts_with("flowchart ")
                || trimmed.starts_with("sequenceDiagram")
                || trimmed.starts_with("classDiagram")
                || trimmed.starts_with("stateDiagram")
                || trimmed.starts_with("erDiagram")
                || trimmed.starts_with("journey")
                || trimmed.starts_with("gantt")
        })
        .unwrap_or(false)
}

pub fn timestamp_now() -> String {
    use std::time::SystemTime;

    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

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

pub fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

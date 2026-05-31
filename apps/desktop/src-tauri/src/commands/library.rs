// ============================================================
// 输出目录知识库索引 — .myriad-mind/ 管理
// P0: ensure, scan, placement, update
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ---- 数据结构 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
    pub schema: String,
    pub library_id: String,
    pub root: String,
    pub created_at: String,
    pub updated_at: String,
    pub index: LibraryIndexStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryIndexStats {
    pub note_count: usize,
    pub category_count: usize,
    pub last_scan_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteIndexEntry {
    pub id: String,
    pub path: String,
    pub title: String,
    pub category: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub difficulty: String,
    pub current_version: u32,
    pub updated_at: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub ai_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryEntry {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub note_count: usize,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintEntry {
    pub note_id: String,
    pub path: String,
    pub source_raw: String,
    pub source_type: String,
    pub first_seen_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlacementPlan {
    pub action: String, // "create_note" | "update_note" | "append_to_note"
    pub confidence: f64,
    pub reason: String,
    pub category: PlacementCategory,
    pub target: PlacementTarget,
    pub source: PlacementSource,
    pub candidates: Vec<PlacementCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlacementCategory {
    pub name: String,
    pub path: String,
    pub is_existing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlacementTarget {
    pub path: String,
    pub exists: bool,
    pub note_id: Option<String>,
    pub current_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlacementSource {
    pub raw: String,
    pub canonical: String,
    pub fingerprint: String,
    pub source_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlacementCandidate {
    pub path: String,
    pub title: String,
    pub score: f64,
    pub reason: String,
}

// ---- 主入口 ----

/// 确保 .myriad-mind/ 索引存在。缺失则扫描重建。
pub fn ensure_library(base_dir: &str) -> Result<(), String> {
    let lib_dir = lib_path(base_dir);
    if lib_dir.join("library.json").exists() {
        return Ok(());
    }
    log::info!("[library] building index for {}", base_dir);
    rebuild_library(base_dir)
}

/// 为输入生成 PlacementPlan
pub fn plan_placement(
    base_dir: &str,
    source_raw: &str,
    source_type: &str,
    fingerprint: &str,
    suggested_category: &str,
    suggested_title: &str,
    user_category: Option<&str>,
) -> PlacementPlan {
    let lib_dir = lib_path(base_dir);

    // 1. 检查 fingerprint 命中
    if let Ok(fp_data) = std::fs::read_to_string(lib_dir.join("fingerprints.json")) {
        if let Ok(fp_map) = serde_json::from_str::<FingerprintMap>(&fp_data) {
            let fp_key = format!("sha256:{fingerprint}");
            if let Some(entry) = fp_map.items.get(&fp_key) {
                let exists = PathBuf::from(base_dir).join(&entry.path).exists();
                return PlacementPlan {
                    action: if exists {
                        "update_note".into()
                    } else {
                        "create_note".into()
                    },
                    confidence: 0.98,
                    reason: format!("source_fingerprint 命中 {}", entry.path),
                    category: PlacementCategory {
                        name: PathBuf::from(&entry.path)
                            .parent()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        path: PathBuf::from(&entry.path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        is_existing: true,
                    },
                    target: PlacementTarget {
                        path: entry.path.clone(),
                        exists,
                        note_id: Some(entry.note_id.clone()),
                        current_version: None,
                    },
                    source: PlacementSource {
                        raw: source_raw.into(),
                        canonical: format!("{source_type}:{fingerprint}"),
                        fingerprint: fingerprint.into(),
                        source_type: source_type.into(),
                    },
                    candidates: vec![],
                };
            }
        }
    }

    // 2. 检查用户指定
    if let Some(cat) = user_category {
        let cat_parts: Vec<&str> = cat.split('/').collect();
        return PlacementPlan {
            action: "create_note".into(),
            confidence: 0.95,
            reason: "用户指定子目录".into(),
            category: PlacementCategory {
                name: cat_parts[0].into(),
                path: cat.into(),
                is_existing: lib_dir.join("..").join(cat).exists(),
            },
            target: PlacementTarget {
                path: format!("{cat}/{suggested_title}.md"),
                exists: false,
                note_id: None,
                current_version: None,
            },
            source: PlacementSource {
                raw: source_raw.into(),
                canonical: format!("{source_type}:{fingerprint}"),
                fingerprint: fingerprint.into(),
                source_type: source_type.into(),
            },
            candidates: vec![],
        };
    }

    // 3. 默认：在 suggested category 下新建
    PlacementPlan {
        action: "create_note".into(),
        confidence: 0.8,
        reason: format!("根据内容识别为 {suggested_category}"),
        category: PlacementCategory {
            name: suggested_category.into(),
            path: suggested_category.into(),
            is_existing: PathBuf::from(base_dir).join(suggested_category).exists(),
        },
        target: PlacementTarget {
            path: format!("{suggested_category}/{suggested_title}.md"),
            exists: false,
            note_id: None,
            current_version: None,
        },
        source: PlacementSource {
            raw: source_raw.into(),
            canonical: format!("{source_type}:{fingerprint}"),
            fingerprint: fingerprint.into(),
            source_type: source_type.into(),
        },
        candidates: vec![],
    }
}

/// 保存笔记后更新索引
pub fn update_library_after_save(
    base_dir: &str,
    note_path: &str,
    title: &str,
    category: &str,
    version: u32,
    source_raw: &str,
    source_type: &str,
    fingerprint: &str,
    note_id: &str,
    note_content: &str,
) {
    let lib_dir = lib_path(base_dir);
    let _ = std::fs::create_dir_all(&lib_dir);
    let now = crate::commands::notes::timestamp_now();

    // Update notes.jsonl
    let notes_path = lib_dir.join("notes.jsonl");
    let mut notes: Vec<NoteIndexEntry> = if notes_path.exists() {
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(&notes_path).unwrap_or_default(),
        )
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
    } else {
        vec![]
    };

    // Remove old entry if exists, add new
    notes.retain(|n| n.path != note_path);
    notes.push(NoteIndexEntry {
        id: note_id.into(),
        path: note_path.into(),
        title: title.into(),
        category: category.into(),
        summary: String::new(),
        tags: vec![],
        topics: vec![],
        difficulty: String::new(),
        current_version: version,
        updated_at: now.clone(),
        sources: vec![format!("sha256:{fingerprint}")],
        ai_model: "deepseek-v4-pro".into(),
    });
    let _ = std::fs::write(
        &notes_path,
        serde_json::to_string_pretty(&notes).unwrap_or_default(),
    );

    // Update fingerprints
    update_fingerprints(
        &lib_dir,
        note_id,
        note_path,
        source_raw,
        source_type,
        fingerprint,
        &now,
    );

    // Update categories
    update_categories(&lib_dir, base_dir);

    // Update library.json
    update_library_json(&lib_dir, base_dir, notes.len(), &now);

    // Update memory.md (incremental)
    update_memory_md(&lib_dir, title, category, note_path, &notes);

    // Update scan-state
    update_scan_state(&lib_dir, base_dir);
}

// ---- 内部实现 ----

#[derive(Debug, Serialize, Deserialize)]
struct FingerprintMap {
    items: HashMap<String, FingerprintEntry>,
}

fn lib_path(base_dir: &str) -> PathBuf {
    PathBuf::from(base_dir).join(".myriad-mind")
}

fn rebuild_library(base_dir: &str) -> Result<(), String> {
    let lib_dir = lib_path(base_dir);
    std::fs::create_dir_all(&lib_dir).map_err(|e| e.to_string())?;
    let now = crate::commands::notes::timestamp_now();

    // Scan .md files
    let mut notes: Vec<NoteIndexEntry> = vec![];
    let mut fp_items: HashMap<String, FingerprintEntry> = HashMap::new();
    scan_markdown_files(base_dir, base_dir, &mut notes, &mut fp_items);

    // Write files
    let notes_path = lib_dir.join("notes.jsonl");
    std::fs::write(
        &notes_path,
        serde_json::to_string_pretty(&notes).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;

    let fp_path = lib_dir.join("fingerprints.json");
    let fp_map = FingerprintMap { items: fp_items };
    std::fs::write(
        &fp_path,
        serde_json::to_string_pretty(&fp_map).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;

    update_categories(&lib_dir, base_dir);
    update_library_json(&lib_dir, base_dir, notes.len(), &now);
    build_memory_md(&lib_dir, &notes);
    update_scan_state(&lib_dir, base_dir);

    log::info!("[library] index rebuilt: {} notes", notes.len());
    Ok(())
}

fn scan_markdown_files(
    base_dir: &str,
    current_dir: &str,
    notes: &mut Vec<NoteIndexEntry>,
    fp_items: &mut HashMap<String, FingerprintEntry>,
) {
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                scan_markdown_files(base_dir, &path.to_string_lossy(), notes, fp_items);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path
                        .strip_prefix(base_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    let entry = parse_note_metadata(&rel, &content);
                    let eid = entry.id.clone();
                    let erel = rel.clone();
                    notes.push(entry);

                    // Extract fingerprint
                    if let Some(block) = extract_metadata_block(&content) {
                        for line in block.lines() {
                            if line.trim().starts_with("fingerprint:") {
                                let fp = line
                                    .split(':')
                                    .nth(1)
                                    .unwrap_or("")
                                    .trim()
                                    .trim_matches('"')
                                    .to_string();
                                if !fp.is_empty() {
                                    let fp_key = format!("sha256:{fp}");
                                    fp_items.entry(fp_key).or_insert(FingerprintEntry {
                                        note_id: eid.clone(),
                                        path: erel.clone(),
                                        source_raw: String::new(),
                                        source_type: String::new(),
                                        first_seen_at: String::new(),
                                        last_used_at: String::new(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn parse_note_metadata(rel_path: &str, content: &str) -> NoteIndexEntry {
    let title = content
        .lines()
        .find(|l| l.trim().starts_with("# "))
        .map(|l| l.trim()[2..].trim().to_string())
        .unwrap_or_else(|| rel_path.to_string());
    let category = PathBuf::from(rel_path)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "未分类".into());
    let mut version = 1u32;
    if let Some(block) = extract_metadata_block(content) {
        for line in block.lines() {
            if line.trim().starts_with("current_version:") {
                version = line
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(1);
            }
        }
    }
    NoteIndexEntry {
        id: format!("note_{:016x}", simple_hash(rel_path)),
        path: rel_path.into(),
        title,
        category,
        summary: String::new(),
        tags: vec![],
        topics: vec![],
        difficulty: String::new(),
        current_version: version,
        updated_at: String::new(),
        sources: vec![],
        ai_model: String::new(),
    }
}

fn extract_metadata_block(content: &str) -> Option<String> {
    if let Some(start) = content.rfind("<!-- MYRIAD_MIND_METADATA_START -->") {
        let after = &content[start + 37..];
        if let Some(end) = after.find("<!-- MYRIAD_MIND_METADATA_END -->") {
            let block = &after[..end];
            if let Some(ys) = block.find("```yaml") {
                let y = &block[ys + 7..];
                if let Some(ye) = y.find("```") {
                    return Some(y[..ye].trim().to_string());
                }
            }
            return Some(block.trim().to_string());
        }
    }
    None
}

fn update_fingerprints(
    lib_dir: &PathBuf,
    note_id: &str,
    path: &str,
    source_raw: &str,
    source_type: &str,
    fingerprint: &str,
    now: &str,
) {
    let fp_path = lib_dir.join("fingerprints.json");
    let mut map: FingerprintMap = if fp_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&fp_path).unwrap_or_default()).unwrap_or(
            FingerprintMap {
                items: HashMap::new(),
            },
        )
    } else {
        FingerprintMap {
            items: HashMap::new(),
        }
    };
    let fp_key = format!("sha256:{fingerprint}");
    if let Some(entry) = map.items.get_mut(&fp_key) {
        entry.last_used_at = now.into();
    } else {
        map.items.insert(
            fp_key.clone(),
            FingerprintEntry {
                note_id: note_id.into(),
                path: path.into(),
                source_raw: source_raw.into(),
                source_type: source_type.into(),
                first_seen_at: now.into(),
                last_used_at: now.into(),
            },
        );
    }
    let _ = std::fs::write(
        &fp_path,
        serde_json::to_string_pretty(&map).unwrap_or_default(),
    );
}

fn update_categories(lib_dir: &PathBuf, base_dir: &str) {
    let mut cat_map: HashMap<String, usize> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let count = count_md_files(&entry.path());
                cat_map.insert(name, count);
            }
        }
    }
    let cats: Vec<CategoryEntry> = cat_map
        .into_iter()
        .map(|(name, count)| CategoryEntry {
            name: name.clone(),
            path: name,
            aliases: vec![],
            note_count: count,
            tags: vec![],
        })
        .collect();
    let _ = std::fs::write(
        lib_dir.join("categories.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "myriad-mind-categories/v1",
            "categories": cats,
        }))
        .unwrap_or_default(),
    );
}

fn count_md_files(dir: &std::path::Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
                count += 1;
            }
        }
    }
    count
}

fn update_library_json(lib_dir: &PathBuf, base_dir: &str, note_count: usize, now: &str) {
    let lib = LibraryInfo {
        schema: "myriad-mind-library/v1".into(),
        library_id: format!("lib_{:08x}", simple_hash(base_dir)),
        root: base_dir.into(),
        created_at: now.into(),
        updated_at: now.into(),
        index: LibraryIndexStats {
            note_count,
            category_count: count_categories(base_dir),
            last_scan_at: now.into(),
        },
    };
    let _ = std::fs::write(
        lib_dir.join("library.json"),
        serde_json::to_string_pretty(&lib).unwrap_or_default(),
    );
}

fn count_categories(base_dir: &str) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) && !name.starts_with('.') {
                count += 1;
            }
        }
    }
    count
}

fn build_memory_md(lib_dir: &PathBuf, notes: &[NoteIndexEntry]) {
    let mut content = String::from(
        "# 大衍决知识库记忆\n\n> 自动生成的简略索引。用于帮助 AI 判断分类和避免重复生成。\n\n## 分类概览\n\n",
    );
    let mut cat_counts: HashMap<&str, usize> = HashMap::new();
    for n in notes {
        *cat_counts.entry(&n.category).or_default() += 1;
    }
    for (cat, count) in &cat_counts {
        content.push_str(&format!("- {}：{} 篇\n", cat, count));
    }
    content.push_str("\n## 笔记索引\n\n| 标题 | 分类 | 路径 |\n|------|------|------|\n");
    for n in notes.iter().take(50) {
        content.push_str(&format!("| {} | {} | {} |\n", n.title, n.category, n.path));
    }
    let _ = std::fs::write(lib_dir.join("memory.md"), &content);
}

fn update_memory_md(
    lib_dir: &PathBuf,
    title: &str,
    category: &str,
    _path: &str,
    _notes: &[NoteIndexEntry],
) {
    let memory_path = lib_dir.join("memory.md");
    if !memory_path.exists() {
        let _ = build_memory_md(lib_dir, &[]);
    }
    // P0: simple append — add a line about the new note
    if let Ok(mut content) = std::fs::read_to_string(&memory_path) {
        if !content.contains(&format!("| {} | {} |", title, category)) {
            if let Some(pos) = content.rfind('\n') {
                content.insert_str(pos, &format!("\n| {} | {} | {} |", title, category, _path));
            }
        }
        let _ = std::fs::write(&memory_path, &content);
    }
}

fn update_scan_state(lib_dir: &PathBuf, base_dir: &str) {
    let mut files: HashMap<String, serde_json::Value> = HashMap::new();
    collect_file_states(PathBuf::from(base_dir), base_dir, &mut files);
    let state = serde_json::json!({
        "last_scan_at": crate::commands::notes::timestamp_now(),
        "files": files,
    });
    let _ = std::fs::write(
        lib_dir.join("scan-state.json"),
        serde_json::to_string_pretty(&state).unwrap_or_default(),
    );
}

fn collect_file_states(
    dir: PathBuf,
    base_dir: &str,
    files: &mut HashMap<String, serde_json::Value>,
) {
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                collect_file_states(path, base_dir, files);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                let meta = std::fs::metadata(&path).ok();
                let rel = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                files.insert(rel, serde_json::json!({
                    "mtime": meta.as_ref().and_then(|m| m.modified().ok()).map(|t| format!("{:?}", t)).unwrap_or_default(),
                    "size": meta.map(|m| m.len()).unwrap_or(0),
                }));
            }
        }
    }
}

fn simple_hash(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    h.finish()
}

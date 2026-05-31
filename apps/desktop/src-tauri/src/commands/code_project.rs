// ============================================================
// 代码项目扫描 — 目录结构 → 优先级文件 → Markdown 格式化
// 职责: 递归扫描、文件优先级排序、Token 预估、格式化为 AI 输入
// ============================================================

use crate::error::AppError;
use std::path::Path;

// ============================================================
// 数据结构
// ============================================================

pub struct CodeProjectScan {
    pub root_path: String,
    pub file_tree: String,
    pub priority_files: Vec<PriorityFile>,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub size_label: String,
    pub tech_stack: Vec<String>,
}

pub struct PriorityFile {
    pub relative_path: String,
    pub content: String,
    pub _priority: u8,
    pub role: String,
}

// ============================================================
// 常量
// ============================================================

const MAX_FILE_BYTES: usize = 32 * 1024;   // 单文件 32KB
const MAX_TOTAL_BYTES: usize = 512 * 1024;  // 总计 512KB
const MAX_TREE_LINES: usize = 200;

/// 递归扫描排除的目录
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    "dist",
    "build",
    ".next",
    ".cache",
    "vendor",
    "venv",
    ".venv",
    ".tox",
    ".idea",
    ".vscode",
    "coverage",
    ".nuxt",
    ".output",
    "out",
    "bin",
    "obj",
    "Debug",
    "Release",
    "x64",
    ".gradle",
    ".mvn",
    "Pods",
    ".dart_tool",
];

/// 代码文件扩展名
const CODE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp",
    "cs", "rb", "php", "swift", "kt", "scala", "sh", "bash", "sql", "proto",
];

/// 配置文件扩展名
const CONFIG_EXTENSIONS: &[&str] = &[
    "toml", "json", "yaml", "yml", "xml", "ini", "cfg", "conf", "env",
];

// ============================================================
// 主入口
// ============================================================

/// 扫描代码项目目录，按优先级读取文件
pub fn scan_code_project(root: &Path, max_depth: usize) -> Result<CodeProjectScan, AppError> {
    if !root.is_dir() {
        return Err(AppError::Other(format!(
            "路径不是目录: {}",
            root.display()
        )));
    }

    // 1. 递归扫描目录结构
    let mut all_files: Vec<FileMeta> = Vec::new();
    collect_files(root, root, &mut all_files, max_depth, 0)?;

    // 2. 统计
    let total_files = all_files.len();
    let total_size_bytes: u64 = all_files.iter().map(|f| f.size_bytes).sum();
    let size_label = classify_size(total_files, total_size_bytes);

    // 3. 推断技术栈
    let tech_stack = detect_tech_stack(&all_files);

    // 4. 生成目录树
    let file_tree = build_tree(root, max_depth)?;

    // 5. 按优先级读取文件
    let priority_files = read_priority_files(root, &all_files)?;

    Ok(CodeProjectScan {
        root_path: root.to_string_lossy().to_string(),
        file_tree,
        priority_files,
        total_files,
        total_size_bytes,
        size_label,
        tech_stack,
    })
}

/// 格式化扫描结果为 Markdown，供 AI 读取
pub fn format_code_project_for_ai(scan: &CodeProjectScan) -> String {
    let mut md = String::new();

    // 元信息
    md.push_str("## 项目元信息\n\n");
    md.push_str(&format!("- 路径: {}\n", scan.root_path));
    md.push_str(&format!("- 文件数: {}\n", scan.total_files));
    md.push_str(&format!(
        "- 总大小: {} KB\n",
        scan.total_size_bytes / 1024
    ));
    md.push_str(&format!("- 规模: {}\n", scan.size_label));
    md.push_str(&format!(
        "- 技术栈: {}\n",
        scan.tech_stack.join(", ")
    ));

    // 目录树
    md.push_str("\n---\n\n## 目录结构\n\n```\n");
    md.push_str(&scan.file_tree);
    md.push_str("\n```\n");

    // 优先级文件内容
    md.push_str("\n---\n\n## 关键文件\n");
    for pf in &scan.priority_files {
        md.push_str(&format!(
            "\n### {} ({})\n\n",
            pf.relative_path, pf.role
        ));

        // 判断是否需要代码块
        let ext = Path::new(&pf.relative_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if is_code_or_config(ext) {
            md.push_str(&format!("```{}\n{}\n```\n", ext, pf.content));
        } else {
            md.push_str(&pf.content);
            md.push('\n');
        }
    }

    md
}

// ============================================================
// 内部函数
// ============================================================

struct FileMeta {
    relative_path: String,
    size_bytes: u64,
    priority: u8,
    role: String,
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<FileMeta>,
    max_depth: usize,
    current_depth: usize,
) -> Result<(), AppError> {
    if current_depth > max_depth {
        return Ok(());
    }

    let entries = std::fs::read_dir(current).map_err(|e| {
        AppError::Other(format!("无法读取目录 {}: {e}", current.display()))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| AppError::Other(format!("目录项读取失败: {e}")))?;
        let path = entry.path();

        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if should_skip_dir(&name) {
                continue;
            }
            collect_files(root, &path, files, max_depth, current_depth + 1)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let (priority, role) = classify_file(&rel, &path);
            files.push(FileMeta {
                relative_path: rel,
                size_bytes,
                priority,
                role,
            });
        }
    }

    Ok(())
}

fn should_skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || SKIP_DIRS.contains(&name)
        || name.starts_with("gradle") // .gradle cache
}

/// 对文件分类优先级和角色
fn classify_file(rel: &str, path: &Path) -> (u8, String) {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // P1: README
    if filename.starts_with("readme") {
        return (1, "README".into());
    }

    // P2: 构建配置
    let p2_configs = [
        "cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "go.sum",
        "pom.xml",
        "build.gradle",
        "cmakelists.txt",
        "makefile",
        "gemfile",
        "composer.json",
    ];
    if p2_configs.contains(&filename.as_str()) {
        return (2, "构建配置".into());
    }

    // P2: 工作区配置
    if filename == "cargo.lock" || filename == "package-lock.json" || filename == "yarn.lock" {
        return (2, "锁文件".into());
    }

    // P3: 入口文件
    let entry_files = [
        "main.rs",
        "main.py",
        "main.go",
        "main.ts",
        "main.js",
        "main.java",
        "main.c",
        "main.cpp",
        "index.ts",
        "index.js",
        "index.rs",
        "app.tsx",
        "app.jsx",
        "app.py",
        "lib.rs",
        "mod.rs",
    ];
    if entry_files.contains(&filename.as_str()) {
        return (3, "入口文件".into());
    }

    // P4: 核心模块源码
    if rel.contains("src/")
        || rel.contains("lib/")
        || rel.contains("core/")
        || rel.contains("internal/")
        || rel.contains("pkg/")
    {
        if CODE_EXTENSIONS.contains(&ext.as_str()) {
            return (4, "核心源码".into());
        }
        if CONFIG_EXTENSIONS.contains(&ext.as_str()) {
            return (4, "模块配置".into());
        }
    }

    // P5: 其他源码和配置
    if CODE_EXTENSIONS.contains(&ext.as_str()) {
        return (5, "源码".into());
    }
    if CONFIG_EXTENSIONS.contains(&ext.as_str()) {
        return (5, "配置".into());
    }
    if ext == "md" || ext == "txt" || ext == "rst" {
        return (5, "文档".into());
    }

    // 跳过二进制和无关文件
    (99, "其他".into())
}

/// 按优先级读取文件，控制总量
fn read_priority_files(root: &Path, files: &[FileMeta]) -> Result<Vec<PriorityFile>, AppError> {
    let mut sorted: Vec<&FileMeta> = files.iter().filter(|f| f.priority < 99).collect();
    sorted.sort_by_key(|f| f.priority);

    let mut result = Vec::new();
    let mut total_bytes = 0usize;

    for fm in sorted {
        // 总量限制
        if total_bytes >= MAX_TOTAL_BYTES {
            break;
        }

        // 单文件大小限制
        if fm.size_bytes as usize > MAX_FILE_BYTES * 2 {
            // 太大的文件只读开头
            let path = root.join(&fm.relative_path);
            let content = read_file_head(&path, MAX_FILE_BYTES)?;
            let note = format!(
                "\n\n[文件较大 ({} KB)，仅展示前 {} KB]",
                fm.size_bytes / 1024,
                MAX_FILE_BYTES / 1024
            );
            result.push(PriorityFile {
                relative_path: fm.relative_path.clone(),
                content: format!("{}{}", content, note),
                _priority: fm.priority,
                role: fm.role.clone(),
            });
            total_bytes += content.len();
        } else {
            let path = root.join(&fm.relative_path);
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let bytes = content.len();
                    if total_bytes + bytes <= MAX_TOTAL_BYTES {
                        result.push(PriorityFile {
                            relative_path: fm.relative_path.clone(),
                            content,
                            _priority: fm.priority,
                            role: fm.role.clone(),
                        });
                        total_bytes += bytes;
                    }
                }
                Err(_) => {
                    // 跳过无法读取的文件（可能是二进制）
                    continue;
                }
            }
        }
    }

    Ok(result)
}

/// 读取文件开头部分
fn read_file_head(path: &Path, max_bytes: usize) -> Result<String, AppError> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(AppError::Io)?;
    let mut buf = vec![0u8; max_bytes];
    let n = file.read(&mut buf).map_err(AppError::Io)?;
    buf.truncate(n);
    String::from_utf8(buf).map_err(|e| AppError::Other(format!("文件编码错误: {e}")))
}

/// 生成目录树字符串
fn build_tree(root: &Path, max_depth: usize) -> Result<String, AppError> {
    let mut lines = Vec::new();
    build_tree_recursive(root, root, &mut lines, max_depth, 0)?;
    if lines.len() > MAX_TREE_LINES {
        lines.truncate(MAX_TREE_LINES);
        lines.push("... (已截断)".into());
    }
    Ok(lines.join("\n"))
}

fn build_tree_recursive(
    root: &Path,
    current: &Path,
    lines: &mut Vec<String>,
    max_depth: usize,
    current_depth: usize,
) -> Result<(), AppError> {
    if current_depth > max_depth || lines.len() >= MAX_TREE_LINES {
        return Ok(());
    }

    let entries = std::fs::read_dir(current).map_err(AppError::Io)?;
    let mut entries: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            if e.path().is_dir() {
                let name = e.file_name().to_string_lossy().to_string();
                !should_skip_dir(&name)
            } else {
                true
            }
        })
        .collect();
    entries.sort_by_key(|e| {
        let is_dir = e.path().is_dir();
        (/* !is_dir first */ !is_dir, e.file_name())
    });

    let prefix = if current_depth == 0 {
        String::new()
    } else {
        "│   ".repeat(current_depth - 1) + "├── "
    };

    for entry in entries {
        if lines.len() >= MAX_TREE_LINES {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            lines.push(format!("{}{}/", prefix, name));
            build_tree_recursive(root, &entry.path(), lines, max_depth, current_depth + 1)?;
        } else {
            lines.push(format!("{}{}", prefix, name));
        }
    }

    Ok(())
}

fn classify_size(files: usize, bytes: u64) -> String {
    let kb = bytes / 1024;
    match (files, kb) {
        (f, _) if f < 30 => "小型 (<30 文件)".into(),
        (f, kb) if f < 200 && kb < 2048 => format!("中型 ({} 文件, {} KB)", f, kb),
        (f, kb) if f < 1000 && kb < 10240 => format!("大型 ({} 文件, {} KB)", f, kb),
        (f, kb) => format!("巨型 ({} 文件, {} KB)", f, kb),
    }
}

/// 从配置文件推断技术栈
fn detect_tech_stack(files: &[FileMeta]) -> Vec<String> {
    let filenames: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    let mut stack = Vec::new();

    if filenames.iter().any(|f| f.ends_with("Cargo.toml")) {
        stack.push("Rust".into());
    }
    if filenames.iter().any(|f| f.ends_with("package.json")) {
        stack.push("Node.js/TypeScript".into());
    }
    if filenames.iter().any(|f| f.ends_with("go.mod")) {
        stack.push("Go".into());
    }
    if filenames.iter().any(|f| f.ends_with("pyproject.toml") || f.ends_with("setup.py")) {
        stack.push("Python".into());
    }
    if filenames.iter().any(|f| f.ends_with("pom.xml") || f.ends_with("build.gradle")) {
        stack.push("Java".into());
    }
    if filenames.iter().any(|f| f.ends_with("CMakeLists.txt") || f.ends_with("Makefile")) {
        stack.push("C/C++".into());
    }
    if filenames.iter().any(|f| f.ends_with(".csproj")) {
        stack.push("C#/.NET".into());
    }
    if filenames.iter().any(|f| f.ends_with("Gemfile")) {
        stack.push("Ruby".into());
    }
    if filenames
        .iter()
        .any(|f| f.ends_with("pubspec.yaml") || f.ends_with("analysis_options.yaml"))
    {
        stack.push("Dart/Flutter".into());
    }

    if stack.is_empty() {
        // 从文件扩展名推断
        let has_rs = filenames.iter().any(|f| f.ends_with(".rs"));
        let has_py = filenames.iter().any(|f| f.ends_with(".py"));
        let has_ts = filenames.iter().any(|f| f.ends_with(".ts") || f.ends_with(".tsx"));
        if has_rs {
            stack.push("Rust".into());
        }
        if has_py {
            stack.push("Python".into());
        }
        if has_ts {
            stack.push("TypeScript".into());
        }
    }

    stack
}

fn is_code_or_config(ext: &str) -> bool {
    CODE_EXTENSIONS.contains(&ext)
        || CONFIG_EXTENSIONS.contains(&ext)
        || ext == "md"
        || ext == "txt"
}

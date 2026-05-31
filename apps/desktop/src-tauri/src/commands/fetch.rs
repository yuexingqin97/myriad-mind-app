// ============================================================
// 网页抓取 — URL → 结构化文章内容
// 职责: 平台识别 → HTTP 抓取 → HTML 解析 → 纯文本/Markdown 提取
// ============================================================

use crate::error::AppError;
use dom_query::Document;

// ============================================================
// 数据结构
// ============================================================

/// 抓取后的结构化文章内容
pub struct ArticleContent {
    pub title: String,
    pub author: Option<String>,
    pub publish_date: Option<String>,
    pub body_text: String,       // 简化 Markdown
    pub image_urls: Vec<String>, // 保留 URL，不下载
    pub original_url: String,
    pub platform: String,
    pub language_hint: String, // "chinese" / "english" / "mixed"
}

// ============================================================
// 平台识别
// ============================================================

/// 从 URL 域名识别平台
pub fn detect_platform(url: &str) -> &'static str {
    let host = extract_host(url);
    if host.contains("zhihu.com") {
        "zhihu"
    } else if host.contains("csdn.net") {
        "csdn"
    } else if host.contains("juejin.cn") || host.contains("juejin.im") {
        "juejin"
    } else if host.contains("jianshu.com") {
        "jianshu"
    } else if host.contains("mp.weixin.qq.com") || host.contains("weixin.qq.com") {
        "weixin"
    } else if host.contains("wikipedia.org") || host.starts_with("wiki.") {
        "wiki"
    } else if host.contains("github.com") {
        "github"
    } else if host.contains("stackoverflow.com") {
        "stackoverflow"
    } else {
        "generic"
    }
}

fn extract_host(url: &str) -> String {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .to_lowercase()
}

// ============================================================
// 反爬平台判定
// ============================================================

/// 已知无法直接抓取的平台
fn is_unfetchable(platform: &str) -> bool {
    matches!(platform, "zhihu" | "weixin")
}

/// 已知可能部分受限的平台
fn is_limited(platform: &str) -> bool {
    matches!(platform, "csdn")
}

// ============================================================
// 主入口: 抓取文章
// ============================================================

/// 抓取 URL 并提取结构化文章内容
pub async fn fetch_article(url: &str) -> Result<ArticleContent, AppError> {
    let platform = detect_platform(url);

    // 反爬平台直接返回友好错误
    if is_unfetchable(platform) {
        return Err(AppError::Other(format!(
            "⚠️ {platform} 文章受平台反爬保护，无法自动抓取。\n\n\
             请选择以下替代方案：\n\
             方案 A（推荐）：在浏览器打开文章 → Ctrl+S 另存为 HTML 文件 → 用本地文件模式处理\n\
             方案 B：直接复制文章内容粘贴到输入框"
        )));
    }

    // HTTP 抓取
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/125.0.0.0 Safari/537.36",
        )
        .build()
        .map_err(|e| AppError::Other(format!("HTTP 客户端创建失败: {e}")))?;

    let response = client
        .get(url)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Accept", "text/html,application/xhtml+xml")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::Other(format!(
            "HTTP 请求失败: status {}",
            response.status()
        )));
    }

    let html = response.text().await?;

    // 解析 HTML
    parse_article(&html, url, platform)
}

// ============================================================
// HTML 解析
// ============================================================

/// 解析 HTML 提取文章内容
fn parse_article(html: &str, url: &str, platform: &str) -> Result<ArticleContent, AppError> {
    let doc = Document::from(html);

    // 1. 提取标题
    let title = extract_title(&doc);

    // 2. 提取作者
    let author = extract_author(&doc, platform);

    // 3. 提取日期
    let publish_date = extract_date(&doc);

    // 4. 提取正文
    let (body_text, image_urls) = extract_body(&doc, platform);

    if body_text.trim().is_empty() {
        if is_limited(platform) {
            return Err(AppError::Other(format!(
                "⚠️ {platform} 文章内容提取为空，可能需要登录或遇到验证码。\n\n\
                 请选择以下替代方案：\n\
                 方案 A：在浏览器打开文章 → Ctrl+S 另存为 HTML 文件 → 用本地文件模式处理\n\
                 方案 B：直接复制文章内容粘贴到输入框"
            )));
        }
        return Err(AppError::Other(
            "文章正文提取为空，页面可能需要 JavaScript 渲染或有反爬保护。".into(),
        ));
    }

    // 5. 语言检测
    let language_hint = detect_language(&body_text);

    Ok(ArticleContent {
        title,
        author,
        publish_date,
        body_text,
        image_urls,
        original_url: url.to_string(),
        platform: platform.to_string(),
        language_hint: language_hint.to_string(),
    })
}

/// 格式化 ArticleContent 为 AI 可处理的 Markdown
pub fn format_article_for_ai(article: &ArticleContent) -> String {
    let mut md = String::new();

    md.push_str("## 文章元信息\n\n");
    md.push_str(&format!("- URL: {}\n", article.original_url));
    md.push_str(&format!("- 标题: {}\n", article.title));
    md.push_str(&format!(
        "- 作者: {}\n",
        article.author.as_deref().unwrap_or("未知")
    ));
    md.push_str(&format!(
        "- 发布日期: {}\n",
        article.publish_date.as_deref().unwrap_or("未知")
    ));
    md.push_str(&format!("- 平台: {}\n", article.platform));
    md.push_str(&format!("- 语言: {}\n", article.language_hint));

    if !article.image_urls.is_empty() {
        md.push_str(&format!(
            "\n- 文中图片 ({} 张, URL 保留未下载):\n",
            article.image_urls.len()
        ));
        for img_url in &article.image_urls {
            md.push_str(&format!("  - {}\n", img_url));
        }
    }

    md.push_str("\n---\n\n## 正文\n\n");
    md.push_str(&article.body_text);

    md
}

// ============================================================
// 提取子函数
// ============================================================

fn extract_title(doc: &Document) -> String {
    // 优先 og:title
    if let Some(el) = doc.select("meta[property='og:title']").get(0) {
        if let Some(content) = el.attr("content") {
            let s = content.to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    // 再 title 标签
    if let Some(el) = doc.select("title").get(0) {
        let text = el.text();
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    "未知标题".into()
}

fn extract_author(doc: &Document, platform: &str) -> Option<String> {
    let candidates: &[&str] = match platform {
        "csdn" => &[
            "meta[name='author']",
            ".user-name",
            "#uid span",
        ],
        "juejin" => &[
            "meta[name='author']",
            ".author-name",
            ".user-message a",
        ],
        "jianshu" => &[
            "meta[name='author']",
            ".name a",
        ],
        _ => &[
            "meta[property='og:author']",
            "meta[name='author']",
            "meta[name='byline']",
        ],
    };

    for selector in candidates {
        if let Some(el) = doc.select(selector).get(0) {
            // meta 标签优先取 content 属性
            if let Some(content) = el.attr("content") {
                let text = content.to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
            let text = el.text().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn extract_date(doc: &Document) -> Option<String> {
    let candidates = [
        "meta[property='article:published_time']",
        "meta[name='publish_time']",
        "meta[name='date']",
        "time[datetime]",
        "time",
    ];

    for selector in candidates {
        if let Some(el) = doc.select(selector).get(0) {
            let text = el
                .attr("datetime")
                .or_else(|| el.attr("content"))
                .map(|s| s.to_string())
                .unwrap_or_else(|| el.text().trim().to_string());
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn extract_body(doc: &Document, platform: &str) -> (String, Vec<String>) {
    // 平台专用选择器
    let body_selector = match platform {
        "csdn" => "#article_content",
        "juejin" => ".article-content",
        "jianshu" => ".article",
        "wiki" => "#mw-content-text",
        "stackoverflow" => ".js-post-body",
        _ => "",
    };

    let container_html = if !body_selector.is_empty() {
        let sel = doc.select(body_selector);
        sel.get(0).map(|el| el.html().to_string()).unwrap_or_default()
    } else {
        // 通用模式
        let mut result = String::new();
        for &sel_str in &["article", "main", "[role='main']", "body"] {
            let sel = doc.select(sel_str);
            if let Some(el) = sel.get(0) {
                let html = el.html().to_string();
                if html.len() > 200 {
                    result = html;
                    break;
                }
            }
        }
        result
    };

    if container_html.is_empty() {
        return (String::new(), Vec::new());
    }

    let container_doc = Document::from(&*container_html);

    // 提取图片 URL
    let img_sel = container_doc.select("img");
    let image_urls: Vec<String> = img_sel
        .iter()
        .filter_map(|el| {
            el.attr("src")
                .or_else(|| el.attr("data-src"))
                .map(|s| s.to_string())
                .filter(|s| !s.starts_with("data:") && !s.is_empty())
        })
        .collect();

    // 提取纯文本
    let body_sel = container_doc.select("body");
    let text = if let Some(el) = body_sel.get(0) {
        el.text()
    } else {
        let all_sel = container_doc.select("*");
        all_sel.get(0).map(|el| el.text()).unwrap_or_default()
    };

    let cleaned = clean_extracted_text(&text);

    (cleaned, image_urls)
}

/// 清理提取的文本：去除多余空白，保留段落结构
fn clean_extracted_text(text: &str) -> String {
    let mut result = String::new();
    let mut prev_empty = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                result.push('\n');
                prev_empty = true;
            }
        } else {
            result.push_str(trimmed);
            result.push('\n');
            prev_empty = false;
        }
    }

    result.trim().to_string()
}

// ============================================================
// 语言检测
// ============================================================

/// 检测文本主要语言 — 统计 CJK 字符占比
pub fn detect_language(text: &str) -> &'static str {
    let mut cjk_count = 0u32;
    let mut latin_count = 0u32;

    for ch in text.chars() {
        match ch {
            '\u{4E00}'..='\u{9FFF}'    // CJK Unified Ideographs
            | '\u{3400}'..='\u{4DBF}'  // CJK Extension A
            | '\u{F900}'..='\u{FAFF}'  // CJK Compatibility Ideographs
            => cjk_count += 1,
            'a'..='z' | 'A'..='Z' => latin_count += 1,
            _ => {}
        }
    }

    let total = cjk_count + latin_count;
    if total == 0 {
        return "unknown";
    }

    let cjk_ratio = cjk_count as f64 / total as f64;
    if cjk_ratio > 0.3 {
        "chinese"
    } else if cjk_ratio < 0.05 {
        "english"
    } else {
        "mixed"
    }
}

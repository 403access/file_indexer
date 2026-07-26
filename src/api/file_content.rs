use axum::extract::{Query, State};
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::states::app_state::{AppState, IndexerPauseGuard};

#[derive(Deserialize)]
pub struct FileContentParams {
    pub path: String,
}

fn content_type_for_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "txt" | "log" | "csv" | "tsv" | "conf" => "text/plain; charset=utf-8",
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "c" | "cpp" | "h" | "hpp"
        | "go" | "rb" | "php" | "swift" | "kt" | "scala" | "sh" | "bash" | "zsh"
        | "fish" | "ps1" | "bat" | "cmd" | "sql" | "r" | "lua" | "perl" | "pl"
        | "html" | "htm" | "css" | "scss" | "less" | "xml" | "json" | "yaml" | "yml"
        | "toml" | "ini" | "cfg" | "md" | "rst" | "tex" | "vue" | "svelte" => {
            "text/plain; charset=utf-8"
        }
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn ext_from_path(path: &str) -> &str {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    match name.rsplit('.').next() {
        Some(ext) if ext != name && !ext.is_empty() => ext,
        _ => "",
    }
}

pub async fn file_content_handler(
    State(state): State<AppState>,
    Query(params): Query<FileContentParams>,
) -> Result<Response, (StatusCode, String)> {
    let _guard = IndexerPauseGuard::new(&state);
    let raw_path = params.path.replace("//", "/");
    let cwd = state.cwd.trim_end_matches('/');

    // Support both absolute and relative paths
    let full_path = if raw_path.starts_with('/') {
        // Absolute path - check it's under CWD
        if !raw_path.starts_with(cwd) {
            return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
        }
        raw_path
    } else {
        format!("{}/{}", cwd, raw_path)
    };

    let metadata = tokio::fs::metadata(&full_path).await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {}", e)))?;

    let ext = ext_from_path(&full_path).to_lowercase();
    let mime = content_type_for_ext(&ext);
    let is_dir = metadata.is_dir();

    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", metadata.permissions().mode())
    };
    #[cfg(not(unix))]
    let permissions = "N/A".to_string();

    let modified = metadata.modified().ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    let created = metadata.created().ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    let parent = std::path::Path::new(&full_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_string();

    let is_text = mime.starts_with("text/") || mime == "application/json"
        || mime == "application/xml" || mime == "image/svg+xml";

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_str(mime).unwrap());
    headers.insert("x-file-size", HeaderValue::from_str(&metadata.len().to_string()).unwrap());
    headers.insert("x-is-text", HeaderValue::from_str(if is_text { "true" } else { "false" }).unwrap());
    headers.insert("x-ext", HeaderValue::from_str(&ext).unwrap());
    headers.insert("x-mime", HeaderValue::from_str(mime).unwrap());
    headers.insert("x-is-dir", HeaderValue::from_str(if is_dir { "true" } else { "false" }).unwrap());
    headers.insert("x-permissions", HeaderValue::from_str(&permissions).unwrap());
    headers.insert("x-modified", HeaderValue::from_str(&modified).unwrap());
    headers.insert("x-created", HeaderValue::from_str(&created).unwrap());
    headers.insert("x-parent", HeaderValue::from_str(&parent).unwrap());

    if is_text {
        let content = tokio::fs::read_to_string(&full_path).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {}", e)))?;
        Ok((headers, content).into_response())
    } else {
        let content = tokio::fs::read(&full_path).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {}", e)))?;
        headers.insert("content-length", HeaderValue::from_str(&content.len().to_string()).unwrap());
        Ok((headers, Body::from(content)).into_response())
    }
}

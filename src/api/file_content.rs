use axum::extract::{Query, State};
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::states::app_state::AppState;

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
    path.rsplit('.').next().unwrap_or("")
}

pub async fn file_content_handler(
    State(state): State<AppState>,
    Query(params): Query<FileContentParams>,
) -> Result<Response, (StatusCode, String)> {
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

    let is_text = mime.starts_with("text/") || mime == "application/json"
        || mime == "application/xml" || mime == "image/svg+xml";

    if is_text {
        let content = tokio::fs::read_to_string(&full_path).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {}", e)))?;

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_str(mime).unwrap());
        headers.insert("x-file-size", HeaderValue::from_str(&metadata.len().to_string()).unwrap());
        headers.insert("x-is-text", HeaderValue::from_static("true"));

        Ok((headers, content).into_response())
    } else {
        let content = tokio::fs::read(&full_path).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Read error: {}", e)))?;

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_str(mime).unwrap());
        headers.insert("content-length", HeaderValue::from_str(&content.len().to_string()).unwrap());
        headers.insert("x-file-size", HeaderValue::from_str(&metadata.len().to_string()).unwrap());
        headers.insert("x-is-text", HeaderValue::from_static("false"));

        Ok((headers, Body::from(content)).into_response())
    }
}

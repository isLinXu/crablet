//! Knowledge base-related web handlers
//!
//! Handles document management, RAG search, and knowledge ingestion.

use std::sync::Arc;
use axum::{
    extract::{State, Json, Multipart, Query},
    http::StatusCode,
};
use serde::Deserialize;

use crate::gateway::server::CrabletGateway;

#[cfg(feature = "knowledge")]
use std::io::Write;

#[derive(Deserialize)]
pub struct GetChunksQuery {
    source: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
    limit: Option<usize>,
}

pub async fn list_documents(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let _ = gateway;
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        if let Ok(docs) = ingestion.list_documents().await {
            return Json(serde_json::json!({
                "status": "success",
                "documents": docs
            }));
        }
    }
    Json(serde_json::json!({
        "status": "success",
        "documents": []
    }))
}

pub async fn get_document_chunks(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<GetChunksQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        if let Ok(chunks) = ingestion.get_document_chunks(&query.source).await {
            return Ok(Json(serde_json::json!({
                "status": "success",
                "chunks": chunks
            })));
        }
    }

    let _ = gateway;
    let _ = query;

    Ok(Json(serde_json::json!({
        "status": "success",
        "chunks": []
    })))
}

pub async fn search_knowledge(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        if let Ok(results) = ingestion.search(&query.q, query.limit.unwrap_or(5)).await {
            return Ok(Json(serde_json::json!({
                "status": "success",
                "results": results
            })));
        }
    }

    let _ = gateway;
    let _ = query;

    Ok(Json(serde_json::json!({
        "status": "success",
        "results": []
    })))
}

pub async fn upload_knowledge(
    State(gateway): State<Arc<CrabletGateway>>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let _ = gateway;
    let _ = multipart;
    let allowed = [
        "pdf", "doc", "docx", "txt", "md", "csv", "xls", "xlsx",
        "jpg", "jpeg", "png", "gif", "svg", "webp",
        "mp3", "wav", "m4a", "flac",
        "mp4", "avi", "mov", "mkv",
    ];
    let max_size: usize = 300 * 1024 * 1024;
    let _ = allowed;
    let _ = max_size;

    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        let mut multipart = multipart;
        let mut uploaded_files = Vec::new();

        while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
            let file_name = field.file_name().unwrap_or("unknown").to_string();
            if file_name == "unknown" { continue; }
            let ext = file_name
                .split('.')
                .next_back()
                .unwrap_or("")
                .to_lowercase();
            if !allowed.contains(&ext.as_str()) {
                tracing::warn!("Blocked unsupported extension: {}", file_name);
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }

            let data = field.bytes().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if data.len() > max_size {
                tracing::warn!("Blocked oversize upload: {} bytes for {}", data.len(), file_name);
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }
            if data.len() >= 2 && data[0] == 0x4d && data[1] == 0x5a {
                tracing::warn!("Blocked executable signature (MZ): {}", file_name);
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
            if data.len() >= 4 && data[0] == 0x7f && data[1] == 0x45 && data[2] == 0x4c && data[3] == 0x46 {
                tracing::warn!("Blocked executable signature (ELF): {}", file_name);
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }

            let temp_dir = std::env::temp_dir();
            let temp_path = temp_dir.join(&file_name);

            {
                let mut file = std::fs::File::create(&temp_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                file.write_all(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }

            let metadata = serde_json::json!({
                "source": file_name,
                "uploaded_at": chrono::Utc::now().to_rfc3339()
            });

            match ingestion.ingest_file(&temp_path, metadata).await {
                Ok(doc_id) => {
                    uploaded_files.push(file_name.clone());
                    tracing::info!("Ingested {} as {}", file_name, doc_id);
                },
                Err(e) => {
                    tracing::error!("Ingestion failed for {}: {}", file_name, e);
                    let _ = std::fs::remove_file(&temp_path);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }

            let _ = std::fs::remove_file(temp_path);
        }
        return Ok(Json(serde_json::json!({
            "status": "success",
            "uploaded": uploaded_files
        })));
    }

    #[cfg(feature = "knowledge")]
    return Err(StatusCode::NOT_IMPLEMENTED);

    #[cfg(not(feature = "knowledge"))]
    Err(StatusCode::NOT_IMPLEMENTED)
}
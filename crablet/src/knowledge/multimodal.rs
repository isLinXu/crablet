use anyhow::Result;
use serde_json::json;
use std::path::Path;
use tracing::{info, warn};
use crate::sandbox::docker::DockerExecutor;

#[derive(Clone)]
pub struct ProcessedContent {
    pub text: String,
    pub metadata: serde_json::Value,
}

async fn run_cmd_in_docker(image: &str, program: &str, args: &[&str], work_dir: &Path) -> Option<String> {
    let executor = DockerExecutor::strict()
        .with_work_dir(work_dir.to_string_lossy().to_string())
        .with_timeout(30);

    let mut full_cmd = vec![program];
    full_cmd.extend(args.iter().cloned());

    info!("Running {} in Docker sandbox (image: {})", program, image);
    
    match executor.execute(image, &full_cmd).await {
        Ok(result) => {
            if result.success {
                let t = result.stdout.trim().to_string();
                if t.is_empty() { None } else { Some(t) }
            } else {
                warn!("Command {} failed in Docker: {}", program, result.stderr);
                None
            }
        }
        Err(e) => {
            warn!("Failed to execute {} in Docker: {}", program, e);
            None
        }
    }
}

pub async fn process_file(path: &Path, ext: &str) -> Result<ProcessedContent> {
    let source = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
    let parent_dir = path.parent().unwrap_or(Path::new("."));
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let lower = ext.to_lowercase();
    
    if ["txt", "md", "markdown", "csv"].contains(&lower.as_str()) {
        let text = tokio::fs::read_to_string(path).await?;
        return Ok(ProcessedContent {
            text,
            metadata: json!({
                "source_type": "document",
                "extraction_method": "plain_text",
                "retrieval_explainability": "全文切片+向量召回",
                "source_trace": source
            }),
        });
    }
    if ["pdf"].contains(&lower.as_str()) {
        return Ok(ProcessedContent {
            text: crate::knowledge::pdf::PdfParser::extract_text(&path.to_string_lossy())?,
            metadata: json!({
                "source_type": "document",
                "extraction_method": "pdf_parser",
                "retrieval_explainability": "PDF抽取文本+语义切片",
                "source_trace": source
            }),
        });
    }
    if ["doc", "docx", "xls", "xlsx"].contains(&lower.as_str()) {
        let bytes = tokio::fs::read(path).await?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        return Ok(ProcessedContent {
            text: format!("structured_file:{}\n{}", source, text),
            metadata: json!({
                "source_type": "document",
                "extraction_method": "binary_to_text_fallback",
                "retrieval_explainability": "结构化文件回退抽取",
                "source_trace": source
            }),
        });
    }
    if ["jpg", "jpeg", "png", "gif", "svg", "webp"].contains(&lower.as_str()) {
        // 使用包含 tesseract 的 Docker 镜像
        let ocr = run_cmd_in_docker("jbarlow83/ocrmypdf", "tesseract", &[file_name, "stdout"], parent_dir).await.unwrap_or_default();
        let extraction = if ocr.is_empty() { "ocr_fallback" } else { "tesseract_ocr" };
        return Ok(ProcessedContent {
            text: if ocr.is_empty() {
                format!("image_asset:{}\nocr_unavailable\nvisual_feature_placeholder", source)
            } else {
                format!("image_asset:{}\n{}\nvisual_feature_placeholder", source, ocr)
            },
            metadata: json!({
                "source_type": "image",
                "extraction_method": extraction,
                "retrieval_explainability": "OCR文本与视觉特征检索",
                "source_trace": source
            }),
        });
    }
    if ["mp3", "wav", "m4a", "flac"].contains(&lower.as_str()) {
        let ffprobe = run_cmd("ffprobe", &[
            "-v", "error", "-show_entries", "format=duration:stream=codec_name",
            "-of", "default=noprint_wrappers=1", file_name
        ], parent_dir).await.unwrap_or_else(|| "ffprobe_unavailable".to_string());
        
        return Ok(ProcessedContent {
            text: format!("audio_asset:{}\nmetadata:\n{}\ntranscript_placeholder", source, ffprobe),
            metadata: json!({
                "source_type": "audio",
                "extraction_method": "audio_metadata_plus_asr_placeholder",
                "retrieval_explainability": "音频元信息+转写检索",
                "source_trace": source
            }),
        });
    }
    if ["mp4", "avi", "mov", "mkv"].contains(&lower.as_str()) {
        let ffprobe = run_cmd_in_docker("jrottenberg/ffmpeg", "ffprobe", &[
            "-v", "error", "-show_entries", "format=duration:stream=codec_name,width,height",
            "-of", "default=noprint_wrappers=1", file_name
        ], parent_dir).await.unwrap_or_else(|| "ffprobe_unavailable".to_string());
        
        return Ok(ProcessedContent {
            text: format!("video_asset:{}\nmetadata:\n{}\nsubtitle_placeholder\nkeyframe_placeholder", source, ffprobe),
            metadata: json!({
                "source_type": "video",
                "extraction_method": "video_metadata_plus_subtitle_keyframe_placeholder",
                "retrieval_explainability": "关键帧/字幕与元信息联合检索",
                "source_trace": source
            }),
        });
    }
    Err(anyhow::anyhow!("Unsupported file extension: {}", ext))
}

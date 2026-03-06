use anyhow::Result;
use serde_json::json;
use std::path::Path;
use std::process::Command;

#[derive(Clone)]
pub struct ProcessedContent {
    pub text: String,
    pub metadata: serde_json::Value,
}

fn run_cmd(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let t = s.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

pub fn process_file(path: &Path, ext: &str) -> Result<ProcessedContent> {
    let source = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
    let lower = ext.to_lowercase();
    if ["txt", "md", "markdown", "csv"].contains(&lower.as_str()) {
        let text = std::fs::read_to_string(path)?;
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
        let bytes = std::fs::read(path)?;
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
        let ocr = run_cmd("tesseract", &[&path.to_string_lossy(), "stdout"]).unwrap_or_default();
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
            "-of", "default=noprint_wrappers=1", &path.to_string_lossy()
        ]).unwrap_or_else(|| "ffprobe_unavailable".to_string());
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
        let ffprobe = run_cmd("ffprobe", &[
            "-v", "error", "-show_entries", "format=duration:stream=codec_name,width,height",
            "-of", "default=noprint_wrappers=1", &path.to_string_lossy()
        ]).unwrap_or_else(|| "ffprobe_unavailable".to_string());
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

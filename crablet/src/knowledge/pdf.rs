use anyhow::{Result, Context};
use std::path::Path;

pub struct PdfParser;

impl PdfParser {
    pub fn extract_text(path: &str) -> Result<String> {
        let path = Path::new(path);
        if !path.exists() {
            return Err(anyhow::anyhow!("File not found: {:?}", path));
        }

        let content = pdf_extract::extract_text(path)
            .context("Failed to extract text from PDF")?;
            
        Ok(content)
    }
    
    // Future: Add method to extract images and metadata
}

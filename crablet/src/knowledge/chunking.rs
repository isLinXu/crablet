use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub content: String,
    pub metadata: std::collections::HashMap<String, String>,
}

pub trait Chunker: Send + Sync {
    fn chunk(&self, text: &str) -> Result<Vec<Chunk>>;
}

pub struct RecursiveCharacterChunker {
    chunk_size: usize,
    chunk_overlap: usize,
    separators: Vec<String>,
}

impl RecursiveCharacterChunker {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
            separators: vec!["\n\n".to_string(), "\n".to_string(), " ".to_string(), "".to_string()],
        }
    }

    fn split_text(&self, text: &str, separator: &str) -> Vec<String> {
        if separator.is_empty() {
            return text.chars().map(|c| c.to_string()).collect();
        }
        text.split(separator).map(|s| s.to_string()).collect()
    }
    
    fn merge_splits(&self, splits: &[String], separator: &str) -> Vec<String> {
        let mut docs = Vec::new();
        let mut current_doc: Vec<String> = Vec::new();
        let mut total = 0;
        let separator_len = separator.len();

        for split in splits {
            let len = split.len();
            if total + len + (if !current_doc.is_empty() { separator_len } else { 0 }) > self.chunk_size {
                if !current_doc.is_empty() {
                    let doc = current_doc.join(separator);
                    if !doc.trim().is_empty() {
                        docs.push(doc);
                    }
                    
                    // Handle overlap
                    while total > self.chunk_overlap || (total + len + separator_len > self.chunk_size && total > 0) {
                        if !current_doc.is_empty() {
                            total -= current_doc[0].len() + (if current_doc.len() > 1 { separator_len } else { 0 });
                            current_doc.remove(0);
                        } else {
                            break;
                        }
                    }
                }
            }
            
            current_doc.push(split.clone());
            total += len + (if current_doc.len() > 1 { separator_len } else { 0 });
        }
        
        if !current_doc.is_empty() {
            let doc = current_doc.join(separator);
            if !doc.trim().is_empty() {
                docs.push(doc);
            }
        }
        
        docs
    }
    
    fn _chunk(&self, text: &str, separators: &[String]) -> Vec<String> {
        let mut final_chunks = Vec::new();
        let mut separator = separators.last().unwrap().as_str();
        let mut new_separators = Vec::new();
        
        for (i, sep) in separators.iter().enumerate() {
            if text.contains(sep) {
                separator = sep;
                new_separators = separators[i+1..].to_vec();
                break;
            }
        }
        
        let splits = self.split_text(text, separator);
        let mut good_splits = Vec::new();
        
        for split in splits {
            if split.len() < self.chunk_size {
                good_splits.push(split);
            } else {
                if !good_splits.is_empty() {
                    final_chunks.extend(self.merge_splits(&good_splits, separator));
                    good_splits.clear();
                }
                if !new_separators.is_empty() {
                    final_chunks.extend(self._chunk(&split, &new_separators));
                } else {
                    final_chunks.push(split);
                }
            }
        }
        
        if !good_splits.is_empty() {
            final_chunks.extend(self.merge_splits(&good_splits, separator));
        }
        
        final_chunks
    }
}

impl Chunker for RecursiveCharacterChunker {
    fn chunk(&self, text: &str) -> Result<Vec<Chunk>> {
        let texts = self._chunk(text, &self.separators);
        Ok(texts.into_iter().map(|t| Chunk {
            content: t,
            metadata: std::collections::HashMap::new(),
        }).collect())
    }
}

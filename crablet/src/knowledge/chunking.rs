use crate::error::CrabletError;
use serde::{Deserialize, Serialize};
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub content: String,
    pub metadata: std::collections::HashMap<String, String>,
}

pub trait Chunker: Send + Sync {
    fn chunk(&self, text: &str) -> Result<Vec<Chunk>, CrabletError>;
}

pub struct MarkdownChunker {
    chunk_size: usize,
}

impl MarkdownChunker {
    pub fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }

    fn is_header(line: &str) -> Option<(usize, String)> {
        let re = Regex::new(r"^(#{1,6})\s+(.*)").expect("Invalid regex in MarkdownChunker");
        if let Some(cap) = re.captures(line) {
            let level = cap[1].len();
            let text = cap[2].to_string();
            return Some((level, text));
        }
        None
    }
}

impl Chunker for MarkdownChunker {
    fn chunk(&self, text: &str) -> Result<Vec<Chunk>, CrabletError> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut current_headers: Vec<String> = Vec::new(); // Stack of headers
        
        for line in text.lines() {
            if let Some((level, title)) = Self::is_header(line) {
                // If we have content, push it
                if !current_chunk.trim().is_empty() {
                    let mut meta = std::collections::HashMap::new();
                    meta.insert("headers".to_string(), current_headers.join(" > "));
                    chunks.push(Chunk {
                        content: current_chunk.clone(),
                        metadata: meta,
                    });
                    current_chunk.clear();
                }
                
                // Update header stack
                // If level is <= current stack depth, pop until we are at right level
                // e.g. H1 -> H2 -> H3. New H2. Stack becomes H1 -> H2(new).
                // But markdown levels can jump.
                // Simple logic: maintain headers up to current level.
                if level <= current_headers.len() {
                    current_headers.truncate(level - 1);
                }
                current_headers.push(title);
                
                // Add header to chunk content for context?
                // Usually good to keep header in content too.
                current_chunk.push_str(line);
                current_chunk.push('\n');
            } else {
                // Check size limit
                if current_chunk.len() + line.len() > self.chunk_size && !current_chunk.trim().is_empty() {
                     let mut meta = std::collections::HashMap::new();
                     meta.insert("headers".to_string(), current_headers.join(" > "));
                     chunks.push(Chunk {
                        content: current_chunk.clone(),
                        metadata: meta,
                     });
                     current_chunk.clear();
                }
                current_chunk.push_str(line);
                current_chunk.push('\n');
            }
        }
        
        if !current_chunk.trim().is_empty() {
             let mut meta = std::collections::HashMap::new();
             meta.insert("headers".to_string(), current_headers.join(" > "));
             chunks.push(Chunk {
                content: current_chunk,
                metadata: meta,
             });
        }
        
        Ok(chunks)
    }
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
            if total + len + (if !current_doc.is_empty() { separator_len } else { 0 }) > self.chunk_size
                && !current_doc.is_empty() {
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
    fn chunk(&self, text: &str) -> Result<Vec<Chunk>, CrabletError> {
        let texts = self._chunk(text, &self.separators);
        Ok(texts.into_iter().map(|t| Chunk {
            content: t,
            metadata: std::collections::HashMap::new(),
        }).collect())
    }
}

use anyhow::Result;
use fastembed::{TextRerank, RerankInitOptions, RerankerModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredDocument {
    pub content: String,
    pub score: f32,
    pub index: usize, // Original index in the list
}

pub trait Reranker: Send + Sync {
    fn rerank(&self, query: &str, documents: &[String], top_k: usize) -> Result<Vec<ScoredDocument>>;
}

pub struct FastEmbedReranker {
    model: std::sync::Mutex<TextRerank>,
}

impl FastEmbedReranker {
    pub fn new() -> Result<Self> {
        let mut options = RerankInitOptions::default();
        options.model_name = RerankerModel::BGERerankerBase;
        options.show_download_progress = true;
        
        let model = TextRerank::try_new(options)?;
        Ok(Self { model: std::sync::Mutex::new(model) })
    }
}

impl Reranker for FastEmbedReranker {
    fn rerank(&self, query: &str, documents: &[String], top_k: usize) -> Result<Vec<ScoredDocument>> {
        let docs_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();
        // fastembed rerank returns Vec<RerankResult>
        let results = {
            let _model = self.model.lock().map_err(|e| anyhow::anyhow!("Reranker lock poisoned: {}", e))?;
            // If rerank takes &mut self, we need mutable access.
            // But Mutex gives us that if we have mut guard.
            // Actually, if model is Mutex<T>, lock() gives MutexGuard<T>.
            // If T::rerank takes &mut self, we might need to declare `let mut model = ...`.
            // But MutexGuard implements DerefMut.
            // However, calling a method that requires &mut self on a temporary might need explicit mut binding?
            // Let's see.
            // Wait, TextRerank might need to be mutable.
            // Assuming it does:
            // let mut model = ...;
            // model.rerank(...)
            // But wait, if `rerank` takes `&self` (immutable), then `Mutex` is not strictly needed for mutability, but for Sync if TextRerank is not Sync.
            // FastEmbed models are usually Send+Sync?
            // If `rerank` takes `&mut self`, then `Mutex` is required.
            // Let's assume it takes `&self` based on docs, but maybe I am wrong.
            // The previous error was explicit about "cannot borrow as mutable". So it takes `&mut self`.
            // So we need Mutex.
            
            // To call a mutable method on the inner value of a MutexGuard, the guard itself usually needs to be mutable?
            // No, MutexGuard provides interior mutability.
            // But to get `&mut T` from `MutexGuard<T>`, we use `DerefMut`.
            // Calling `model.rerank()` works if `model` is the guard.
            // However, Rust's method resolution will try to borrow from the guard.
            // If `rerank` needs `&mut self`, we need `&mut *model`.
            // MutexGuard allows `&mut T` via `DerefMut`.
            // So `model.rerank(...)` should work if `model` is `MutexGuard`.
            // EXCEPT if `model` binding itself is not mutable?
            // MutexGuard has interior mutability semantics regarding the data, but the guard binding...
            // Actually `lock()` returns a value. If we want to mutate via it, we usually need `let mut guard`.
            // Let's try `let mut model = ...`.
            
            // But wait, `self.model.lock()` requires `&self.model`.
            // `lock` takes `&self`.
            // So `let mut model = self.model.lock()...` is correct.
            
            // Wait, if I use `let mut model`, then I can call `model.rerank()`.
            // If I use `let model`, and `rerank` takes `&mut self`, then `model.rerank()` might fail if `model` is not `mut`.
            
            // So:
            // let mut model = self.model.lock()...
            
            // But wait, `TextRerank` in 5.9.0?
            // Let's just try wrapping in Mutex and calling it.
            
            // Also need to handle the case where `rerank` is not found if I don't deref correctly? No, auto-deref handles it.
            
            // One detail: if `rerank` takes `&self`, Mutex is overhead but safe.
            // If it takes `&mut self`, Mutex is required.
            
            // Wait, previous error said `cannot borrow self.model as mutable`.
            // This means `self` was `&FastEmbedReranker`, so `self.model` was `&TextRerank`.
            // And `rerank` wanted `&mut TextRerank`.
            // So yes, `TextRerank::rerank` takes `&mut self`.
            
            // So Mutex is the way.
            
            // And yes `let mut model` is needed.
            
            // Let's verify imports.
            
            // Code:
            /*
            let mut model = self.model.lock().map_err(|e| anyhow::anyhow!("Reranker lock poisoned: {}", e))?;
            model.rerank(query, docs_refs, true, None)?
            */
            
            // But wait, `TextRerank` in fastembed 5.9.0.
            // If `TextRerank` is not `Send`, `Mutex` won't work across threads if `FastEmbedReranker` is `Send`.
            // `ort` (onnx runtime) sessions are usually `Send + Sync`.
            // `TextRerank` likely wraps an ONNX session.
            
            // Let's assume it works.
            
            // Correction: I need to use `std::sync::Mutex`.
            
            let mut model = self.model.lock().map_err(|e| anyhow::anyhow!("Reranker lock poisoned: {}", e))?;
            model.rerank(query, docs_refs, true, None)?
        };
        
        let mut scored_docs = Vec::new();
        for res in results {
            scored_docs.push(ScoredDocument {
                content: documents[res.index].clone(),
                score: res.score,
                index: res.index,
            });
        }
        
        // Sort by score desc (fastembed might already do this but let's ensure)
        scored_docs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored_docs.truncate(top_k);
        
        Ok(scored_docs)
    }
}

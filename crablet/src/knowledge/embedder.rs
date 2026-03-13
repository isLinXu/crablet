use anyhow::Result;
use fastembed::{TextEmbedding, InitOptions};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct EmbedderPool {
    pool: Vec<Arc<Mutex<TextEmbedding>>>,
    next: AtomicUsize,
    mock_mode: bool,
}

impl EmbedderPool {
    pub fn new(size: usize, options: InitOptions) -> Result<Self> {
        let mut pool = Vec::with_capacity(size);
        for _ in 0..size {
             let opts = options.clone();
             let embedder = TextEmbedding::try_new(opts)?;
             pool.push(Arc::new(Mutex::new(embedder)));
        }
        Ok(Self { pool, next: AtomicUsize::new(0), mock_mode: false })
    }
    
    pub fn new_mock() -> Self {
        Self { pool: Vec::new(), next: AtomicUsize::new(0), mock_mode: true }
    }

    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if self.mock_mode {
            // Return dummy vectors of size 384 (all-MiniLM-L6-v2 size)
            return Ok(texts.iter().map(|_| vec![0.1; 384]).collect());
        }

        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.pool.len();
        let embedder = self.pool[idx].clone();
        
        tokio::task::spawn_blocking(move || {
            let mut embedder = embedder.lock().map_err(|e| anyhow::anyhow!("Embedder lock poisoned: {}", e))?;
            embedder.embed(texts, None)
        }).await?
    }
}

#[derive(Clone)]
pub struct Embedder {
    pool: Arc<EmbedderPool>,
}

impl Embedder {
    pub fn new(pool: Arc<EmbedderPool>) -> Self {
        Self { pool }
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let res = self.pool.embed(vec![text.to_string()]).await?;
        Ok(res[0].clone())
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.pool.embed(texts).await
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a * norm_b)
}

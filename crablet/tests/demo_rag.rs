use anyhow::Result;
use crablet::memory::semantic::{SqliteKnowledgeGraph, KnowledgeGraph};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

#[tokio::test]
async fn test_demo_f_rag_knowledge_graph() -> Result<()> {
    // 1. Setup In-Memory SQLite DB
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await?;
        
    let kg = SqliteKnowledgeGraph::new(pool).await?;
    
    // 2. Ingest Knowledge
    println!("Ingesting knowledge...");
    kg.add_entity("Crablet", "Project").await?;
    kg.add_entity("Rust", "Language").await?;
    kg.add_entity("High Performance", "Feature").await?;
    
    kg.add_relation("Crablet", "Rust", "written_in").await?;
    kg.add_relation("Crablet", "High Performance", "has_feature").await?;
    
    // 3. Query Knowledge (Simulate RAG Retrieval)
    println!("Querying: What is Crablet written in?");
    let relations = kg.find_related("Crablet").await?;
    
    println!("Relations found for Crablet:");
    for (dir, rel, target) in &relations {
        println!("{} {} {}", dir, rel, target);
    }
    
    // 4. Verify
    assert!(relations.iter().any(|(_, rel, target)| rel == "written_in" && target == "Rust"));
    assert!(relations.iter().any(|(_, rel, target)| rel == "has_feature" && target == "High Performance"));
    
    println!("Demo F Verified Successfully!");
    
    Ok(())
}

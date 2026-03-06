#![cfg(feature = "knowledge")]

use crablet::knowledge::vector_store::VectorStore;
use serde_json::json;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn test_vector_store_in_memory() {
    let store = VectorStore::new_in_memory();
    
    // 1. Add document
    store.add_document("Hello world", Some(json!({"source": "doc1"}))).await.expect("Add failed");
    
    // 2. Search
    let results = store.search("Hello", 1).await.expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "Hello world");
    
    // 3. Delete
    store.delete_document("doc1").await.expect("Delete failed");
    let results = store.search("Hello", 1).await.expect("Search failed");
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_vector_store_sqlite() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create pool");
        
    // This might fail if migrations are not found relative to the test binary if macro didn't embed them properly?
    // But sqlx macro usually embeds.
    let store = VectorStore::new(pool).await.expect("Failed to create store with migrations");
    
    store.add_document("Hello sqlite", Some(json!({"source": "doc2"}))).await.expect("Add failed");
    
    let results = store.search("Hello", 1).await.expect("Search failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "Hello sqlite");
    
    // Test listing
    let docs = store.list_documents().await.expect("List failed");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["source"], "doc2");
}

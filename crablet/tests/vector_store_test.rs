use crablet::knowledge::chunking::{Chunker, RecursiveCharacterChunker};

#[tokio::test]
async fn test_chunk_text() {
    let chunker = RecursiveCharacterChunker::new(20, 5);
    let text = "This is a sentence. This is another sentence. And a third one.";
    
    // Window size 20, overlap 5
    let chunks = chunker.chunk(text).unwrap();
    
    assert!(!chunks.is_empty());
    assert!(chunks[0].content.len() <= 20);
    
    // Verify content preservation
    let reassembled = chunks.iter().map(|c| c.content.clone()).collect::<Vec<_>>().join("");
    assert!(reassembled.len() >= text.len());
}

#[tokio::test]
async fn test_chunk_text_overlap() {
    let chunker = RecursiveCharacterChunker::new(5, 2);
    let text = "1234567890";
    
    let chunks = chunker.chunk(text).unwrap();
    
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].content, "12345");
    assert_eq!(chunks[1].content, "45678");
    assert_eq!(chunks[2].content, "7890");
}

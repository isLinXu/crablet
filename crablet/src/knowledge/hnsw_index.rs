//! HNSW (Hierarchical Navigable Small World) Vector Index
//! 
//! Provides approximate nearest neighbor search with O(log N) complexity,
//! significantly faster than brute force O(N) search for large datasets.

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};

/// Configuration for HNSW index
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Maximum number of connections per layer (M parameter)
    pub max_connections: usize,
    /// Size of dynamic candidate list (efConstruction)
    pub ef_construction: usize,
    /// Size of dynamic candidate list for search (efSearch)
    pub ef_search: usize,
    /// Number of layers in the hierarchy
    pub max_layers: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            ef_construction: 200,
            ef_search: 50,
            max_layers: 16,
        }
    }
}

/// A node in the HNSW graph
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HnswNode {
    id: String,
    vector: Vec<f32>,
    /// Connections for each layer
    connections: Vec<Vec<String>>,
    /// Maximum layer this node belongs to
    max_layer: usize,
}

/// Distance metric for vector comparison
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

impl DistanceMetric {
    fn calculate(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            DistanceMetric::Cosine => cosine_distance(a, b),
            DistanceMetric::Euclidean => euclidean_distance(a, b),
            DistanceMetric::DotProduct => -dot_product(a, b), // Negative for min-heap
        }
    }
}

/// HNSW Vector Index
pub struct HnswIndex {
    config: HnswConfig,
    metric: DistanceMetric,
    /// All nodes in the index
    nodes: Arc<RwLock<HashMap<String, HnswNode>>>,
    /// Entry point for search (node with maximum layer)
    entry_point: Arc<RwLock<Option<String>>>,
    /// Current maximum layer in the graph
    max_layer: Arc<RwLock<usize>>,
    /// Dimension of vectors
    dimension: Arc<RwLock<usize>>,
}

impl HnswIndex {
    /// Create a new HNSW index with default configuration
    pub fn new() -> Self {
        Self::with_config(HnswConfig::default(), DistanceMetric::Cosine)
    }

    /// Create a new HNSW index with custom configuration
    pub fn with_config(config: HnswConfig, metric: DistanceMetric) -> Self {
        Self {
            config,
            metric,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            entry_point: Arc::new(RwLock::new(None)),
            max_layer: Arc::new(RwLock::new(0)),
            dimension: Arc::new(RwLock::new(0)),
        }
    }

    /// Insert a vector into the index
    pub fn insert(&self, id: String, vector: Vec<f32>) -> Result<()> {
        // Validate dimension
        let mut dim = self.dimension.write();
        if *dim == 0 {
            *dim = vector.len();
        } else if *dim != vector.len() {
            return Err(anyhow!(
                "Vector dimension mismatch: expected {}, got {}",
                *dim,
                vector.len()
            ));
        }
        drop(dim);

        // Generate random level for this node
        let level = self.random_level();
        
        // Create node
        let node = HnswNode {
            id: id.clone(),
            vector: vector.clone(),
            connections: vec![Vec::new(); level + 1],
            max_layer: level,
        };

        let mut nodes = self.nodes.write();
        
        // If this is the first node, set it as entry point
        let mut entry = self.entry_point.write();
        let mut max_l = self.max_layer.write();
        
        if entry.is_none() {
            *entry = Some(id.clone());
            *max_l = level;
            nodes.insert(id, node);
            return Ok(());
        }

        let entry_id = entry.clone().unwrap();
        
        // Update entry point if this node has higher level
        if level > *max_l {
            *max_l = level;
            *entry = Some(id.clone());
        }

        // Insert at each layer from top to bottom
        let mut current_entry = entry_id;
        
        for layer in (0..=level.min(*max_l)).rev() {
            // Search for nearest neighbors at this layer
            let neighbors = self.search_layer(&nodes, &vector, &current_entry, layer, self.config.max_connections);
            
            // Connect to neighbors
            if let Some(node) = nodes.get_mut(&id) {
                if layer < node.connections.len() {
                    node.connections[layer] = neighbors.clone();
                }
            }
            
            // Bidirectional connections
            for neighbor_id in &neighbors {
                if let Some(neighbor) = nodes.get_mut(neighbor_id) {
                    if layer < neighbor.connections.len() {
                        if !neighbor.connections[layer].contains(&id) {
                            neighbor.connections[layer].push(id.clone());
                            // Trim if exceeds max connections
                            if neighbor.connections[layer].len() > self.config.max_connections {
                                neighbor.connections[layer].truncate(self.config.max_connections);
                            }
                        }
                    }
                }
            }
            
            // Set entry for next (lower) layer
            if !neighbors.is_empty() {
                current_entry = neighbors[0].clone();
            }
        }

        nodes.insert(id, node);
        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        let nodes = self.nodes.read();
        
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let entry = self.entry_point.read();
        let max_l = *self.max_layer.read();
        
        let entry_id = entry.clone().unwrap();
        let mut current = entry_id.clone();

        // Search from top layer down to layer 0
        for layer in (1..=max_l).rev() {
            current = self.greedy_search_layer(&nodes, query, &current, layer);
        }

        // Final search at layer 0 with efSearch
        let candidates = self.search_layer(&nodes, query, &current, 0, self.config.ef_search.max(k));
        
        // Calculate exact distances and sort
        let mut results: Vec<(String, f32)> = candidates
            .into_iter()
            .filter_map(|id| {
                nodes.get(&id).map(|node| {
                    let dist = self.metric.calculate(query, &node.vector);
                    (id, dist)
                })
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(k);

        Ok(results)
    }

    /// Delete a vector from the index
    pub fn delete(&self, id: &str) -> Result<()> {
        let mut nodes = self.nodes.write();
        
        if let Some(node) = nodes.remove(id) {
            // Remove connections from other nodes
            for layer in 0..=node.max_layer {
                if layer < node.connections.len() {
                    for neighbor_id in &node.connections[layer] {
                        if let Some(neighbor) = nodes.get_mut(neighbor_id) {
                            if layer < neighbor.connections.len() {
                                neighbor.connections[layer].retain(|x| x != id);
                            }
                        }
                    }
                }
            }
        }

        // Update entry point if necessary
        let mut entry = self.entry_point.write();
        if entry.as_ref() == Some(id) {
            // Find new entry point (node with max layer)
            let mut new_entry: Option<(String, usize)> = None;
            for (node_id, node) in nodes.iter() {
                if new_entry.is_none() || node.max_layer > new_entry.as_ref().unwrap().1 {
                    new_entry = Some((node_id.clone(), node.max_layer));
                }
            }
            *entry = new_entry.map(|(id, _)| id);
            
            if let Some((_, max_l)) = new_entry {
                *self.max_layer.write() = max_l;
            } else {
                *self.max_layer.write() = 0;
            }
        }

        Ok(())
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        self.nodes.read().len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.read().is_empty()
    }

    /// Clear all vectors from the index
    pub fn clear(&self) {
        self.nodes.write().clear();
        *self.entry_point.write() = None;
        *self.max_layer.write() = 0;
        *self.dimension.write() = 0;
    }

    // Helper methods

    fn random_level(&self) -> usize {
        let mut level = 0;
        let m_l = 1.0 / (self.config.max_connections as f64).ln();
        let mut rng = fastrand::Rng::new();
        
        while rng.f64() < m_l && level < self.config.max_layers {
            level += 1;
        }
        
        level
    }

    fn greedy_search_layer(
        &self,
        nodes: &HashMap<String, HnswNode>,
        query: &[f32],
        entry_id: &str,
        layer: usize,
    ) -> String {
        let mut current = entry_id.to_string();
        let mut current_dist = self.metric.calculate(
            query,
            &nodes.get(&current).unwrap().vector,
        );

        loop {
            let node = nodes.get(&current).unwrap();
            if layer >= node.connections.len() {
                break;
            }

            let mut improved = false;
            for neighbor_id in &node.connections[layer] {
                if let Some(neighbor) = nodes.get(neighbor_id) {
                    let dist = self.metric.calculate(query, &neighbor.vector);
                    if dist < current_dist {
                        current = neighbor_id.clone();
                        current_dist = dist;
                        improved = true;
                    }
                }
            }

            if !improved {
                break;
            }
        }

        current
    }

    fn search_layer(
        &self,
        nodes: &HashMap<String, HnswNode>,
        query: &[f32],
        entry_id: &str,
        layer: usize,
        ef: usize,
    ) -> Vec<String> {
        use std::collections::{BinaryHeap, HashSet};
        use std::cmp::Ordering;

        #[derive(Clone)]
        struct Candidate {
            id: String,
            distance: f32,
        }

        impl PartialEq for Candidate {
            fn eq(&self, other: &Self) -> bool {
                self.distance == other.distance
            }
        }

        impl Eq for Candidate {}

        impl PartialOrd for Candidate {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                other.distance.partial_cmp(&self.distance) // Reverse for min-heap
            }
        }

        impl Ord for Candidate {
            fn cmp(&self, other: &Self) -> Ordering {
                other.distance.partial_cmp(&self.distance).unwrap()
            }
        }

        let mut visited: HashSet<String> = HashSet::new();
        let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new();
        let mut results: BinaryHeap<Candidate> = BinaryHeap::new();

        let entry_dist = self.metric.calculate(
            query,
            &nodes.get(entry_id).unwrap().vector,
        );

        candidates.push(Candidate {
            id: entry_id.to_string(),
            distance: entry_dist,
        });
        results.push(Candidate {
            id: entry_id.to_string(),
            distance: entry_dist,
        });
        visited.insert(entry_id.to_string());

        while let Some(current) = candidates.pop() {
            // Early termination
            if let Some(worst) = results.peek() {
                if current.distance > worst.distance && results.len() >= ef {
                    break;
                }
            }

            if let Some(node) = nodes.get(&current.id) {
                if layer < node.connections.len() {
                    for neighbor_id in &node.connections[layer] {
                        if visited.insert(neighbor_id.clone()) {
                            if let Some(neighbor) = nodes.get(neighbor_id) {
                                let dist = self.metric.calculate(query, &neighbor.vector);
                                candidates.push(Candidate {
                                    id: neighbor_id.clone(),
                                    distance: dist,
                                });
                                results.push(Candidate {
                                    id: neighbor_id.clone(),
                                    distance: dist,
                                });

                                if results.len() > ef {
                                    results.pop();
                                }
                            }
                        }
                    }
                }
            }
        }

        results.into_iter().map(|c| c.id).collect()
    }
}

impl Default for HnswIndex {
    fn default() -> Self {
        Self::new()
    }
}

// Distance functions

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot = dot_product(a, b);
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    
    1.0 - (dot / (norm_a * norm_b))
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_basic_operations() {
        let index = HnswIndex::new();
        
        // Insert vectors
        index.insert("a".to_string(), vec![1.0, 0.0, 0.0]).unwrap();
        index.insert("b".to_string(), vec![0.0, 1.0, 0.0]).unwrap();
        index.insert("c".to_string(), vec![0.0, 0.0, 1.0]).unwrap();
        
        assert_eq!(index.len(), 3);
        
        // Search
        let results = index.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "a");
        
        // Delete
        index.delete("a").unwrap();
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dist = cosine_distance(&a, &b);
        assert!((dist - 1.0).abs() < 0.001);
    }
}

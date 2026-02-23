use sqlx::{sqlite::SqlitePool, Row};
use anyhow::Result;
use uuid::Uuid;
use serde::Serialize;
use async_trait::async_trait;
#[cfg(feature = "knowledge")]
use neo4rs::{Graph, query};
use std::sync::Arc;
// use serde_json::Value; // Removed unused import
// use futures::StreamExt; // Removed unused import

#[derive(Serialize)]
struct D3Node {
    id: String,
    group: u32, // 1 for entity, 2 for concept etc.
}

#[derive(Serialize)]
struct D3Link {
    source: String,
    target: String,
    value: u32,
    label: String,
}

#[derive(Serialize)]
pub struct D3Graph {
    nodes: Vec<D3Node>,
    links: Vec<D3Link>,
}

#[async_trait]
pub trait KnowledgeGraph: Send + Sync {
    async fn add_entity(&self, name: &str, type_: &str) -> Result<String>;
    async fn add_relation(&self, source: &str, target: &str, relation: &str) -> Result<()>;
    async fn find_related(&self, entity_name: &str) -> Result<Vec<(String, String, String)>>;
    async fn export_d3_json(&self) -> Result<String>;
}

pub type SharedKnowledgeGraph = Arc<dyn KnowledgeGraph>;

pub struct SqliteKnowledgeGraph {
    pool: SqlitePool,
}

impl SqliteKnowledgeGraph {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        let schema = r#"
        CREATE TABLE IF NOT EXISTS entities (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            type TEXT NOT NULL,
            metadata TEXT
        );

        CREATE TABLE IF NOT EXISTS relations (
            id TEXT PRIMARY KEY,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation TEXT NOT NULL,
            metadata TEXT,
            FOREIGN KEY (source_id) REFERENCES entities(id),
            FOREIGN KEY (target_id) REFERENCES entities(id),
            UNIQUE(source_id, target_id, relation)
        );
        "#;
        
        sqlx::query(schema).execute(&pool).await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl KnowledgeGraph for SqliteKnowledgeGraph {
    async fn add_entity(&self, name: &str, type_: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT OR IGNORE INTO entities (id, name, type) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(type_)
            .execute(&self.pool)
            .await?;
            
        // Return existing ID if duplicate
        let row = sqlx::query("SELECT id FROM entities WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
            
        Ok(row.get("id"))
    }

    async fn add_relation(&self, source: &str, target: &str, relation: &str) -> Result<()> {
        let source_id = self.add_entity(source, "concept").await?;
        let target_id = self.add_entity(target, "concept").await?;
        
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT OR IGNORE INTO relations (id, source_id, target_id, relation) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(source_id)
            .bind(target_id)
            .bind(relation)
            .execute(&self.pool)
            .await?;
            
        Ok(())
    }

    async fn find_related(&self, entity_name: &str) -> Result<Vec<(String, String, String)>> {
        // Find outgoing relations: entity -> relation -> target
        let outgoing = sqlx::query(
            r#"
            SELECT r.relation, e.name as target_name, e.type as target_type
            FROM entities source
            JOIN relations r ON source.id = r.source_id
            JOIN entities e ON r.target_id = e.id
            WHERE source.name = ?
            "#
        )
        .bind(entity_name)
        .fetch_all(&self.pool)
        .await?;

        // Find incoming relations: source -> relation -> entity
        let incoming = sqlx::query(
            r#"
            SELECT r.relation, e.name as source_name, e.type as source_type
            FROM entities target
            JOIN relations r ON target.id = r.target_id
            JOIN entities e ON r.source_id = e.id
            WHERE target.name = ?
            "#
        )
        .bind(entity_name)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        
        for row in outgoing {
            results.push((
                "->".to_string(), 
                row.get::<String, _>("relation"), 
                row.get::<String, _>("target_name")
            ));
        }

        for row in incoming {
            results.push((
                "<-".to_string(), 
                row.get::<String, _>("relation"), 
                row.get::<String, _>("source_name")
            ));
        }

        Ok(results)
    }

    async fn export_d3_json(&self) -> Result<String> {
        let entities = sqlx::query("SELECT name, type FROM entities")
            .fetch_all(&self.pool)
            .await?;
            
        let relations = sqlx::query(
            r#"
            SELECT s.name as source, t.name as target, r.relation 
            FROM relations r
            JOIN entities s ON r.source_id = s.id
            JOIN entities t ON r.target_id = t.id
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut nodes = Vec::new();
        for row in entities {
            nodes.push(D3Node {
                id: row.get("name"),
                group: 1, // Default group
            });
        }
        
        let mut links = Vec::new();
        for row in relations {
            links.push(D3Link {
                source: row.get("source"),
                target: row.get("target"),
                value: 1,
                label: row.get("relation"),
            });
        }
        
        let graph = D3Graph { nodes, links };
        Ok(serde_json::to_string_pretty(&graph)?)
    }
}

#[cfg(feature = "knowledge")]
pub struct Neo4jKnowledgeGraph {
    graph: Arc<Graph>,
}

#[cfg(feature = "knowledge")]
impl Neo4jKnowledgeGraph {
    pub async fn new(uri: &str, user: &str, pass: &str) -> Result<Self> {
        let graph = Graph::new(uri, user, pass).await?;
        Ok(Self { graph: Arc::new(graph) })
    }
}

#[cfg(feature = "knowledge")]
#[async_trait]
impl KnowledgeGraph for Neo4jKnowledgeGraph {
    async fn add_entity(&self, name: &str, type_: &str) -> Result<String> {
        // MERGE (n:Entity {name: $name}) SET n.type = $type RETURN elementId(n)
        let q = query("MERGE (n:Entity {name: $name}) SET n.type = $type RETURN elementId(n) as id")
            .param("name", name)
            .param("type", type_);
            
        let mut stream = self.graph.execute(q).await?;
        if let Some(row) = stream.next().await? {
            // elementId is string in recent neo4j
            let id: String = row.get("id").unwrap_or_else(|_| "unknown".to_string());
            return Ok(id);
        }
        Ok("".to_string())
    }

    async fn add_relation(&self, source: &str, target: &str, relation: &str) -> Result<()> {
        let q = query("
            MERGE (s:Entity {name: $source})
            MERGE (t:Entity {name: $target})
            MERGE (s)-[r:RELATED {type: $relation}]->(t)
        ")
        .param("source", source)
        .param("target", target)
        .param("relation", relation);
        
        self.graph.run(q).await?;
        Ok(())
    }

    async fn find_related(&self, entity_name: &str) -> Result<Vec<(String, String, String)>> {
        let q = query("
            MATCH (n:Entity {name: $name})-[r]-(m:Entity)
            RETURN type(r) as rel_type, r.type as rel_name, startNode(r) = n as is_outgoing, m.name as other_name
        ")
        .param("name", entity_name);
        
        let mut stream = self.graph.execute(q).await?;
        let mut results = Vec::new();
        
        while let Some(row) = stream.next().await? {
            let rel_name: String = row.get("rel_name").unwrap_or_else(|_| "RELATED".to_string());
            let is_outgoing: bool = row.get("is_outgoing").unwrap_or(true);
            let other_name: String = row.get("other_name").unwrap_or_default();
            
            let dir = if is_outgoing { "->".to_string() } else { "<-".to_string() };
            results.push((dir, rel_name, other_name));
        }
        
        Ok(results)
    }

    async fn export_d3_json(&self) -> Result<String> {
        // Get all nodes
        let mut stream = self.graph.execute(query("MATCH (n:Entity) RETURN n.name as name")).await?;
        let mut nodes = Vec::new();
        while let Some(row) = stream.next().await? {
            let name: String = row.get("name").unwrap_or_default();
            nodes.push(D3Node { id: name, group: 1 });
        }
        
        // Get all links
        let mut stream = self.graph.execute(query("MATCH (s:Entity)-[r]->(t:Entity) RETURN s.name as source, t.name as target, r.type as relation")).await?;
        let mut links = Vec::new();
        while let Some(row) = stream.next().await? {
            let source: String = row.get("source").unwrap_or_default();
            let target: String = row.get("target").unwrap_or_default();
            let relation: String = row.get("relation").unwrap_or_default();
            
            links.push(D3Link {
                source,
                target,
                value: 1,
                label: relation,
            });
        }
        
        let graph = D3Graph { nodes, links };
        Ok(serde_json::to_string_pretty(&graph)?)
    }
}

use super::{CreateNodeRequest, NodeType};
use crate::ag::{AgType, Vertex};
use crate::utils::generate_props_clause;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CreateNodeResponse {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    graph_id: String,
    node_type: String,
    properties: HashMap<String, JsonValue>,
}

impl Node {
    async fn try_from(
        pool: &sqlx::PgPool,
        vertex: Vertex,
        graph_id: &str,
    ) -> Result<Self, serde_json::Error> {
        let node_type = NodeType::from_id(pool, graph_id, &vertex.label)
            .await
            .map_err(|e| {
                // Create a JSON error with a custom message
                serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "Error fetching node type for label '{}': {}",
                        &vertex.label, e
                    ),
                ))
            })?;

        let properties: HashMap<String, JsonValue> = serde_json::from_value(vertex.properties)?;
        let node = Node {
            graph_id: graph_id.to_string(),
            node_type: node_type.id,
            properties,
        };
        Ok(node)
    }

    fn from_request(
        request: CreateNodeRequest,
        graph_id: String,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            graph_id,
            node_type: request.node_type,
            properties: request.properties,
        })
    }

    pub async fn list(
        pool: &sqlx::PgPool,
        graph_id: &str,
        node_type: Option<&str>,
        page: Option<u32>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let page = page.unwrap_or(1);
        let page_size = 5;
        let offset = (page - 1) * page_size;

        let query = if node_type.is_some() {
            format!(
                "SELECT * FROM cypher('{}', $$ MATCH (v:{}) RETURN v ORDER BY v.name SKIP {} LIMIT {} $$) as (row agtype)",
                graph_id, node_type.unwrap(), offset, page_size
            )
        } else {
            format!(
                "SELECT * FROM cypher('{}', $$ MATCH (v) RETURN v ORDER BY v.name SKIP {} LIMIT {} $$) as (row agtype)",
                graph_id, offset, page_size
            )
        };

        let ag_rows = sqlx::query_as::<_, AgType>(&query)
            .fetch_all(&*pool)
            .await?;

        let vertices: Vec<Vertex> = ag_rows
            .iter()
            .map(|ag_row| Vertex::try_from(ag_row.clone()).unwrap())
            .collect();

        let node_futures = vertices
            .into_iter() // Use into_iter() to move values
            .map(|vertex| async move {
                Node::try_from(pool, vertex, graph_id)
                    .await
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))
            });

        let nodes = try_join_all(node_futures).await?;
        Ok(nodes)
    }

    pub async fn get_by_name(
        pool: &sqlx::PgPool,
        graph_id: &str,
        node_type: &str,
        name: &str,
    ) -> Result<Self, sqlx::Error> {
        let node_type = NodeType::from_id(pool, graph_id, node_type).await?;
        let escaped_name = name.replace("'", "''");
        let query = format!(
            "SELECT * FROM cypher('{}', $$ MATCH (n:{} {{name: '{}'}}) RETURN n $$) as (row agtype)",
            graph_id,
            &node_type.id,
            &escaped_name
        );

        let ag_row = sqlx::query_as::<_, AgType>(&query)
            .fetch_one(&*pool)
            .await?;

        let vertex: Vertex =
            Vertex::try_from(ag_row).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        let node = Node::try_from(pool, vertex, graph_id)
            .await
            .map_err(|e| sqlx::Error::Decode(Box::new(e)));
        node
    }

    pub async fn create(
        pool: &sqlx::PgPool,
        create_node_request: CreateNodeRequest,
        created_by: Uuid,
        graph_id: String,
    ) -> Result<(), sqlx::Error> {
        let mut node = Node::from_request(create_node_request, graph_id)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        // Add created_by and created_at to properties
        node.properties.insert(
            "created_by".to_string(),
            JsonValue::String(created_by.to_string()),
        );
        node.properties.insert(
            "created_at".to_string(),
            JsonValue::String(chrono::Utc::now().to_rfc3339()),
        );

        let props_clause = generate_props_clause(&node.properties);
        let query = format!(
            "SELECT * FROM cypher('{}', $$ CREATE (n:{} {}) RETURN n $$) as (row agtype)",
            &node.graph_id, &node.node_type, &props_clause
        );

        sqlx::query(&query).fetch_one(&*pool).await?;
        Ok(())
    }
}

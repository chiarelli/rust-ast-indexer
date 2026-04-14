use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpTool {
    pub fn list_languages() -> Self {
        Self {
            name: "list_languages".to_string(),
            description: "Returns the list of supported languages".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    pub fn index_path() -> Self {
        let mut properties = Map::new();
        properties.insert(
            "path".to_string(),
            json!({ "type": "string", "description": "Path to index" }),
        );
        properties.insert(
            "options".to_string(),
            json!({
                "type": "object",
                "description": "Indexing options",
                "properties": {
                    "max_concurrency": json!({"type": "integer"}),
                    "include": json!({"type": "array", "items": json!({"type": "string"})}),
                    "exclude": json!({"type": "array", "items": json!({"type": "string"})})
                }
            }),
        );
        Self {
            name: "index_path".to_string(),
            description: "Index a directory path".to_string(),
            input_schema: json!({ "type": "object", "properties": properties }),
        }
    }

    pub fn stop() -> Self {
        Self {
            name: "stop".to_string(),
            description: "Stop the current indexing job".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    pub fn status() -> Self {
        Self {
            name: "status".to_string(),
            description: "Get the current status of the indexer".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        }
    }

    pub fn to_json_schema(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema
        })
    }
}

pub struct McpTools;

impl McpTools {
    pub fn list() -> Vec<McpTool> {
        vec![
            McpTool::list_languages(),
            McpTool::index_path(),
            McpTool::stop(),
            McpTool::status(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_schema_define_list_languages() {
        let tool = McpTool::list_languages();
        assert_eq!(tool.name, "list_languages");
        assert!(!tool.description.is_empty());
        assert!(tool.input_schema.is_object());
    }

    #[test]
    fn mcp_tool_schema_define_index_path() {
        let tool = McpTool::index_path();
        assert_eq!(tool.name, "index_path");
        assert!(tool.input_schema.get("properties").is_some());
    }

    #[test]
    fn mcp_tool_schema_define_stop() {
        let tool = McpTool::stop();
        assert_eq!(tool.name, "stop");
    }

    #[test]
    fn mcp_tool_schema_define_status() {
        let tool = McpTool::status();
        assert_eq!(tool.name, "status");
    }

    #[test]
    fn mcp_tools_list_returns_all() {
        let tools = McpTools::list();
        assert_eq!(tools.len(), 4);
        let names: Vec<_> = tools.iter().map(|t| t.name.clone()).collect();
        assert!(names.contains(&"list_languages".to_string()));
        assert!(names.contains(&"index_path".to_string()));
        assert!(names.contains(&"stop".to_string()));
        assert!(names.contains(&"status".to_string()));
    }

    #[test]
    fn mcp_tool_to_json_schema() {
        let tool = McpTool::list_languages();
        let schema = tool.to_json_schema();
        assert!(schema.get("name").is_some());
        assert!(schema.get("inputSchema").is_some());
    }
}

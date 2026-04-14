use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }

    pub fn to_value(&self) -> Value {
        if let Some(err) = &self.error {
            json!({
                "jsonrpc": self.jsonrpc,
                "id": self.id,
                "error": {
                    "code": err.code,
                    "message": err.message
                }
            })
        } else {
            json!({
                "jsonrpc": self.jsonrpc,
                "id": self.id,
                "result": self.result
            })
        }
    }
}

pub struct McpAdapter {
    handlers: Arc<Mutex<HashMap<String, fn(Value) -> Result<Value, String>>>>,
}

impl McpAdapter {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_handler(&self, method: &str, handler: fn(Value) -> Result<Value, String>) {
        if let Ok(mut handlers) = self.handlers.lock() {
            handlers.insert(method.to_string(), handler);
        }
    }

    pub fn parse_request(&self, raw: &str) -> Result<JsonRpcRequest, String> {
        let value: Value =
            serde_json::from_str(raw).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let obj = value
            .as_object()
            .ok_or("Request must be a JSON object")?
            .clone();

        let jsonrpc = obj
            .get("jsonrpc")
            .and_then(|v| v.as_str())
            .unwrap_or("2.0")
            .to_string();

        if jsonrpc != "2.0" {
            return Err(format!("Unsupported jsonrpc version: {}", jsonrpc));
        }

        let method = obj
            .get("method")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'method' field")?
            .to_string();

        let id = obj.get("id").cloned();
        let params = obj.get("params").cloned();

        Ok(JsonRpcRequest {
            jsonrpc,
            id,
            method,
            params,
        })
    }

    pub fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let handler = self
            .handlers
            .lock()
            .ok()
            .and_then(|h| h.get(&request.method).cloned());

        match handler {
            Some(handler_fn) => {
                let params = request.params.unwrap_or(json!({}));
                match handler_fn(params) {
                    Ok(result) => JsonRpcResponse::success(request.id, result),
                    Err(e) => JsonRpcResponse::error(request.id, -32603, &e),
                }
            }
            None => JsonRpcResponse::error(request.id, -32601, "Method not found"),
        }
    }
}

impl Default for McpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_response_success() {
        let resp = JsonRpcResponse::success(Some(json!(1)), json!({"languages": ["rust"]}));
        let value = resp.to_value();
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 1);
        assert!(value["result"].is_object());
    }

    #[test]
    fn json_rpc_response_error() {
        let resp = JsonRpcResponse::error(Some(json!(1)), -32600, "Invalid Request");
        let value = resp.to_value();
        assert_eq!(value["error"]["code"], -32600);
        assert_eq!(value["error"]["message"], "Invalid Request");
    }

    #[test]
    fn mcp_adapter_parse_valid_request() {
        let adapter = McpAdapter::new();
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"list_languages","params":{}}"#;
        let req = adapter.parse_request(raw).unwrap();
        assert_eq!(req.method, "list_languages");
        assert_eq!(req.id, Some(json!(1)));
    }

    #[test]
    fn mcp_adapter_parse_missing_method() {
        let adapter = McpAdapter::new();
        let raw = r#"{"jsonrpc":"2.0","id":1,"params":{}}"#;
        let result = adapter.parse_request(raw);
        assert!(result.is_err());
    }

    #[test]
    fn mcp_adapter_handle_unknown_method() {
        let adapter = McpAdapter::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "unknown".to_string(),
            params: None,
        };
        let resp = adapter.handle_request(req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn mcp_adapter_register_and_invoke_handler() {
        let adapter = McpAdapter::new();
        adapter.register_handler("test_method", |params| Ok(json!({ "echo": params })));

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "test_method".to_string(),
            params: Some(json!({"value": 42})),
        };
        let resp = adapter.handle_request(req);
        assert!(resp.error.is_none());
        assert_eq!(resp.result.as_ref().unwrap()["echo"]["value"], 42);
    }
}

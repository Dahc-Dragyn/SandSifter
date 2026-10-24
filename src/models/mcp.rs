use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool definition conforming to Model Context Protocol (MCP 2024-11-05).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Content block inside an MCP tool call result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl McpContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: text.into(),
        }
    }
}

/// Standard CallToolResult returned from MCP server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<McpContentBlock>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
}

impl McpToolResult {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContentBlock::text(text)],
            is_error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![McpContentBlock::text(message)],
            is_error: true,
        }
    }
}

/// MCP JSON-RPC 2.0 Request Envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpJsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// MCP JSON-RPC 2.0 Response Envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpJsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpJsonRpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpJsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpJsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

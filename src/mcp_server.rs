// src/mcp_server.rs

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::core::distill_json;
use crate::error::DistillError;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DistillRequest {
    /// The JSON data as a string
    pub json_string: String,
    /// Use strict type checking (default: true)
    #[serde(default = "default_strict_typing")]
    pub strict_typing: bool,
    /// Minimum repeat count for summarization (default: 2)
    #[serde(default = "default_repeat_threshold")]
    pub repeat_threshold: usize,
    /// Position-dependent mode: show examples at each nesting level (default: true)
    /// When false, shows examples only at shallowest depth (more concise)
    #[serde(default = "default_position_dependent")]
    pub position_dependent: bool,
}

fn default_strict_typing() -> bool {
    true
}

fn default_repeat_threshold() -> usize {
    2
}

fn default_position_dependent() -> bool {
    false  // Match Python's default (POSITION_DEPENDENT = False)
}

#[derive(Clone)]
pub struct JsonDistillerServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl JsonDistillerServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Reverse engineer arbitrary JSON structures for LLM analysis without context overflow. Dramatically increases information density by identifying unique structural patterns and summarizing repetition. Shows one representative example per structure type plus summaries - perfect when you care about structure and patterns, not individual content. Achieves 99%+ compression on repetitive data while preserving full structural information.")]
    async fn distill_json_content(
        &self,
        Parameters(params): Parameters<DistillRequest>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!(
            "Distilling JSON with strict_typing={}, repeat_threshold={}",
            params.strict_typing,
            params.repeat_threshold
        );

        // Parse the input JSON string
        let input_value: serde_json::Value = serde_json::from_str(&params.json_string)
            .map_err(|e| McpError {
                code: ErrorCode(-32602), // Invalid params
                message: format!("Failed to parse JSON: {}", e).into(),
                data: None,
            })?;

        // Perform distillation
        let distilled_value = distill_json(
            input_value,
            params.strict_typing,
            params.repeat_threshold,
            params.position_dependent,
        )
        .map_err(|e: DistillError| McpError {
            code: ErrorCode(-32603), // Internal error
            message: format!("Distillation failed: {}", e).into(),
            data: None,
        })?;

        // Convert result to pretty JSON string
        let result_string = serde_json::to_string_pretty(&distilled_value).map_err(|e| {
            McpError {
                code: ErrorCode(-32603), // Internal error
                message: format!("Failed to serialize result: {}", e).into(),
                data: None,
            }
        })?;

        Ok(CallToolResult::success(vec![Content::text(
            result_string,
        )]))
    }
}

#[tool_handler]
impl ServerHandler for JsonDistillerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Analyze and reverse engineer arbitrary JSON structures for LLMs. This tool dramatically increases information \
                density (99%+ compression) by identifying unique structural patterns in large JSON payloads. Perfect for \
                understanding API responses, datasets, and complex JSON without overwhelming context windows. Preserves complete \
                structural information while removing repetitive content."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        tracing::info!("Client initialized MCP server");
        Ok(self.get_info())
    }
}

/// Start the MCP server with stdio transport
pub async fn start_mcp() -> anyhow::Result<()> {
    tracing::info!("Starting JSON Distiller MCP server...");

    let server = JsonDistillerServer::new();

    // Use stdio transport (stdin/stdout)
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("Serving error: {:?}", e);
        })?;

    tracing::info!("MCP server running on stdio transport");

    // Wait for shutdown
    service.waiting().await?;

    tracing::info!("MCP server shutdown");
    Ok(())
}

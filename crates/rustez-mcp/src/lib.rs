//! `rustez-mcp`: MCP client stub (stdio + SSE land after onboarding).

/// MCP server ref (minimal).
#[derive(Debug, Clone)]
pub struct EzMcpServer {
    pub name: String,
    pub command: String,
}

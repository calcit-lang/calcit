use argh::FromArgs;
use axum::{
  Router,
  extract::{Json, State},
  response::Json as ResponseJson,
  routing::{get, post},
};
use calcit::snapshot;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

// 导入 MCP 模块
use calcit::mcp::*;

/// MCP server for Calcit
#[derive(FromArgs)]
struct Args {
  /// compact.cirru file path
  #[argh(option, short = 'f', default = "String::from(\"compact.cirru\")")]
  file: String,

  /// port to listen on
  #[argh(option, short = 'p', default = "7200")]
  port: u16,
}

// 使用mcp模块中的AppState
use calcit::mcp::AppState;

// Standard MCP JSON-RPC 2.0 endpoint
async fn mcp_jsonrpc(
  State(data): State<Arc<AppState>>,
  Json(req): Json<calcit::mcp::JsonRpcRequest>,
) -> ResponseJson<serde_json::Value> {
  calcit::mcp::handle_jsonrpc_axum(data, req).await
}

// Legacy endpoint for backward compatibility
async fn discover() -> ResponseJson<serde_json::Value> {
  calcit::mcp::legacy_discover_axum().await
}

async fn mcp_config(State(data): State<Arc<AppState>>) -> ResponseJson<serde_json::Value> {
  println!("handling /mcp/");
  let config = serde_json::json!({
    "mcpServers": {
      "calcit": {
        "command": "http",
        "args": {
          "url": format!("http://localhost:{}", data.port),
          "headers": {
            "Content-Type": "application/json"
          }
        },
        "tools": get_mcp_tools()
      }
    },
    "gemini_cli_command": format!("gemini mcp add --transport http calcit http://localhost:{}", data.port),
    "server_info": {
      "name": "Calcit MCP Server",
      "version": "1.0.0",
      "description": "MCP server for Calcit language code editing and analysis",
      "current_module": &data.current_module_name,
      "compact_file": &data.compact_cirru_path
    }
  });

  ResponseJson(config)
}

// Legacy endpoint for backward compatibility
async fn execute(State(data): State<Arc<AppState>>, Json(req): Json<McpRequest>) -> ResponseJson<serde_json::Value> {
  calcit::mcp::legacy_execute_axum(data, req).await
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
  let args: Args = argh::from_env();

  // 加载当前模块名称
  let current_module_name = {
    let content = match std::fs::read_to_string(&args.file) {
      Ok(c) => c,
      Err(e) => {
        eprintln!("Failed to read compact.cirru: {e}");
        std::process::exit(1);
      }
    };

    let edn_data = match cirru_edn::parse(&content) {
      Ok(d) => d,
      Err(e) => {
        eprintln!("Failed to parse compact.cirru as EDN: {e}");
        std::process::exit(1);
      }
    };

    match snapshot::load_snapshot_data(&edn_data, &args.file) {
      Ok(snapshot) => snapshot.package,
      Err(e) => {
        eprintln!("Failed to load snapshot: {e}");
        std::process::exit(1);
      }
    }
  };

  let app_state = Arc::new(AppState {
    compact_cirru_path: args.file.clone(),
    current_module_name,
    port: args.port,
    module_cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
  });

  println!("Starting MCP server on port {}", args.port);
  println!("Loading file: {}", args.file);

  let app = Router::new()
    .route("/mcp_jsonrpc", post(mcp_jsonrpc)) // Standard MCP JSON-RPC 2.0 endpoint
    .route("/mcp/discover", get(discover)) // Legacy endpoint
    .route("/mcp/", get(mcp_config)) // Legacy endpoint
    .route("/mcp/execute", post(execute)) // Legacy endpoint
    .layer(CorsLayer::permissive())
    .with_state(app_state);

  let listener = TcpListener::bind(("127.0.0.1", args.port)).await?;
  axum::serve(listener, app).await
}

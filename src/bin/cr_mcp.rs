use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use argh::FromArgs;
use calcit::snapshot;
use std::collections::HashMap;

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
#[post("/mcp_jsonrpc")]
async fn mcp_jsonrpc(data: web::Data<AppState>, req: web::Json<calcit::mcp::JsonRpcRequest>) -> impl Responder {
  calcit::mcp::handle_jsonrpc(data, req).await
}

// Legacy endpoint for backward compatibility
#[get("/mcp/discover")]
async fn discover() -> impl Responder {
  calcit::mcp::legacy_discover().await
}

#[get("/mcp/")]
async fn mcp_config(data: web::Data<AppState>) -> impl Responder {
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

  HttpResponse::Ok().json(config)
}

// Legacy endpoint for backward compatibility
#[post("/mcp/execute")]
async fn execute(data: web::Data<AppState>, req: web::Json<McpRequest>) -> impl Responder {
  calcit::mcp::legacy_execute(data, req).await
}

#[actix_web::main]
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

  let app_state = AppState {
    compact_cirru_path: args.file.clone(),
    current_module_name,
    port: args.port,
    module_cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
  };

  println!("Starting MCP server on port {}", args.port);
  println!("Loading file: {}", args.file);

  HttpServer::new(move || {
      App::new()
        .app_data(web::Data::new(app_state.clone()))
        .service(mcp_jsonrpc)  // Standard MCP JSON-RPC 2.0 endpoint
        .service(discover)     // Legacy endpoint
        .service(mcp_config)   // Legacy endpoint
        .service(execute)      // Legacy endpoint
    })
  .bind(("127.0.0.1", args.port))?
  .run()
  .await
}

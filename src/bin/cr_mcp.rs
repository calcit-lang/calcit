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

#[get("/mcp/discover")]
async fn discover() -> impl Responder {
  println!("handling /mcp/discover");
  let tools = get_mcp_tools();
  HttpResponse::Ok().json(serde_json::json!({
    "tools": tools
  }))
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

#[post("/mcp/execute")]
async fn execute(data: web::Data<AppState>, req: web::Json<McpRequest>) -> impl Responder {
  println!("handling /mcp/execute with tool: {}", req.tool_name);
  match req.tool_name.as_str() {
    // 读取操作
    "list_definitions" => list_definitions(&data, req.into_inner()),
    "list_namespaces" => list_namespaces(&data, req.into_inner()),
    "read_namespace" => read_namespace(&data, req.into_inner()),
    "read_definition" => read_definition(&data, req.into_inner()),

    // 命名空间操作
    "add_namespace" => add_namespace(&data, req.into_inner()),
    "delete_namespace" => delete_namespace(&data, req.into_inner()),
    "update_namespace_imports" => update_namespace_imports(&data, req.into_inner()),

    // 定义操作
    "add_definition" => add_definition(&data, req.into_inner()),
    "delete_definition" => delete_definition(&data, req.into_inner()),
    "update_definition" => update_definition(&data, req.into_inner()),

    // 模块管理
    "list_modules" => list_modules(&data, req.into_inner()),
    "read_module" => read_module(&data, req.into_inner()),
    "add_module_dependency" => add_module_dependency(&data, req.into_inner()),
    "remove_module_dependency" => remove_module_dependency(&data, req.into_inner()),
    "clear_module_cache" => clear_module_cache(&data, req.into_inner()),

    // Cirru 转换工具
    "parse_to_json" => parse_to_json(&data, req.into_inner()),
    "format_from_json" => format_from_json(&data, req.into_inner()),

    _ => HttpResponse::BadRequest().body(format!("Unknown tool: {}", req.tool_name)),
  }
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
      .service(discover)
      .service(mcp_config)
      .service(execute)
  })
  .bind(("127.0.0.1", args.port))?
  .run()
  .await
}

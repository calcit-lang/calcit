use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use argh::FromArgs;
use calcit::snapshot;
use std::collections::HashMap;

// 导入 MCP 模块
mod mcp;
use mcp::cirru_handlers::*;
use mcp::definition_handlers::*;
use mcp::module_handlers::*;
use mcp::namespace_handlers::*;
use mcp::read_handlers::*;
use mcp::tools::*;

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

#[derive(Clone)]
struct AppState {
  compact_cirru_path: String,
  current_module_name: String,
  module_cache: std::sync::Arc<std::sync::RwLock<HashMap<String, snapshot::Snapshot>>>,
}

#[get("/mcp/discover")]
async fn discover() -> impl Responder {
  let tools = get_mcp_tools();
  HttpResponse::Ok().json(serde_json::json!({
    "tools": tools
  }))
}

#[post("/mcp/execute")]
async fn execute(data: web::Data<AppState>, req: web::Json<McpRequest>) -> impl Responder {
  match req.tool_name.as_str() {
    // 读取操作
    "list_definitions" => list_definitions(&data, req.into_inner()),
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
    "parseToJson" => parse_to_json(&data, req.into_inner()),
    "formatFromJson" => format_from_json(&data, req.into_inner()),

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
    module_cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
  };

  println!("Starting MCP server on port {}", args.port);
  println!("Loading file: {}", args.file);

  HttpServer::new(move || {
    App::new()
      .app_data(web::Data::new(app_state.clone()))
      .service(discover)
      .service(execute)
  })
  .bind(("127.0.0.1", args.port))?
  .run()
  .await
}

use anyhow::{Context, Result, anyhow, bail};
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Config, Engine, Store};

#[allow(unused_imports)]
mod generated_http {
  include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/generated-component/rust/wasmtime_http_adapter.rs"
  ));
}

fn request(url: String, max_response_bytes: u64) -> Val {
  Val::Record(vec![
    ("body".to_owned(), Val::Variant("empty".to_owned(), None)),
    ("headers".to_owned(), Val::List(vec![])),
    ("max-response-bytes".to_owned(), Val::U64(max_response_bytes)),
    ("method".to_owned(), Val::Variant("get".to_owned(), None)),
    ("url".to_owned(), Val::String(url)),
  ])
}

fn print_response(value: &Val) -> Result<()> {
  let Val::Result(Ok(Some(response))) = value else {
    if let Val::Result(Err(Some(error))) = value {
      bail!("HTTP request returned a typed error: {error:?}");
    }
    bail!("HTTP request returned an unexpected value: {value:?}");
  };
  let Val::Record(fields) = response.as_ref() else {
    bail!("HTTP response is not a record: {response:?}");
  };
  let status = fields.iter().find_map(|(name, value)| (name == "status").then_some(value));
  let body = fields.iter().find_map(|(name, value)| (name == "body").then_some(value));
  println!("# Calcit WASI HTTP response");
  println!();
  println!("- status: `{status:?}`");
  println!("- body: `{body:?}`");
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let mut args = std::env::args().skip(1);
  let allowed_origin = args
    .next()
    .context("usage: starter-host <allowed-origin> <url> [max-response-bytes]")?;
  let url = args
    .next()
    .context("usage: starter-host <allowed-origin> <url> [max-response-bytes]")?;
  let max_response_bytes = args
    .next()
    .map(|value| value.parse::<u64>().context("max-response-bytes must be an unsigned integer"))
    .transpose()?
    .unwrap_or(65_536);
  if args.next().is_some() {
    bail!("usage: starter-host <allowed-origin> <url> [max-response-bytes]");
  }

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_more_async_builtins(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).map_err(|error| anyhow!(error.to_string()))?;
  let component = Component::from_file(
    &engine,
    concat!(env!("CARGO_MANIFEST_DIR"), "/generated-component/component/component.wasm"),
  )
  .map_err(|error| anyhow!(error.to_string()))?;
  let mut linker = Linker::new(&engine);
  let http = generated_http::WasiHttpConfig::default()
    .allow_origin(&allowed_origin)
    .map_err(|error| anyhow!("allowed-origin must be an exact scheme://authority value: {error}"))?;
  generated_http::add_to_linker(&mut linker, http).map_err(|error| anyhow!(error.to_string()))?;
  let mut store = Store::new(&engine, ());
  let instance = linker
    .instantiate_async(&mut store, &component)
    .await
    .map_err(|error| anyhow!(error.to_string()))?;
  let function = instance
    .get_func(&mut store, "call-host-http-request")
    .context("Component does not export call-host-http-request")?;
  let mut results = [Val::Bool(false)];
  function
    .call_async(&mut store, &[request(url, max_response_bytes)], &mut results)
    .await
    .map_err(|error| anyhow!(error.to_string()))?;
  print_response(&results[0])
}

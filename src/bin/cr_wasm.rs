#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
mod injection;

use std::str::FromStr;

use argh::FromArgs;
use calcit::builtins;
use calcit::codegen::emit_wasm::WasmTarget;
#[cfg(not(target_arch = "wasm32"))]
use calcit::runner;
use calcit::wasm_cli::{self, WasmCliOptions};

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// Internal compatibility wrapper for WASM codegen.
struct WasmArgs {
  /// emit path for generated artifacts, defaults to "js-out/"
  #[argh(option, default = "String::from(\"js-out/\")")]
  emit_path: String,
  /// specify `init_fn` which is main function
  #[argh(option)]
  init_fn: Option<String>,
  /// specify `reload_fn` which is called after hot reload
  #[argh(option)]
  reload_fn: Option<String>,
  /// specify with config entry
  #[argh(option)]
  entry: Option<String>,
  /// check-only mode: validate without codegen
  #[argh(switch)]
  check_only: bool,
  /// output target: core (browser/embedded, default) or wasi (command module)
  #[argh(option, default = "String::from(\"core\")")]
  target: String,
  /// print version only
  #[argh(switch, short = 'v')]
  version: bool,
  /// input source file, defaults to "calcit.cirru"; retired compact.cirru inputs receive migration guidance
  #[argh(positional, default = "String::from(calcit::DEFAULT_SNAPSHOT_FILE)")]
  input: String,
}

fn main() -> Result<(), String> {
  let cli_args: WasmArgs = argh::from_env();

  if cli_args.version {
    println!("{}", calcit::cli_args::CALCIT_VERSION);
    return Ok(());
  }
  let target = WasmTarget::from_str(&cli_args.target)?;

  builtins::effects::init_effects_states();

  #[cfg(not(target_arch = "wasm32"))]
  injection::inject_platform_apis();

  wasm_cli::run(
    &WasmCliOptions {
      input: cli_args.input,
      emit_path: cli_args.emit_path,
      init_fn: cli_args.init_fn,
      reload_fn: cli_args.reload_fn,
      entry: cli_args.entry,
      check_only: cli_args.check_only,
    },
    target,
  )
}

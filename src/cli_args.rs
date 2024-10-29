use argh::FromArgs;

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// Top-level command.
pub struct ToplevelCalcit {
  #[argh(subcommand)]
  pub subcommand: Option<CalcitCommand>,
  /// skip watching mode, just run once
  #[argh(switch, short = '1')]
  pub once: bool,
  /// disable stack trace for errors
  #[argh(switch)]
  pub disable_stack: bool,
  /// skip arity check in js codegen
  #[argh(switch)]
  pub skip_arity_check: bool,
  /// entry file path, defaults to "js-out/"
  #[argh(option, default = "String::from(\"js-out/\")")]
  pub emit_path: String,
  /// specify `init_fn` which is main function
  #[argh(option)]
  pub init_fn: Option<String>,
  /// specify `reload_fn` which is called after hot reload
  #[argh(option)]
  pub reload_fn: Option<String>,
  /// specify with config entry
  #[argh(option)]
  pub entry: Option<String>,
  #[argh(switch)]
  /// force reloading libs data during code reload
  pub reload_libs: bool,
  #[argh(option)]
  /// specify a path to watch assets changes
  pub watch_dir: Option<String>,
  /// input source file, defaults to "compact.cirru"
  #[argh(positional, default = "String::from(\"compact.cirru\")")]
  pub input: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum CalcitCommand {
  /// emit JavaScript rather than interpreting
  EmitJs(EmitJsCommand),
  /// emit Cirru EDN representation of program to program-ir.cirru
  EmitIr(EmitIrCommand),
  /// evaluate snippet
  Eval(EvalCommand),
}

/// emit JavaScript rather than interpreting
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "js")]
pub struct EmitJsCommand {
  /// skip watching mode, just run once
  #[argh(switch, short = '1')]
  pub once: bool,
}

/// emit Cirru EDN representation of program to program-ir.cirru
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ir")]
pub struct EmitIrCommand {
  /// skip watching mode, just run once
  #[argh(switch, short = '1')]
  pub once: bool,
}

/// run program
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "eval")]
pub struct EvalCommand {
  /// evaluate a snippet
  #[argh(positional)]
  pub snippet: String,
  /// entry file path
  #[argh(option)]
  pub dep: Vec<String>,
}

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
  /// print version only
  #[argh(switch)]
  pub version: bool,
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
  /// check examples in namespace
  CheckExamples(CheckExamplesCommand),
  /// analyze call tree structure from entry point
  CallTree(CallTreeCommand),
  /// query project information (namespaces, definitions, configs)
  Query(QueryCommand),
  /// documentation tools (API docs, guidebook)
  Docs(DocsCommand),
  /// Cirru syntax tools (parse, format)
  Cirru(CirruCommand),
  /// fetch available Calcit libraries from registry
  Libs(LibsCommand),
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

/// check examples in namespace
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "check-examples")]
pub struct CheckExamplesCommand {
  /// target namespace to check examples
  #[argh(option)]
  pub ns: String,
}

/// analyze call tree structure from entry point
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "call-tree")]
pub struct CallTreeCommand {
  /// directly specify root definition to analyze (format: ns/def)
  #[argh(option)]
  pub root: Option<String>,
  /// only show definitions whose namespace starts with this prefix
  #[argh(option)]
  pub ns_prefix: Option<String>,
  /// include core/library calls in the output
  #[argh(switch)]
  pub include_core: bool,
  /// maximum depth to traverse (0 = unlimited)
  #[argh(option, default = "0")]
  pub max_depth: usize,
  /// show unused definitions for the selected entry
  #[argh(switch)]
  pub show_unused: bool,
  /// output format: "text" (default, LLM-friendly) or "json"
  #[argh(option, default = "String::from(\"text\")")]
  pub format: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query subcommand - project information queries
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "query")]
/// query project information (namespaces, definitions, configs)
pub struct QueryCommand {
  #[argh(subcommand)]
  pub subcommand: QuerySubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum QuerySubcommand {
  /// list all namespaces in the project
  LsNs(QueryLsNsCommand),
  /// list definitions in a namespace
  LsDefs(QueryLsDefsCommand),
  /// read namespace information
  ReadNs(QueryReadNsCommand),
  /// get package name of the project
  PkgName(QueryPkgNameCommand),
  /// read project configs
  Configs(QueryConfigsCommand),
  /// read .calcit-error.cirru file
  Error(QueryErrorCommand),
  /// list modules in the project
  LsModules(QueryLsModulesCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ls-ns")]
/// list all namespaces in the project
pub struct QueryLsNsCommand {
  /// include dependency namespaces
  #[argh(switch)]
  pub deps: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ls-defs")]
/// list definitions in a namespace
pub struct QueryLsDefsCommand {
  /// namespace to query
  #[argh(positional)]
  pub namespace: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "read-ns")]
/// read namespace information (imports, definitions preview)
pub struct QueryReadNsCommand {
  /// namespace to read
  #[argh(positional)]
  pub namespace: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "pkg-name")]
/// get package name of the project
pub struct QueryPkgNameCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "configs")]
/// read project configs (init_fn, reload_fn, version)
pub struct QueryConfigsCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "error")]
/// read .calcit-error.cirru file for error stack traces
pub struct QueryErrorCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ls-modules")]
/// list modules in the project
pub struct QueryLsModulesCommand {}

// ═══════════════════════════════════════════════════════════════════════════════
// Docs subcommand - documentation tools
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "docs")]
/// documentation tools (API docs, guidebook)
pub struct DocsCommand {
  #[argh(subcommand)]
  pub subcommand: DocsSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum DocsSubcommand {
  /// query API documentation
  Api(DocsApiCommand),
  /// query guidebook/reference documentation
  Ref(DocsRefCommand),
  /// list all API documentation topics
  ListApi(DocsListApiCommand),
  /// list all guidebook documentation topics
  ListGuide(DocsListGuideCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "api")]
/// query API documentation
pub struct DocsApiCommand {
  /// query type: "all", "tag", "keyword"
  #[argh(option, short = 't', default = "String::from(\"keyword\")")]
  pub query_type: String,
  /// query value (tag name or keyword to search)
  #[argh(positional)]
  pub query: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ref")]
/// query guidebook/reference documentation
pub struct DocsRefCommand {
  /// query type: "all", "filename", "keyword"
  #[argh(option, short = 't', default = "String::from(\"keyword\")")]
  pub query_type: String,
  /// query value (filename or keyword to search)
  #[argh(positional)]
  pub query: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "list-api")]
/// list all API documentation topics
pub struct DocsListApiCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "list-guide")]
/// list all guidebook documentation topics
pub struct DocsListGuideCommand {}

// ═══════════════════════════════════════════════════════════════════════════════
// Cirru subcommand - syntax tools
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "cirru")]
/// Cirru syntax tools (parse, format, edn)
pub struct CirruCommand {
  #[argh(subcommand)]
  pub subcommand: CirruSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum CirruSubcommand {
  /// parse Cirru code to JSON
  Parse(CirruParseCommand),
  /// format JSON to Cirru code
  Format(CirruFormatCommand),
  /// parse Cirru EDN to JSON
  ParseEdn(CirruParseEdnCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "parse")]
/// parse Cirru code to JSON
pub struct CirruParseCommand {
  /// cirru code to parse
  #[argh(positional)]
  pub code: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "format")]
/// format JSON to Cirru code
pub struct CirruFormatCommand {
  /// JSON data to format (as string)
  #[argh(positional)]
  pub json: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "parse-edn")]
/// parse Cirru EDN to JSON
pub struct CirruParseEdnCommand {
  /// cirru EDN to parse
  #[argh(positional)]
  pub edn: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Libs subcommand - library registry
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "libs")]
/// fetch available Calcit libraries from registry
pub struct LibsCommand {
  #[argh(subcommand)]
  pub subcommand: Option<LibsSubcommand>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum LibsSubcommand {
  /// show README of a library
  Readme(LibsReadmeCommand),
  /// search libraries by keyword
  Search(LibsSearchCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "readme")]
/// show README of a library from its GitHub repository
pub struct LibsReadmeCommand {
  /// package name to look up
  #[argh(positional)]
  pub package: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// search libraries by keyword in name or description
pub struct LibsSearchCommand {
  /// keyword to search
  #[argh(positional)]
  pub keyword: String,
}

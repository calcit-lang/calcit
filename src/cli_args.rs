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
  /// check-only mode: validate without execution or codegen
  #[argh(switch)]
  pub check_only: bool,
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
  /// analyze code structure (call-tree, count-calls, check-examples)
  Analyze(AnalyzeCommand),
  /// query project information (namespaces, definitions, configs)
  Query(QueryCommand),
  /// documentation tools (API docs, guidebook)
  Docs(DocsCommand),
  /// Cirru syntax tools (parse, format)
  Cirru(CirruCommand),
  /// fetch available Calcit libraries from registry
  Libs(LibsCommand),
  /// edit project code (definitions, namespaces, modules, configs)
  Edit(EditCommand),
}

/// emit JavaScript rather than interpreting
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "js")]
pub struct EmitJsCommand {
  /// skip watching mode, just run once
  #[argh(switch, short = '1')]
  pub once: bool,
  /// check-only mode for JS emit
  #[argh(switch)]
  pub check_only: bool,
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

// ═══════════════════════════════════════════════════════════════════════════════
// Analyze subcommand - code structure analysis
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "analyze")]
/// analyze code structure (call-tree, count-calls, check-examples)
pub struct AnalyzeCommand {
  #[argh(subcommand)]
  pub subcommand: AnalyzeSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum AnalyzeSubcommand {
  /// analyze call tree structure from entry point
  CallTree(CallTreeCommand),
  /// count call occurrences from entry point
  CountCalls(CountCallsCommand),
  /// check examples in namespace
  CheckExamples(CheckExamplesCommand),
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

/// count call occurrences from entry point
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "count-calls")]
pub struct CountCallsCommand {
  /// directly specify root definition to analyze (format: ns/def)
  #[argh(option)]
  pub root: Option<String>,
  /// only show definitions whose namespace starts with this prefix
  #[argh(option)]
  pub ns_prefix: Option<String>,
  /// include core/library calls in the count
  #[argh(switch)]
  pub include_core: bool,
  /// output format: "text" (default) or "json"
  #[argh(option, default = "String::from(\"text\")")]
  pub format: String,
  /// sort by: "count" (default, descending) or "name"
  #[argh(option, default = "String::from(\"count\")")]
  pub sort: String,
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
  /// list namespaces (or show ns details if namespace provided)
  Ns(QueryNsCommand),
  /// list definitions in a namespace
  Defs(QueryDefsCommand),
  /// get package name
  Pkg(QueryPkgCommand),
  /// read project configs
  Config(QueryConfigCommand),
  /// read .calcit-error.cirru file
  Error(QueryErrorCommand),
  /// list modules in the project
  Modules(QueryModulesCommand),
  /// read a definition's full code
  Def(QueryDefCommand),
  /// read content at specific path in a definition
  At(QueryAtCommand),
  /// peek definition signature without full body
  Peek(QueryPeekCommand),
  /// read examples of a definition
  Examples(QueryExamplesCommand),
  /// find symbol across namespaces
  Find(QueryFindCommand),
  /// find usages of a definition
  Usages(QueryUsagesCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ns")]
/// list namespaces, or show ns details if namespace provided
pub struct QueryNsCommand {
  /// namespace to show details (optional, lists all if omitted)
  #[argh(positional)]
  pub namespace: Option<String>,
  /// include dependency and core namespaces
  #[argh(switch)]
  pub deps: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "defs")]
/// list definitions in a namespace
pub struct QueryDefsCommand {
  /// namespace to query
  #[argh(positional)]
  pub namespace: String,
}

// read-ns merged into ns command

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "pkg")]
/// get package name
pub struct QueryPkgCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "config")]
/// read project configs (init_fn, reload_fn, version)
pub struct QueryConfigCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "error")]
/// read .calcit-error.cirru file for error stack traces
pub struct QueryErrorCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "modules")]
/// list modules in the project
pub struct QueryModulesCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "def")]
/// read a definition's full code
pub struct QueryDefCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "at")]
/// read content at specific path in a definition
pub struct QueryAtCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0", empty for root)
  #[argh(option, short = 'p', default = "String::new()")]
  pub path: String,
  /// max depth for JSON output (0 = unlimited, default 0)
  #[argh(option, short = 'd', default = "0")]
  pub depth: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "peek")]
/// peek definition signature without full body
pub struct QueryPeekCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "examples")]
/// read examples of a definition
pub struct QueryExamplesCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "find")]
/// find symbol across namespaces; use --fuzzy for pattern matching
pub struct QueryFindCommand {
  /// symbol name to search for (exact match by default, pattern if --fuzzy)
  #[argh(positional)]
  pub symbol: String,
  /// include dependency namespaces in search
  #[argh(switch)]
  pub deps: bool,
  /// fuzzy search: match pattern against "namespace/definition" paths
  #[argh(switch, short = 'f')]
  pub fuzzy: bool,
  /// maximum number of results for fuzzy search (default 20)
  #[argh(option, short = 'n', default = "20")]
  pub limit: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "usages")]
/// find usages of a definition across the project
pub struct QueryUsagesCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// include dependency namespaces in search
  #[argh(switch)]
  pub deps: bool,
}

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
  /// show Cirru syntax guide for LLM code generation
  ShowGuide(CirruShowGuideCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "parse")]
/// parse Cirru code to JSON
pub struct CirruParseCommand {
  /// cirru code to parse
  #[argh(positional)]
  pub code: String,
  /// parse input as a single-line Cirru expression (one-liner parser)
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub expr_one_liner: bool,
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

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "show-guide")]
/// show Cirru syntax guide for LLM code generation
pub struct CirruShowGuideCommand {}

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

// ═══════════════════════════════════════════════════════════════════════════════
// Edit subcommand - code editing operations
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "edit")]
/// edit project code (definitions, namespaces, modules, configs)
pub struct EditCommand {
  #[argh(subcommand)]
  pub subcommand: EditSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum EditSubcommand {
  /// add or update a definition
  Def(EditDefCommand),
  /// delete a definition
  RmDef(EditRmDefCommand),
  /// update definition documentation
  Doc(EditDocCommand),
  /// set definition examples
  Examples(EditExamplesCommand),
  /// add a single example to definition
  AddExample(EditAddExampleCommand),
  /// remove an example from definition by index
  RmExample(EditRmExampleCommand),
  /// operate on definition at specific path
  At(EditAtCommand),
  /// add a new namespace
  AddNs(EditAddNsCommand),
  /// delete a namespace
  RmNs(EditRmNsCommand),
  /// update namespace imports (replace all)
  Imports(EditImportsCommand),
  /// add a require rule to namespace
  Require(EditRequireCommand),
  /// remove a require rule from namespace
  RmRequire(EditRmRequireCommand),
  /// update namespace documentation
  NsDoc(EditNsDocCommand),
  /// create a new module
  AddModule(EditAddModuleCommand),
  /// delete a module
  RmModule(EditRmModuleCommand),
  /// update project configs
  Config(EditConfigCommand),
}

// --- Definition operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "def")]
/// add or update a definition
pub struct EditDefCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// read syntax_tree from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// syntax_tree as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser).
  /// Useful when your input is a single expression (no indentation), e.g.:
  ///   cr edit def ns/name -O --code 'println $ str $ &+ 1 2'
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "json-leaf")]
  pub json_leaf: bool,
  /// read syntax_tree from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// force replace if definition exists
  #[argh(switch, short = 'r')]
  pub replace: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-def")]
/// delete a definition
pub struct EditRmDefCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "doc")]
/// update definition documentation
pub struct EditDocCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// documentation text
  #[argh(positional)]
  pub doc: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "examples")]
/// set definition examples (replaces all)
pub struct EditExamplesCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// read examples from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// examples as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// examples as inline JSON array string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read examples from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser).
  /// For examples, this represents exactly ONE example expression.
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON array
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// clear all examples
  #[argh(switch, long = "clear")]
  pub clear: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "add-example")]
/// add a single example to definition
pub struct EditAddExampleCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// position to insert at (default: append to end)
  #[argh(option, long = "at")]
  pub at: Option<usize>,
  /// read example from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// example as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// example as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read example from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser)
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-example")]
/// remove an example from definition by index
pub struct EditRmExampleCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// index of example to remove (0-based)
  #[argh(positional)]
  pub index: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "at")]
/// operate on definition at specific path
pub struct EditAtCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// operation: "insert-before", "insert-after", "replace", "delete", "insert-child"
  #[argh(option, short = 'o')]
  pub operation: String,
  /// read syntax_tree from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// syntax_tree as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read syntax_tree from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser)
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "json-leaf")]
  pub json_leaf: bool,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
}

// --- Namespace operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "add-ns")]
/// add a new namespace (ns syntax_tree input: Cirru by default; use --json-input or -j for JSON)
pub struct EditAddNsCommand {
  /// namespace name to create
  #[argh(positional)]
  pub namespace: String,
  /// read ns syntax_tree from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// ns syntax_tree as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// ns syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read ns syntax_tree from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser)
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "json-leaf")]
  pub json_leaf: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-ns")]
/// delete a namespace
pub struct EditRmNsCommand {
  /// namespace to delete
  #[argh(positional)]
  pub namespace: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "imports")]
/// update namespace imports (replaces all)
pub struct EditImportsCommand {
  /// namespace to update
  #[argh(positional)]
  pub namespace: String,
  /// read imports from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// imports as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// imports as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read imports from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser)
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "json-leaf")]
  pub json_leaf: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "require")]
/// add a require rule to namespace
pub struct EditRequireCommand {
  /// namespace to add require rule to
  #[argh(positional)]
  pub namespace: String,
  /// read require rule from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// require rule as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// require rule as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read require rule from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default, explicit for clarity)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// parse input as a single-line Cirru expression (one-liner parser)
  #[argh(switch, short = 'O', long = "cirru-one")]
  pub cirru_expr_one_liner: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "json-leaf")]
  pub json_leaf: bool,
  /// overwrite existing rule for the same source namespace
  #[argh(switch, short = 'o', long = "overwrite")]
  pub overwrite: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-require")]
/// remove a require rule from namespace
pub struct EditRmRequireCommand {
  /// namespace to remove require rule from
  #[argh(positional)]
  pub namespace: String,
  /// source namespace to remove (e.g. "calcit.core")
  #[argh(positional)]
  pub source_ns: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ns-doc")]
/// update namespace documentation
pub struct EditNsDocCommand {
  /// namespace to update
  #[argh(positional)]
  pub namespace: String,
  /// documentation text
  #[argh(positional)]
  pub doc: String,
}

// --- Module operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "add-module")]
/// create a new module (adds to configs.modules)
pub struct EditAddModuleCommand {
  /// module path to add (e.g. "calcit-test/")
  #[argh(positional)]
  pub module_path: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-module")]
/// delete a module
pub struct EditRmModuleCommand {
  /// module path to delete
  #[argh(positional)]
  pub module_path: String,
}

// --- Config operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "config")]
/// update project config values
pub struct EditConfigCommand {
  /// config key: "init-fn", "reload-fn", "version"
  #[argh(positional)]
  pub key: String,
  /// config value
  #[argh(positional)]
  pub value: String,
}

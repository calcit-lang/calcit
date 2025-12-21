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
  /// read a definition's content
  ReadDef(QueryReadDefCommand),
  /// read content at specific path in a definition
  ReadAt(QueryReadAtCommand),
  /// peek definition signature (name, params, doc) without full body
  PeekDef(QueryPeekDefCommand),
  /// find symbol across all namespaces
  FindSymbol(QueryFindSymbolCommand),
  /// find usages of a definition
  Usages(QueryUsagesCommand),
  /// fuzzy search definitions by namespace/name pattern
  Search(QuerySearchCommand),
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

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "read-def")]
/// read a definition's full syntax tree as JSON
pub struct QueryReadDefCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name
  #[argh(positional)]
  pub definition: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "read-at")]
/// read content at specific path in a definition (for exploring code tree)
pub struct QueryReadAtCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name
  #[argh(positional)]
  pub definition: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0", empty for root)
  #[argh(option, short = 'p', default = "String::new()")]
  pub path: String,
  /// max depth for JSON output (0 = unlimited, default 0)
  #[argh(option, short = 'd', default = "0")]
  pub depth: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "peek-def")]
/// peek definition signature (name, params, doc) without full implementation body
pub struct QueryPeekDefCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name
  #[argh(positional)]
  pub definition: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "find-symbol")]
/// find symbol across all namespaces (definitions and references)
pub struct QueryFindSymbolCommand {
  /// symbol name to search for
  #[argh(positional)]
  pub symbol: String,
  /// include dependency namespaces in search
  #[argh(switch)]
  pub deps: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "usages")]
/// find usages of a definition across the project
pub struct QueryUsagesCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name to find usages of
  #[argh(positional)]
  pub definition: String,
  /// include dependency namespaces in search
  #[argh(switch)]
  pub deps: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// fuzzy search definitions by namespace/name pattern (e.g. "app/add", "core", "my-fn")
pub struct QuerySearchCommand {
  /// search pattern (matches against "namespace/definition" path)
  #[argh(positional)]
  pub pattern: String,
  /// include dependency namespaces in search
  #[argh(switch)]
  pub deps: bool,
  /// maximum number of results (default 20)
  #[argh(option, short = 'n', default = "20")]
  pub limit: usize,
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
  /// add or update a definition (upsert)
  UpsertDef(EditUpsertDefCommand),
  /// delete a definition
  DeleteDef(EditDeleteDefCommand),
  /// update definition documentation
  UpdateDefDoc(EditUpdateDefDocCommand),
  /// operate on definition at specific path
  OperateAt(EditOperateAtCommand),
  /// add a new namespace
  AddNs(EditAddNsCommand),
  /// delete a namespace
  DeleteNs(EditDeleteNsCommand),
  /// update namespace imports
  UpdateImports(EditUpdateImportsCommand),
  /// update namespace documentation
  UpdateNsDoc(EditUpdateNsDocCommand),
  /// create a new module
  AddModule(EditAddModuleCommand),
  /// delete a module
  DeleteModule(EditDeleteModuleCommand),
  /// update project configs
  SetConfig(EditSetConfigCommand),
}

// --- Definition operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "upsert-def")]
/// add or update a definition (syntax_tree input: Cirru by default; use --json-input or -j for JSON)
pub struct EditUpsertDefCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name
  #[argh(positional)]
  pub definition: String,
  /// read syntax_tree JSON from file
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// treat input as cirru text (default if not specified)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// treat file/stdin input as JSON (must be specified to accept JSON files/stdin)
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// read syntax_tree JSON from stdin
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// force replace if definition exists
  #[argh(switch, short = 'r')]
  pub replace: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "delete-def")]
/// delete a definition from namespace
pub struct EditDeleteDefCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name to delete
  #[argh(positional)]
  pub definition: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "update-def-doc")]
/// update documentation for a definition
pub struct EditUpdateDefDocCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name
  #[argh(positional)]
  pub definition: String,
  /// documentation text
  #[argh(positional)]
  pub doc: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "operate-at")]
/// operate on definition at specific path (input: Cirru by default; use --json-input or -j for JSON)
pub struct EditOperateAtCommand {
  /// namespace containing the definition
  #[argh(positional)]
  pub namespace: String,
  /// definition name
  #[argh(positional)]
  pub definition: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// operation: "insert-before", "insert-after", "replace", "delete", "insert-child"
  #[argh(option, short = 'o')]
  pub operation: String,
  /// read syntax_tree JSON from file (for insert/replace operations)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read syntax_tree JSON from stdin
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default if not specified)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// treat file/stdin input as JSON (must be specified to accept JSON files/stdin)
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

// --- Namespace operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "add-ns")]
/// add a new namespace (ns syntax_tree input: Cirru by default; use --json-input or -j for JSON)
pub struct EditAddNsCommand {
  /// namespace name to create
  #[argh(positional)]
  pub namespace: String,
  /// read ns syntax_tree JSON from file (optional, creates minimal ns if not provided)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// ns syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read ns syntax_tree JSON from stdin
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default if not specified)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// treat file/stdin input as JSON (must be specified to accept JSON files/stdin)
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "delete-ns")]
/// delete a namespace
pub struct EditDeleteNsCommand {
  /// namespace to delete
  #[argh(positional)]
  pub namespace: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "update-imports")]
/// update namespace import rules (input: Cirru by default; use --json-input or -j for JSON)
pub struct EditUpdateImportsCommand {
  /// namespace to update
  #[argh(positional)]
  pub namespace: String,
  /// read imports JSON from file
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// imports as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read imports JSON from stdin
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat input as cirru text (default if not specified)
  #[argh(switch, short = 'c', long = "cirru")]
  pub cirru: bool,
  /// treat file/stdin input as JSON (must be specified to accept JSON files/stdin)
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "update-ns-doc")]
/// update documentation for a namespace
pub struct EditUpdateNsDocCommand {
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
#[argh(subcommand, name = "delete-module")]
/// delete a module from configs
pub struct EditDeleteModuleCommand {
  /// module path to delete
  #[argh(positional)]
  pub module_path: String,
}

// --- Config operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "set-config")]
/// update project config values
pub struct EditSetConfigCommand {
  /// config key: "init-fn", "reload-fn", "version"
  #[argh(positional)]
  pub key: String,
  /// config value
  #[argh(positional)]
  pub value: String,
}

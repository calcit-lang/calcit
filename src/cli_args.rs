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
  /// analyze code structure (call-graph, count-calls, check-examples)
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
  /// fine-grained tree operations (view and modify AST nodes)
  Tree(TreeCommand),
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
/// analyze code structure (call-graph, count-calls, check-examples)
pub struct AnalyzeCommand {
  #[argh(subcommand)]
  pub subcommand: AnalyzeSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum AnalyzeSubcommand {
  /// analyze call graph structure from entry point
  CallGraph(CallGraphCommand),
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
#[argh(subcommand, name = "call-graph")]
pub struct CallGraphCommand {
  /// directly specify root definition to analyze (format: ns/def). If omitted, uses init-fn from config
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
  /// directly specify root definition to analyze (format: ns/def). If omitted, uses init-fn from config
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
  /// peek definition signature without full body
  Peek(QueryPeekCommand),
  /// read examples of a definition
  Examples(QueryExamplesCommand),
  /// find symbol across namespaces
  Find(QueryFindCommand),
  /// find usages of a definition
  Usages(QueryUsagesCommand),
  /// search for leaf nodes (strings) in definition
  Search(QuerySearchCommand),
  /// search for structural patterns (Cirru expr or JSON array) in definition
  SearchPattern(QuerySearchPatternCommand),
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

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// search for leaf nodes (strings) across project or in specific namespace/definition
pub struct QuerySearchCommand {
  /// string pattern to search for in leaf nodes
  #[argh(positional)]
  pub pattern: String,
  /// filter search to specific namespace or namespace/definition (optional)
  #[argh(option, short = 'f', long = "filter")]
  pub filter: Option<String>,
  /// loose match: find nodes containing the pattern (not exact match)
  #[argh(switch, short = 'l')]
  pub loose: bool,
  /// maximum search depth (0 = unlimited)
  #[argh(option, short = 'd', default = "0")]
  pub max_depth: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search-pattern")]
/// search for structural patterns (Cirru expr or JSON array) across project or in specific namespace/definition
pub struct QuerySearchPatternCommand {
  /// pattern to search for (Cirru one-liner or JSON array with -j)
  #[argh(positional)]
  pub pattern: String,
  /// filter search to specific namespace or namespace/definition (optional)
  #[argh(option, short = 'f', long = "filter")]
  pub filter: Option<String>,
  /// loose match: find sequences containing the pattern (not exact match)
  #[argh(switch, short = 'l')]
  pub loose: bool,
  /// maximum search depth (0 = unlimited)
  #[argh(option, short = 'd', default = "0")]
  pub max_depth: usize,
  /// treat pattern as JSON array instead of Cirru expr
  #[argh(switch, short = 'j')]
  pub json: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Docs subcommand - documentation tools
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "docs")]
/// documentation tools (guidebook)
pub struct DocsCommand {
  #[argh(subcommand)]
  pub subcommand: DocsSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum DocsSubcommand {
  /// search guidebook documentation by keyword
  Search(DocsSearchCommand),
  /// read a specific guidebook document
  Read(DocsReadCommand),
  /// list all guidebook documentation topics
  List(DocsListCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// search guidebook documentation by keyword
pub struct DocsSearchCommand {
  /// keyword to search
  #[argh(positional)]
  pub keyword: String,
  /// number of context lines to show before and after match (default: 5)
  #[argh(option, short = 'c', default = "5")]
  pub context: usize,
  /// filter by filename (optional)
  #[argh(option, short = 'f')]
  pub filename: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "read")]
/// read a specific guidebook document
pub struct DocsReadCommand {
  /// filename to read (e.g., "syntax.md")
  #[argh(positional)]
  pub filename: String,
  /// starting line number (default: 0)
  #[argh(option, short = 's', default = "0")]
  pub start: usize,
  /// number of lines to read (default: 80)
  #[argh(option, short = 'n', default = "80")]
  pub lines: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "list")]
/// list all guidebook documentation topics
pub struct DocsListCommand {}

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
  /// parse input as a single-line Cirru expression (one-liner parser, default is multi-line)
  #[argh(switch, short = 'e', long = "expr-one")]
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
  /// scan markdown files in a module directory
  ScanMd(LibsScanMdCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "readme")]
/// show README of a library from local ~/.config/calcit/modules or GitHub
pub struct LibsReadmeCommand {
  /// package name to look up
  #[argh(positional)]
  pub package: String,
  /// optional file path relative to package directory (e.g., "Skills.md")
  #[argh(option, short = 'f')]
  pub file: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// search libraries by keyword in name or description
pub struct LibsSearchCommand {
  /// keyword to search
  #[argh(positional)]
  pub keyword: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "scan-md")]
/// scan markdown files in a module directory
pub struct LibsScanMdCommand {
  /// module name to scan
  #[argh(positional)]
  pub module: String,
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
  /// add a new namespace
  AddNs(EditAddNsCommand),
  /// delete a namespace
  RmNs(EditRmNsCommand),
  /// update namespace imports (replace all)
  Imports(EditImportsCommand),
  /// add a single import rule to namespace
  AddImport(EditAddImportCommand),
  /// remove an import rule from namespace
  RmImport(EditRmImportCommand),
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
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// treat file/stdin input as JSON array
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "add-import")]
/// add a single import rule to namespace
pub struct EditAddImportCommand {
  /// namespace to add import rule to
  #[argh(positional)]
  pub namespace: String,
  /// read import rule from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// import rule as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// import rule as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// read import rule from stdin (Cirru format by default, use -J for JSON)
  #[argh(switch, short = 's')]
  pub stdin: bool,
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// overwrite existing rule for the same source namespace
  #[argh(switch, short = 'o', long = "overwrite")]
  pub overwrite: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-import")]
/// remove an import rule from namespace
pub struct EditRmImportCommand {
  /// namespace to remove import rule from
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

// ========================================================================
// Code command - fine-grained code tree operations
// ========================================================================

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "tree")]
/// fine-grained code tree operations (view and modify AST nodes)
pub struct TreeCommand {
  #[argh(subcommand)]
  pub subcommand: TreeSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum TreeSubcommand {
  Show(TreeShowCommand),
  Replace(TreeReplaceCommand),
  Delete(TreeDeleteCommand),
  InsertBefore(TreeInsertBeforeCommand),
  InsertAfter(TreeInsertAfterCommand),
  InsertChild(TreeInsertChildCommand),
  AppendChild(TreeAppendChildCommand),
  SwapNext(TreeSwapNextCommand),
  SwapPrev(TreeSwapPrevCommand),
  Wrap(TreeWrapCommand),
}

/// view tree node at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "show")]
pub struct TreeShowCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// replace node at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "replace")]
pub struct TreeReplaceCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
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
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// delete node at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "delete")]
pub struct TreeDeleteCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// insert node before target at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "insert-before")]
pub struct TreeInsertBeforeCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// insert node after target at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "insert-after")]
pub struct TreeInsertAfterCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// insert node as first child of target at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "insert-child")]
pub struct TreeInsertChildCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// append node as last child of target at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "append-child")]
pub struct TreeAppendChildCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// swap node with next sibling
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "swap-next")]
pub struct TreeSwapNextCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// swap node with previous sibling
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "swap-prev")]
pub struct TreeSwapPrevCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// wrap node with new structure (use refer-original placeholder for original node)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "wrap")]
pub struct TreeWrapCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file/stdin input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file/stdin input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// placeholder to refer to the original node (e.g., "$$$$")
  #[argh(option, long = "refer-original")]
  pub refer_original: Option<String>,
  /// comma-separated path to inner branch of original node (e.g., "1,2,3")
  #[argh(option, long = "refer-inner-branch")]
  pub refer_inner_branch: Option<String>,
  /// placeholder for inner branch reference (e.g., "####")
  #[argh(option, long = "refer-inner-placeholder")]
  pub refer_inner_placeholder: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

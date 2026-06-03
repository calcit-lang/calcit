use argh::FromArgs;

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(FromArgs, PartialEq, Debug, Clone)]
/// Top-level command.
pub struct ToplevelCalcit {
  #[argh(subcommand)]
  pub subcommand: Option<CalcitCommand>,
  /// enable watch mode for direct run mode (default behavior is run once)
  #[argh(switch, short = 'w')]
  pub watch: bool,
  /// check-only mode: validate without execution or codegen
  #[argh(switch)]
  pub check_only: bool,
  /// disable stack trace for errors
  #[argh(switch)]
  pub disable_stack: bool,
  /// skip arity check in js codegen
  #[argh(switch)]
  pub skip_arity_check: bool,
  /// warn on dynamic method calls that cannot be monomorphized
  #[argh(switch)]
  pub warn_dyn_method: bool,
  /// print FFI dylib calls and callbacks for debugging native crashes
  #[argh(switch)]
  pub trace_ffi: bool,
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
  /// input source file, defaults to "calcit.cirru" and falls back to "compact.cirru"
  #[argh(positional, default = "String::from(crate::DEFAULT_SNAPSHOT_FILE)")]
  pub input: String,
  /// print version only
  #[argh(switch)]
  pub version: bool,
  /// show full tips output in all commands
  #[argh(switch)]
  pub tips: bool,
  /// control tips verbosity: minimal (default), full, none
  #[argh(option)]
  pub tips_level: Option<String>,
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
  /// analyze code structure and helpers (call-graph, call-graph-diff, count-calls, def-diff, check-examples)
  Analyze(AnalyzeCommand),
  /// query project information (namespaces, definitions, configs)
  Query(QueryCommand),
  /// documentation tools for guidebook, installed module docs, and local markdown docs
  Docs(DocsCommand),
  /// Cirru syntax tools (parse, format)
  Cirru(CirruCommand),
  /// legacy alias for docs remote-libs
  Libs(LibsCommand),
  /// edit project code (definitions, namespaces, modules, configs)
  Edit(EditCommand),
  /// fine-grained tree operations (view and modify AST nodes)
  Tree(TreeCommand),
  /// manage project configuration (show, set, modules, add-module, rm-module)
  Config(ConfigCommand),
}

/// emit JavaScript rather than interpreting
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "js")]
pub struct EmitJsCommand {
  /// enable watch mode (default behavior is run once)
  #[argh(switch, short = 'w')]
  pub watch: bool,
  /// check-only mode for JS emit
  #[argh(switch)]
  pub check_only: bool,
}

/// emit Cirru EDN representation of program to program-ir.cirru
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "ir")]
pub struct EmitIrCommand {
  /// enable watch mode (default behavior is run once)
  #[argh(switch, short = 'w')]
  pub watch: bool,
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
/// analyze code structure and helpers (call-graph, call-graph-diff, count-calls, program-diff, check-examples, check-types, weak-types, js-escape)
pub struct AnalyzeCommand {
  #[argh(subcommand)]
  pub subcommand: AnalyzeSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum AnalyzeSubcommand {
  /// analyze call graph structure from entry point
  CallGraph(CallGraphCommand),
  /// compare call graph structure against a Git ref and annotate code changes
  CallGraphDiff(CallGraphDiffCommand),
  /// count call occurrences from entry point
  CountCalls(CountCallsCommand),
  /// compare current snapshot (or one definition) against a Git ref with structured tree diff
  ProgramDiff(ProgramDiffCommand),
  /// check examples in namespace
  CheckExamples(CheckExamplesCommand),
  /// check type-information coverage in namespace definitions
  CheckTypes(CheckTypesCommand),
  /// locate weakly-typed hotspots such as :dynamic schema usage and nil literals
  WeakTypes(WeakTypesCommand),
  /// escape a Calcit symbol into JavaScript-safe identifier form
  JsEscape(JsEscapeCommand),
  /// decode escaped JavaScript identifier back to Calcit symbol (best-effort)
  JsUnescape(JsUnescapeCommand),
}

/// escape a Calcit symbol into JavaScript-safe identifier form
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "js-escape")]
pub struct JsEscapeCommand {
  /// original Calcit symbol
  #[argh(positional)]
  pub symbol: String,
}

/// decode escaped JavaScript identifier back to Calcit symbol (best-effort)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "js-unescape")]
pub struct JsUnescapeCommand {
  /// escaped JavaScript identifier
  #[argh(positional)]
  pub symbol: String,
}

/// check type-information coverage in namespace definitions
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "check-types")]
pub struct CheckTypesCommand {
  /// exact namespace to analyze
  #[argh(option)]
  pub ns: Option<String>,
  /// namespace prefix scope filter
  #[argh(option)]
  pub ns_prefix: Option<String>,
  /// coverage levels to include, comma-separated: none,partial,full
  #[argh(option)]
  pub only: Option<String>,
  /// include dependency/core namespaces
  #[argh(switch)]
  pub deps: bool,
}

/// locate weakly-typed hotspots in schema and code
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "weak-types")]
pub struct WeakTypesCommand {
  /// exact namespace to analyze
  #[argh(option)]
  pub ns: Option<String>,
  /// namespace prefix scope filter
  #[argh(option)]
  pub ns_prefix: Option<String>,
  /// match kinds to include, comma-separated: schema-dynamic,code-dynamic,code-nil
  #[argh(option)]
  pub only: Option<String>,
  /// include dependency/core namespaces
  #[argh(switch)]
  pub deps: bool,
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

/// compare call graph structure against a Git ref and annotate code changes
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "call-graph-diff")]
pub struct CallGraphDiffCommand {
  /// git reference to compare against, e.g. HEAD~1, main, v0.1.0, or a commit SHA
  #[argh(positional)]
  pub git_ref: String,
  /// directly specify root definition to analyze (format: ns/def). If omitted, uses current config init-fn
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

/// compare current snapshot against a Git ref with structured tree diff; use --def to narrow to one definition
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "program-diff")]
pub struct ProgramDiffCommand {
  /// git reference to compare against, e.g. HEAD~1, main, v0.1.0, or a commit SHA
  #[argh(positional)]
  pub git_ref: String,
  /// narrow diff to a single definition in format ns/def
  #[argh(option)]
  pub def: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Query subcommand - project information queries
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "query")]
/// query project and builtin information (namespaces, definitions, configs)
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
  /// search for structural expressions (Cirru expr or JSON array) in definition
  SearchExpr(QuerySearchExprCommand),
  /// read a definition's schema (type information)
  Schema(QuerySchemaCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "schema")]
/// read a definition's schema (type information)
pub struct QuerySchemaCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// also output JSON format for programmatic consumption
  #[argh(switch, short = 'j')]
  pub json: bool,
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
/// read a definition's full code, or builtin metadata when source is unavailable
pub struct QueryDefCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// also output JSON format for programmatic consumption
  #[argh(switch, short = 'j')]
  pub json: bool,
  /// preferred nodes per display fragment when large expressions are chunked
  #[argh(option, default = "56")]
  pub chunk_target_nodes: usize,
  /// stop recursive chunk splitting once fragments fall below this node count
  #[argh(option, default = "68")]
  pub chunk_max_nodes: usize,
  /// only enable chunked display when total expression nodes reach this threshold
  #[argh(option, default = "88")]
  pub chunk_trigger_nodes: usize,
  /// force raw full-definition display without chunking
  #[argh(switch)]
  pub raw: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "peek")]
/// peek definition signature or builtin metadata without full body
pub struct QueryPeekCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "examples")]
/// read examples of a definition or builtin helper
pub struct QueryExamplesCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "find")]
/// find symbol across namespaces (fuzzy match by default; use --exact for precise match)
pub struct QueryFindCommand {
  /// symbol name or pattern to search for (fuzzy match by default)
  #[argh(positional)]
  pub symbol: String,
  /// include dependency namespaces in search
  #[argh(switch)]
  pub deps: bool,
  /// exact match: only match definitions with this exact name
  #[argh(switch)]
  pub exact: bool,
  /// maximum number of results (default 20)
  #[argh(option, short = 'n', default = "20")]
  pub limit: usize,
  /// start index for detailed display window (3 detailed items)
  #[argh(option, long = "detail-offset", default = "0")]
  pub detail_offset: usize,
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
  /// start index for detailed display window (3 detailed items)
  #[argh(option, long = "detail-offset", default = "0")]
  pub detail_offset: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// search for leaf nodes (strings) across project or in specific namespace/definition (fuzzy match by default)
pub struct QuerySearchCommand {
  /// string pattern to search for in leaf nodes
  #[argh(positional)]
  pub pattern: String,
  /// filter search to specific namespace or namespace/definition (optional)
  #[argh(option, short = 'f', long = "filter")]
  pub filter: Option<String>,
  /// exact match: only match nodes equal to the pattern (default is contains-match)
  #[argh(switch)]
  pub exact: bool,
  /// maximum search depth (0 = unlimited)
  #[argh(option, short = 'd', default = "0")]
  pub max_depth: usize,
  /// start search from specific path (dot-separated indices preferred, e.g. "2.1.0")
  #[argh(option, short = 'p', long = "start-path")]
  pub start_path: Option<String>,
  /// include modules configured for a specific entry in `entries`
  #[argh(option, long = "entry")]
  pub entry: Option<String>,
  /// start index for detailed display window (3 detailed items)
  #[argh(option, long = "detail-offset", default = "0")]
  pub detail_offset: usize,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search-expr")]
/// search for structural expressions (Cirru expr or JSON array) across project or in specific namespace/definition (fuzzy match by default)
pub struct QuerySearchExprCommand {
  /// pattern to search for (Cirru one-liner or JSON array with -j)
  #[argh(positional)]
  pub pattern: String,
  /// filter search to specific namespace or namespace/definition (optional)
  #[argh(option, short = 'f', long = "filter")]
  pub filter: Option<String>,
  /// exact match: only match structurally identical expressions (default is prefix/contains match)
  #[argh(switch)]
  pub exact: bool,
  /// maximum search depth (0 = unlimited)
  #[argh(option, short = 'd', default = "0")]
  pub max_depth: usize,
  /// treat pattern as JSON array instead of Cirru expr
  #[argh(switch, short = 'j')]
  pub json: bool,
  /// include modules configured for a specific entry in `entries`
  #[argh(option, long = "entry")]
  pub entry: Option<String>,
  /// start index for detailed display window (3 detailed items)
  #[argh(option, long = "detail-offset", default = "0")]
  pub detail_offset: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Docs subcommand - documentation tools
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "docs")]
/// documentation tools for calcit guidebook and installed module docs
pub struct DocsCommand {
  #[argh(subcommand)]
  pub subcommand: DocsSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum DocsSubcommand {
  /// list available doc scopes (calcit and installed modules)
  Scopes(DocsScopesCommand),
  /// browse remote library registry and read remote library docs
  RemoteLibs(DocsRemoteLibsCommand),
  /// search calcit guidebook or installed module docs by keyword
  Search(DocsSearchCommand),
  /// list available files in calcit guidebook or one installed module
  List(DocsListCommand),
  /// list markdown section headings in one file
  Sections(DocsSectionsCommand),
  /// read markdown content from calcit guidebook or one installed module
  Read(DocsReadCommand),
  /// read cached Agents guide (auto-refresh daily)
  Agents(DocsAgentsCommand),
  /// read a specific line range from calcit guidebook or installed module docs
  ReadLines(DocsReadLinesCommand),
  /// check ```cirru code blocks in a markdown file via eval
  CheckMd(DocsCheckMdCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "scopes")]
/// list available doc scopes (calcit and installed modules)
pub struct DocsScopesCommand {}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "remote-libs")]
/// browse remote library registry and read remote library docs
pub struct DocsRemoteLibsCommand {
  #[argh(subcommand)]
  pub subcommand: Option<LibsSubcommand>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "search")]
/// search calcit guidebook by default, or switch to one installed module with --module
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
  /// search docs for a specific installed module (e.g. respo.calcit)
  #[argh(option)]
  pub module: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "list")]
/// list available files in calcit guidebook or one installed module
pub struct DocsListCommand {
  /// limit listing to one installed module (e.g. respo.calcit)
  #[argh(option)]
  pub module: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "sections")]
/// list markdown section headings in one file
pub struct DocsSectionsCommand {
  /// filename to inspect (e.g., "intro.md")
  #[argh(positional)]
  pub filename: String,
  /// read docs from a specific installed module (e.g. respo.calcit)
  #[argh(option)]
  pub module: Option<String>,
  /// show line numbers in section titles
  #[argh(switch)]
  pub with_lines: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "read")]
/// read markdown content from calcit guidebook by default, or from one installed module with --module
pub struct DocsReadCommand {
  /// filename to read (e.g., "syntax.md")
  #[argh(positional)]
  pub filename: String,
  /// optional section heading keyword(s) for fuzzy match, can pass multiple; omit to read full file
  #[argh(positional)]
  pub headings: Vec<String>,
  /// do not include nested subheadings when showing matched parent heading content
  #[argh(switch)]
  pub no_subheadings: bool,
  /// show full file content directly
  #[argh(switch)]
  pub full: bool,
  /// show line numbers in heading list and section titles
  #[argh(switch)]
  pub with_lines: bool,
  /// read docs from a specific installed module (e.g. respo.calcit)
  #[argh(option)]
  pub module: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "agents")]
/// read Agents.md with local cache (~/.config/calcit/Agents.md), refresh if older than 1 day
pub struct DocsAgentsCommand {
  /// heading keyword(s) for fuzzy match, can pass multiple; if omitted, list all markdown headings
  #[argh(positional)]
  pub headings: Vec<String>,
  /// do not include nested subheadings when showing matched parent heading content
  #[argh(switch)]
  pub no_subheadings: bool,
  /// show full file content directly
  #[argh(switch)]
  pub full: bool,
  /// show line numbers in heading list and section titles
  #[argh(switch)]
  pub with_lines: bool,
  /// force refresh from remote and ignore cache age
  #[argh(switch)]
  pub refresh: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "read-lines")]
/// read a specific line range from calcit guidebook by default, or from one installed module with --module
pub struct DocsReadLinesCommand {
  /// filename to read (e.g., "syntax.md")
  #[argh(positional)]
  pub filename: String,
  /// starting line number (default: 0)
  #[argh(option, short = 's', default = "0")]
  pub start: usize,
  /// number of lines to read (default: 80)
  #[argh(option, short = 'n', default = "80")]
  pub lines: usize,
  /// read docs from a specific installed module (e.g. respo.calcit)
  #[argh(option)]
  pub module: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "check-md")]
/// check ```cirru code blocks in a markdown file via eval
pub struct DocsCheckMdCommand {
  /// path to the markdown file to check
  #[argh(positional)]
  pub file: String,
  /// entry .cirru file for eval context (default: demos/calcit.cirru)
  #[argh(option, short = 'd', default = "String::from(\"demos/calcit.cirru\")")]
  pub entry: String,
  /// extra dependency module path for eval context, can be provided multiple times; defaults to modules from entry configs.modules; paths ending with '/' prefer calcit.cirru and fall back to compact.cirru
  #[argh(option)]
  pub dep: Vec<String>,
}

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
  /// perform basic syntax validation after parsing (checks keywords, strings, numbers)
  #[argh(switch, short = 'v', long = "validate")]
  pub validate: bool,
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
/// legacy alias for docs remote-libs
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
  /// heading keyword(s) for fuzzy match, can pass multiple; if omitted, list markdown headings
  #[argh(positional)]
  pub headings: Vec<String>,
  /// optional file path relative to package directory (e.g., "Skills.md")
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// do not include nested subheadings when showing matched parent heading content
  #[argh(switch)]
  pub no_subheadings: bool,
  /// show full file content directly
  #[argh(switch)]
  pub full: bool,
  /// show line numbers in heading list and section titles
  #[argh(switch)]
  pub with_lines: bool,
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
  #[argh(positional, default = "String::new()")]
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
  /// rewrite snapshot file in canonical format without semantic changes
  Format(EditFormatCommand),
  /// add or update a definition
  Def(EditDefCommand),
  /// move a definition to another namespace
  MvDef(EditMvDefCommand),
  /// delete a definition
  RmDef(EditRmDefCommand),
  /// update definition documentation
  Doc(EditDocCommand),
  /// update definition schema payload (inside quote)
  Schema(EditSchemaCommand),
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
  /// describe incremental code changes and export them to .calcit-error.cirru
  Inc(EditIncCommand),
  /// copy node from one path to another within a definition
  Cp(EditCpCommand),
  /// move node from one path to another within a definition (removes source)
  Mv(EditMvNodeCommand),
  /// rename a definition within its namespace (no overwrite)
  Rename(EditRenameCommand),
  /// extract a sub-expression into a new definition and replace in-place with the new name
  SplitDef(EditSplitDefCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "format")]
/// rewrite target snapshot file in canonical format
pub struct EditFormatCommand {}

// --- Definition operations ---

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "def")]
/// add a new definition
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
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// overwrite existing definition if it already exists
  #[argh(switch, long = "overwrite")]
  pub overwrite: bool,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "mv-def")]
/// move a definition to another namespace or rename it
pub struct EditMvDefCommand {
  /// source in format "namespace/definition"
  #[argh(positional)]
  pub source: String,
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
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
/// documentation tools for guidebook, installed module docs, and local markdown docs
pub struct EditDocCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// documentation text
  #[argh(positional)]
  pub doc: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "schema")]
/// update definition schema (validates structure before writing; cr edit format normalises old quote-wrapped schemas to direct map)
pub struct EditSchemaCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// read schema from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// schema as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// schema as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// clear schema field
  #[argh(switch, long = "clear")]
  pub clear: bool,
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
  /// treat file input as JSON array
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
  /// treat file input as JSON
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
  /// treat file input as JSON
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
  /// treat file input as JSON
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
  /// treat file input as JSON
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

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "inc")]
/// record incremental changes (defs and namespaces) for downstream tooling
pub struct EditIncCommand {
  /// namespaces whose entire file should be treated as newly added (e.g. "app.new")
  #[argh(option, long = "added-ns")]
  pub added_ns: Vec<String>,
  /// namespaces that should be treated as removed from the project
  #[argh(option, long = "removed-ns")]
  pub removed_ns: Vec<String>,
  /// namespaces whose ns form/imports changed (stores latest ns block)
  #[argh(option, long = "ns-updated")]
  pub ns_updated: Vec<String>,
  /// definitions that were newly added (format: namespace/definition)
  #[argh(option, long = "added")]
  pub added: Vec<String>,
  /// definitions that were deleted (format: namespace/definition)
  #[argh(option, long = "removed")]
  pub removed: Vec<String>,
  /// definitions that were modified (format: namespace/definition)
  #[argh(option, long = "changed")]
  pub changed: Vec<String>,
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
  ReplaceLeaf(TreeReplaceLeafCommand),
  Delete(TreeDeleteCommand),
  InsertBefore(TreeInsertBeforeCommand),
  InsertAfter(TreeInsertAfterCommand),
  InsertChild(TreeInsertChildCommand),
  AppendChild(TreeAppendChildCommand),
  SwapNext(TreeSwapNextCommand),
  SwapPrev(TreeSwapPrevCommand),
  Unwrap(TreeUnwrapCommand),
  Raise(TreeRaiseCommand),
  Wrap(TreeWrapCommand),
  TargetReplace(TreeTargetReplaceCommand),
  Rewrite(TreeStructuralCommand),
}

/// view tree node at specific path
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "show")]
pub struct TreeShowCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node (dot-separated preferred; e.g. "2.1.0"); omit to show from root
  #[argh(option, short = 'p')]
  pub path: Option<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
  /// also output JSON format for programmatic consumption
  #[argh(switch, short = 'j')]
  pub json: bool,
  /// preferred nodes per display fragment when large expressions are chunked
  #[argh(option, default = "56")]
  pub chunk_target_nodes: usize,
  /// stop recursive chunk splitting once fragments fall below this node count
  #[argh(option, default = "68")]
  pub chunk_max_nodes: usize,
  /// only enable chunked display when total expression nodes reach this threshold
  #[argh(option, default = "88")]
  pub chunk_trigger_nodes: usize,
  /// nested chunk layers to expand beyond ROOT (default 1 shows ROOT + direct chunks only)
  #[argh(option, default = "1")]
  pub chunk_expand_depth: usize,
  /// force raw subtree display without chunking
  #[argh(switch)]
  pub raw: bool,
}

/// copy node from one path to another within a definition
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "cp")]
pub struct EditCpCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the source node (comma-separated indices)
  #[argh(option, long = "from")]
  pub from: String,
  /// path to the destination node (comma-separated indices)
  #[argh(option, short = 'p', long = "path")]
  pub path: String,
  /// position relative to the destination node (before, after, append-child, prepend-child, replace)
  #[argh(option, long = "at", default = "String::from(\"after\")")]
  pub at: String,
}

/// move node from one path to another within a definition (removes source)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "mv")]
pub struct EditMvNodeCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the source node (comma-separated indices)
  #[argh(option, long = "from")]
  pub from: String,
  /// path to the destination node (comma-separated indices)
  #[argh(option, short = 'p', long = "path")]
  pub path: String,
  /// position relative to the destination node (before, after, append-child, prepend-child, replace)
  #[argh(option, long = "at", default = "String::from(\"after\")")]
  pub at: String,
}

/// rename a definition within its namespace (no overwrite)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rename")]
pub struct EditRenameCommand {
  /// source in format "namespace/definition"
  #[argh(positional)]
  pub source: String,
  /// new definition name (within same namespace)
  #[argh(positional)]
  pub new_name: String,
}

/// extract a sub-expression into a new definition and replace the original location with the new name
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "split-def")]
pub struct EditSplitDefCommand {
  /// source definition in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node to extract (comma-separated indices, e.g. "3,2,1")
  #[argh(option, short = 'p', long = "path")]
  pub path: String,
  /// name for the new extracted definition (within the same namespace)
  #[argh(option, short = 'n', long = "name")]
  pub new_name: String,
}

/// rewrite node using references; requires `--with name=path` (use `replace` if no references)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rewrite")]
pub struct TreeStructuralCommand {
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
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// bind placeholder to original-node path: `--with self=.` , `--with rhs=2`
  #[argh(option, short = 'w', long = "with")]
  pub with: Vec<String>,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// find unique leaf node and replace it; if multiple found, returns error with helpful hints
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "target-replace")]
pub struct TreeTargetReplaceCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// pattern to search for (exact match on leaf nodes)
  #[argh(option, long = "pattern")]
  pub pattern: String,
  /// read syntax_tree from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// syntax_tree as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes; e.g. --leaf -e 'sym' or --leaf -e '|text')
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// find and replace all matching leaf nodes in definition (no path needed)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "replace-leaf")]
pub struct TreeReplaceLeafCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// pattern to search for (exact match on leaf nodes)
  #[argh(option, long = "pattern")]
  pub pattern: String,
  /// read syntax_tree from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// syntax_tree as inline Cirru text (or JSON when used with -J/--json-input)
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// syntax_tree as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node (single symbol or string, no JSON quotes)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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
  /// parse input as a single-line Cirru expression (one-liner parser)
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat file input as a leaf node (for strings, use Cirru syntax: |text or "text)
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
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

/// splice all children of a node into its parent (inverse of wrap/rewrite)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "unwrap")]
pub struct TreeUnwrapCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node to unwrap (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// replace the parent node with this child node (Paredit raise-sexp)
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "raise")]
pub struct TreeRaiseCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the child node to raise (must have at least one element; its parent will be replaced)
  #[argh(option, short = 'p')]
  pub path: String,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

/// wrap the node at path inside a new expression, using `self` as placeholder for the original node
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "wrap")]
pub struct TreeWrapCommand {
  /// target in format "namespace/definition"
  #[argh(positional)]
  pub target: String,
  /// path to the node to wrap (comma-separated indices, e.g. "2,1,0")
  #[argh(option, short = 'p')]
  pub path: String,
  /// wrapping expression with `self` as placeholder for the original node (e.g. 'println self')
  #[argh(option, short = 'e', long = "code")]
  pub code: Option<String>,
  /// read wrapping expression from file (Cirru format by default, use -J for JSON)
  #[argh(option, short = 'f')]
  pub file: Option<String>,
  /// wrapping expression as inline JSON string
  #[argh(option, short = 'j')]
  pub json: Option<String>,
  /// treat file input as JSON
  #[argh(switch, short = 'J', long = "json-input")]
  pub json_input: bool,
  /// treat input as a Cirru leaf node
  #[argh(switch, long = "leaf")]
  pub leaf: bool,
  /// max depth for result preview (0 = unlimited, default 2)
  #[argh(option, short = 'd', default = "2")]
  pub depth: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Config command — top-level shortcut for configuration management
// ═══════════════════════════════════════════════════════════════════════════════

/// manage project configuration
#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "config")]
pub struct ConfigCommand {
  #[argh(subcommand)]
  pub subcommand: ConfigSubcommand,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand)]
pub enum ConfigSubcommand {
  /// show project configuration values and entries
  Show(ConfigShowCommand),
  /// list modules included in the project
  Modules(ConfigModulesCommand),
  /// show or bump the project version (omit value to show; use patch|minor|major to bump; or pass a semver string)
  Version(ConfigVersionCommand),
  /// set a configuration key to a value (init-fn, reload-fn, version)
  Set(ConfigSetCommand),
  /// add a module path to configs.modules
  AddModule(ConfigAddModuleCommand),
  /// remove a module path from configs.modules
  RmModule(ConfigRmModuleCommand),
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "show")]
/// show project configuration values and entries
pub struct ConfigShowCommand {
  /// show config for a named entry (e.g. "test") instead of the default configs
  #[argh(option)]
  pub entry: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "modules")]
/// list modules included in the project
pub struct ConfigModulesCommand {
  /// list modules for a named entry (e.g. "test") instead of the default configs
  #[argh(option)]
  pub entry: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "version")]
/// show or bump the project version
pub struct ConfigVersionCommand {
  /// patch | minor | major to bump, or a semver string to set explicitly; omit to show current version
  #[argh(positional)]
  pub value: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "set")]
/// set a configuration key (init-fn, reload-fn, version)
pub struct ConfigSetCommand {
  /// apply to a named entry (e.g. "test") instead of the default configs
  #[argh(option)]
  pub entry: Option<String>,
  /// config key: init-fn, reload-fn, version
  #[argh(positional)]
  pub key: String,
  /// config value; for "version" accepts semver string or patch|minor|major
  #[argh(positional)]
  pub value: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "add-module")]
/// add a module path to configs.modules
pub struct ConfigAddModuleCommand {
  /// add to a named entry (e.g. "test") instead of the default configs
  #[argh(option)]
  pub entry: Option<String>,
  /// module path to add (e.g. "calcit-test/")
  #[argh(positional)]
  pub module_path: String,
}

#[derive(FromArgs, PartialEq, Debug, Clone)]
#[argh(subcommand, name = "rm-module")]
/// remove a module path from configs.modules
pub struct ConfigRmModuleCommand {
  /// remove from a named entry (e.g. "test") instead of the default configs
  #[argh(option)]
  pub entry: Option<String>,
  /// module path to remove
  #[argh(positional)]
  pub module_path: String,
}

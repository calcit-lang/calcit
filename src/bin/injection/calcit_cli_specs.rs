//! Parameter specs for every `calcit.cli/*` builtin.

use super::calcit_cli_args::{CliArgDefault, CliArgKind, CliArgSpec, spec};

// ─── shared fields ───────────────────────────────────────────────────────────

pub const FILE_PATH: CliArgSpec = spec("file-path", CliArgKind::String, false, None);
pub const TARGET: CliArgSpec = spec("target", CliArgKind::String, true, None);
pub const NAMESPACE: CliArgSpec = spec("namespace", CliArgKind::String, true, None);
pub const PATH: CliArgSpec = spec("path", CliArgKind::String, true, None);
pub const CODE: CliArgSpec = spec("code", CliArgKind::String, true, None);
pub const CIRRU_CODE: CliArgSpec = spec("code", CliArgKind::CirruQuote, true, None);
pub const OPTIONAL_CIRRU_CODE: CliArgSpec = spec("code", CliArgKind::CirruQuote, false, None);
pub const KEYWORD: CliArgSpec = spec("keyword", CliArgKind::String, true, None);
pub const PATTERN: CliArgSpec = spec("pattern", CliArgKind::String, true, None);
pub const REPLACEMENT: CliArgSpec = spec("replacement", CliArgKind::String, true, None);
pub const SYMBOL: CliArgSpec = spec("symbol", CliArgKind::String, true, None);
pub const DOC: CliArgSpec = spec("doc", CliArgKind::String, true, None);
pub const TAG: CliArgSpec = spec("tag", CliArgKind::String, true, None);
pub const FILENAME: CliArgSpec = spec("filename", CliArgKind::String, true, None);
pub const TEXT_PATH: CliArgSpec = spec("path", CliArgKind::String, true, None);

pub const LINES: CliArgSpec = spec("lines", CliArgKind::Usize, false, Some(CliArgDefault::Usize(5)));
pub const MAX_LINES: CliArgSpec = spec("max-lines", CliArgKind::Usize, false, Some(CliArgDefault::Usize(80)));
pub const MAX_DEPTH: CliArgSpec = spec("max-depth", CliArgKind::Usize, false, Some(CliArgDefault::Usize(0)));
pub const INDEX: CliArgSpec = spec("index", CliArgKind::Usize, true, None);

pub const OVERWRITE: CliArgSpec = spec("overwrite", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const INCLUDE_CORE: CliArgSpec = spec("include-core", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const INCLUDE_DEPS: CliArgSpec = spec("include-deps", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const EXACT: CliArgSpec = spec("exact", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const JSON: CliArgSpec = spec("json", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const ONE_LINER: CliArgSpec = spec("one-liner", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const FULL: CliArgSpec = spec("full", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));
pub const SHOW_UNUSED: CliArgSpec = spec("show-unused", CliArgKind::Bool, false, Some(CliArgDefault::Bool(false)));

pub const OPTIONAL_PATH: CliArgSpec = spec("path", CliArgKind::String, false, None);
pub const OPTIONAL_ROOT: CliArgSpec = spec("root", CliArgKind::String, false, None);
pub const OPTIONAL_FILTER: CliArgSpec = spec("filter", CliArgKind::String, false, None);
pub const OPTIONAL_NS_PREFIX: CliArgSpec = spec("ns-prefix", CliArgKind::String, false, None);
pub const OPTIONAL_NS: CliArgSpec = spec("namespace", CliArgKind::String, false, None);
pub const OPTIONAL_ENTRY: CliArgSpec = spec("entry", CliArgKind::String, false, None);
pub const OPTIONAL_DOCS_DIR: CliArgSpec = spec("docs-dir", CliArgKind::String, false, None);
pub const OPTIONAL_HEADINGS: CliArgSpec = spec("headings", CliArgKind::String, false, None);
pub const OPTIONAL_TAG: CliArgSpec = spec("tag", CliArgKind::String, false, None);

pub const FORMAT_TEXT: CliArgSpec = spec("format", CliArgKind::String, false, Some(CliArgDefault::String("text")));
pub const SORT_COUNT: CliArgSpec = spec("sort", CliArgKind::String, false, Some(CliArgDefault::String("count")));
pub const DETAIL_SUMMARY: CliArgSpec = spec("detail", CliArgKind::String, false, Some(CliArgDefault::String("summary")));
pub const POSITION_AFTER: CliArgSpec = spec("position", CliArgKind::String, false, Some(CliArgDefault::String("after")));
pub const ERROR_FILE: CliArgSpec = spec(
  "error-file",
  CliArgKind::String,
  false,
  Some(CliArgDefault::String(".calcit-error.cirru")),
);

pub const FROM_PATH: CliArgSpec = spec("from-path", CliArgKind::String, true, None);
pub const TO_PATH: CliArgSpec = spec("to-path", CliArgKind::String, true, None);
pub const SOURCE: CliArgSpec = spec("source", CliArgKind::String, true, None);
pub const NEW_NAME: CliArgSpec = spec("new-name", CliArgKind::String, true, None);
pub const SOURCE_NS: CliArgSpec = spec("source-ns", CliArgKind::String, true, None);
pub const REFER_SYM: CliArgSpec = spec("refer-sym", CliArgKind::String, true, None);
pub const RULES_CODE: CliArgSpec = spec("rules-code", CliArgKind::String, true, None);
pub const SCHEMA_CODE: CliArgSpec = spec("schema-code", CliArgKind::CirruQuote, true, None);
pub const EXAMPLES_CODE: CliArgSpec = spec("examples-code", CliArgKind::String, true, None);
pub const TEMPLATE_CODE: CliArgSpec = spec("template-code", CliArgKind::CirruQuote, true, None);
pub const WRAPPER_CODE: CliArgSpec = spec("wrapper-code", CliArgKind::CirruQuote, true, None);
pub const REPLACEMENT_CODE: CliArgSpec = spec("replacement-code", CliArgKind::CirruQuote, true, None);
pub const REFS: CliArgSpec = spec("refs", CliArgKind::String, true, None);
pub const PATHS_LIST: CliArgSpec = spec("paths", CliArgKind::StringList, true, None);
pub const CONFIG_KEY: CliArgSpec = spec("key", CliArgKind::String, true, None);
pub const CONFIG_VALUE: CliArgSpec = spec("value", CliArgKind::String, true, None);
pub const MODULE_PATH: CliArgSpec = spec("module-path", CliArgKind::String, true, None);
pub const BUMP_KIND: CliArgSpec = spec("kind", CliArgKind::String, true, None);
pub const TAGS_CSV: CliArgSpec = spec("tags", CliArgKind::String, true, None);
pub const REGEX: CliArgSpec = spec("regex", CliArgKind::String, true, None);
pub const EDN: CliArgSpec = spec("edn", CliArgKind::String, true, None);
pub const JSON_INPUT: CliArgSpec = spec("json", CliArgKind::String, true, None);

pub const ONLY_LEVELS: CliArgSpec = spec("only-levels", CliArgKind::String, false, None);
pub const ONLY_KINDS: CliArgSpec = spec("only-kinds", CliArgKind::String, false, None);

// ─── per-function specs ──────────────────────────────────────────────────────

pub static LIST_NS: &[CliArgSpec] = &[FILE_PATH];
pub static LIST_DEFS: &[CliArgSpec] = &[FILE_PATH, NAMESPACE];
pub static SHOW_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static PEEK_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET, LINES];
pub static SEARCH_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET, KEYWORD];
pub static FIND_SYMBOL: &[CliArgSpec] = &[FILE_PATH, SYMBOL];
pub static SHOW_SCHEMA: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static LIST_EXAMPLES: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static LIST_USAGES: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static LIST_CONFIG: &[CliArgSpec] = &[FILE_PATH];
pub static LIST_MODULES: &[CliArgSpec] = &[FILE_PATH];
pub static TREE_SHOW: &[CliArgSpec] = &[FILE_PATH, TARGET, OPTIONAL_PATH, MAX_LINES];

pub static EDIT_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET, CIRRU_CODE, OVERWRITE];
pub static TREE_REPLACE: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH, CIRRU_CODE];
pub static SEARCH_REPLACE: &[CliArgSpec] = &[FILE_PATH, TARGET, PATTERN, REPLACEMENT];
pub static ADD_IMPORT: &[CliArgSpec] = &[FILE_PATH, NAMESPACE, SOURCE_NS, REFER_SYM];

pub static TREE_DELETE: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH];
pub static TREE_INSERT: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH, CIRRU_CODE, POSITION_AFTER];
pub static TREE_WRAP: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH, WRAPPER_CODE];
pub static TREE_UNWRAP: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH];
pub static TREE_RAISE: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH];
pub static TREE_CP: &[CliArgSpec] = &[FILE_PATH, TARGET, FROM_PATH, TO_PATH, POSITION_AFTER];
pub static TREE_MV: &[CliArgSpec] = &[FILE_PATH, TARGET, FROM_PATH, TO_PATH, POSITION_AFTER];
pub static RENAME_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET, NEW_NAME];
pub static RM_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static MV_DEF: &[CliArgSpec] = &[FILE_PATH, SOURCE, TARGET];
pub static SPLIT_DEF: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH, NEW_NAME];
pub static ADD_NS: &[CliArgSpec] = &[FILE_PATH, NAMESPACE, OPTIONAL_CIRRU_CODE];
pub static RM_IMPORT: &[CliArgSpec] = &[FILE_PATH, NAMESPACE, SOURCE_NS];
pub static EDIT_DOC: &[CliArgSpec] = &[FILE_PATH, TARGET, DOC];
pub static EDIT_SCHEMA: &[CliArgSpec] = &[FILE_PATH, TARGET, SCHEMA_CODE];
pub static ADD_EXAMPLE: &[CliArgSpec] = &[FILE_PATH, TARGET, CIRRU_CODE, spec("index", CliArgKind::Usize, false, None)];
pub static SHOW_ERROR: &[CliArgSpec] = &[ERROR_FILE];

pub static RM_NS: &[CliArgSpec] = &[FILE_PATH, NAMESPACE];
pub static SET_IMPORTS: &[CliArgSpec] = &[FILE_PATH, NAMESPACE, RULES_CODE];
pub static FORMAT_FILE: &[CliArgSpec] = &[FILE_PATH];
pub static SHOW_DOC: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static SHOW_NS_DOC: &[CliArgSpec] = &[FILE_PATH, NAMESPACE];
pub static EDIT_NS_DOC: &[CliArgSpec] = &[FILE_PATH, NAMESPACE, DOC];
pub static BUMP_VERSION: &[CliArgSpec] = &[FILE_PATH, BUMP_KIND];
pub static LIST_TAGS: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static SET_TAGS: &[CliArgSpec] = &[FILE_PATH, TARGET, TAGS_CSV];
pub static RM_EXAMPLE: &[CliArgSpec] = &[FILE_PATH, TARGET, INDEX];
pub static CLEAR_EXAMPLES: &[CliArgSpec] = &[FILE_PATH, TARGET];
pub static SET_EXAMPLES: &[CliArgSpec] = &[FILE_PATH, TARGET, EXAMPLES_CODE];
pub static TREE_REPLACE_LEAF: &[CliArgSpec] = &[FILE_PATH, TARGET, PATTERN, REPLACEMENT_CODE];
pub static TREE_REPLACE_LEAF_REGEX: &[CliArgSpec] = &[FILE_PATH, TARGET, REGEX, REPLACEMENT_CODE];
pub static TREE_SWAP_NEXT: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH];
pub static TREE_SWAP_PREV: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH];
pub static TREE_BATCH_DELETE: &[CliArgSpec] = &[FILE_PATH, TARGET, PATHS_LIST];
pub static TREE_REWRITE: &[CliArgSpec] = &[FILE_PATH, TARGET, PATH, TEMPLATE_CODE, REFS];
pub static SET_CONFIG: &[CliArgSpec] = &[FILE_PATH, CONFIG_KEY, CONFIG_VALUE, OPTIONAL_ENTRY];
pub static ADD_MODULE: &[CliArgSpec] = &[FILE_PATH, MODULE_PATH, OPTIONAL_ENTRY];
pub static RM_MODULE: &[CliArgSpec] = &[FILE_PATH, MODULE_PATH, OPTIONAL_ENTRY];
pub static CIRRU_PARSE: &[CliArgSpec] = &[CODE, ONE_LINER];
pub static CIRRU_FORMAT: &[CliArgSpec] = &[JSON_INPUT];
pub static READ_TEXT_FILE: &[CliArgSpec] = &[TEXT_PATH];
pub static CIRRU_PARSE_EDN: &[CliArgSpec] = &[EDN];
pub static CIRRU_SHOW_GUIDE: &[CliArgSpec] = &[];
pub static DOCS_SEARCH: &[CliArgSpec] = &[KEYWORD, OPTIONAL_DOCS_DIR];
pub static TRIGGER_INC: &[CliArgSpec] = &[
  FILE_PATH,
  spec("changed", CliArgKind::String, false, Some(CliArgDefault::String(""))),
  spec("added", CliArgKind::String, false, Some(CliArgDefault::String(""))),
  spec("removed", CliArgKind::String, false, Some(CliArgDefault::String(""))),
  spec("added-ns", CliArgKind::String, false, Some(CliArgDefault::String(""))),
  spec("removed-ns", CliArgKind::String, false, Some(CliArgDefault::String(""))),
  spec("ns-updated", CliArgKind::String, false, Some(CliArgDefault::String(""))),
];

pub static ANALYZE_CALL_GRAPH: &[CliArgSpec] = &[
  FILE_PATH,
  OPTIONAL_ROOT,
  FORMAT_TEXT,
  MAX_DEPTH,
  INCLUDE_CORE,
  OPTIONAL_NS_PREFIX,
  SHOW_UNUSED,
];
pub static ANALYZE_EFFECTS_GRAPH: &[CliArgSpec] = &[
  FILE_PATH,
  OPTIONAL_ROOT,
  FORMAT_TEXT,
  MAX_DEPTH,
  INCLUDE_CORE,
  OPTIONAL_NS_PREFIX,
  DETAIL_SUMMARY,
];
pub static ANALYZE_COUNT_CALLS: &[CliArgSpec] = &[FILE_PATH, OPTIONAL_ROOT, FORMAT_TEXT, INCLUDE_CORE, OPTIONAL_NS_PREFIX, SORT_COUNT];
pub static ANALYZE_CHECK_TYPES: &[CliArgSpec] = &[FILE_PATH, OPTIONAL_NS, OPTIONAL_NS_PREFIX, ONLY_LEVELS, INCLUDE_DEPS];
pub static ANALYZE_WEAK_TYPES: &[CliArgSpec] = &[FILE_PATH, OPTIONAL_NS, OPTIONAL_NS_PREFIX, ONLY_KINDS, INCLUDE_DEPS];

pub static SHOW_PKG: &[CliArgSpec] = &[FILE_PATH];
pub static SHOW_NS: &[CliArgSpec] = &[FILE_PATH, NAMESPACE];
pub static LIST_DEFS_BY_TAG: &[CliArgSpec] = &[FILE_PATH, TAG, OPTIONAL_NS];
pub static VALIDATE_FILE: &[CliArgSpec] = &[FILE_PATH];
pub static SEARCH_PROJECT: &[CliArgSpec] = &[FILE_PATH, PATTERN, OPTIONAL_FILTER, EXACT, MAX_DEPTH];
pub static SEARCH_DEF_REGEX: &[CliArgSpec] = &[FILE_PATH, TARGET, REGEX];
pub static SEARCH_EXPR: &[CliArgSpec] = &[FILE_PATH, PATTERN, OPTIONAL_FILTER, JSON, EXACT, MAX_DEPTH];
pub static LIST_HOST_PROCS: &[CliArgSpec] = &[OPTIONAL_TAG];

pub static DOCS_AGENTS: &[CliArgSpec] = &[OPTIONAL_HEADINGS, FULL];
pub static DOCS_READ: &[CliArgSpec] = &[FILENAME, OPTIONAL_HEADINGS, FULL];
pub static DOCS_SECTIONS: &[CliArgSpec] = &[FILENAME];

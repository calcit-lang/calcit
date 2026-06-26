use calcit::cli_args::*;
use calcit::data::cirru::code_to_calcit;
use cirru_parser::Cirru;
use colored::Colorize;

macro_rules! echo_items {
  ($tokens:expr $(,)?) => {};
  ($tokens:expr, pos $name:literal => $value:expr $(, $($rest:tt)*)?) => {{
    push_positional($tokens, $name, $value);
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, switch $name:literal => $value:expr $(, $($rest:tt)*)?) => {{
    push_switch($tokens, $name, $value);
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, value $name:literal => $value:expr ; default $default:expr $(, $($rest:tt)*)?) => {{
    let value = ($value).to_string();
    push_value($tokens, $name, &value, Some($default));
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, value $name:literal => $value:expr $(, $($rest:tt)*)?) => {{
    let value = ($value).to_string();
    push_value($tokens, $name, &value, None);
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, opt $name:literal => $value:expr ; default $default:expr $(, $($rest:tt)*)?) => {{
    push_optional($tokens, $name, $value, $default);
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, opt_owned $name:literal => $value:expr ; default $default:expr $(, $($rest:tt)*)?) => {{
    push_optional_owned($tokens, $name, $value, $default);
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, list $name:literal => $value:expr $(, $($rest:tt)*)?) => {{
    push_list($tokens, $name, $value);
    echo_items!($tokens $(, $($rest)*)?);
  }};
  ($tokens:expr, code_input $opts:expr $(, $($rest:tt)*)?) => {{
    let opts = &$opts;
    push_code_input(
      $tokens,
      &CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      ),
    );
    echo_items!($tokens $(, $($rest)*)?);
  }};
}

pub fn should_echo_command(cli_args: &ToplevelCalcit) -> bool {
  matches!(
    cli_args.subcommand,
    Some(CalcitCommand::Query(_))
      | Some(CalcitCommand::Docs(_))
      | Some(CalcitCommand::Libs(_))
      | Some(CalcitCommand::Edit(_))
      | Some(CalcitCommand::Tree(_))
      | Some(CalcitCommand::Config(_))
      | Some(CalcitCommand::Analyze(_))
      | Some(CalcitCommand::Cirru(_))
  )
}

pub fn print_command_echo(cli_args: &ToplevelCalcit) {
  let Some(command) = render_command_echo(cli_args) else {
    return;
  };

  eprintln!("{}", format!("Command: {command}").blue().bold());

  if let Some(explanation) = render_command_explanation(cli_args) {
    eprintln!("{}", format!("# Explanation: {explanation}").blue().bold());
  }
}

fn render_command_echo(cli_args: &ToplevelCalcit) -> Option<String> {
  let subcommand = cli_args.subcommand.as_ref()?;
  let mut tokens = vec![match subcommand {
    CalcitCommand::Query(cmd) => format!("cr query {}", query_name(&cmd.subcommand)),
    CalcitCommand::Docs(cmd) => match &cmd.subcommand {
      DocsSubcommand::RemoteLibs(opts) => match opts.subcommand.as_ref() {
        Some(subcommand) => format!("cr docs remote-libs {}", libs_name(subcommand)),
        None => "cr docs remote-libs".to_string(),
      },
      other => format!("cr docs {}", docs_name(other)),
    },
    CalcitCommand::Libs(cmd) => format!("cr libs {}", libs_name(cmd.subcommand.as_ref()?)),
    CalcitCommand::Edit(cmd) => format!("cr edit {}", edit_name(&cmd.subcommand)),
    CalcitCommand::Tree(cmd) => format!("cr tree {}", tree_name(&cmd.subcommand)),
    CalcitCommand::Config(cmd) => format!("cr config {}", config_name(&cmd.subcommand)),
    CalcitCommand::Analyze(cmd) => format!("cr analyze {}", analyze_name(&cmd.subcommand)),
    CalcitCommand::Cirru(cmd) => format!("cr cirru {}", cirru_name(&cmd.subcommand)),
    _ => return None,
  }];

  match subcommand {
    CalcitCommand::Query(cmd) => push_query(&mut tokens, cmd),
    CalcitCommand::Docs(cmd) => push_docs(&mut tokens, cmd),
    CalcitCommand::Libs(cmd) => push_libs(&mut tokens, cmd.subcommand.as_ref()?),
    CalcitCommand::Edit(cmd) => push_edit(&mut tokens, cmd),
    CalcitCommand::Tree(cmd) => push_tree(&mut tokens, cmd),
    CalcitCommand::Config(cmd) => push_config(&mut tokens, cmd),
    CalcitCommand::Analyze(cmd) => push_analyze(&mut tokens, cmd),
    CalcitCommand::Cirru(cmd) => push_cirru(&mut tokens, cmd),
    _ => return None,
  }

  Some(tokens.join(" "))
}

fn render_command_explanation(cli_args: &ToplevelCalcit) -> Option<String> {
  let subcommand = cli_args.subcommand.as_ref()?;
  match subcommand {
    CalcitCommand::Query(cmd) => render_query_explanation(cmd),
    CalcitCommand::Edit(cmd) => render_edit_explanation(cmd),
    CalcitCommand::Tree(cmd) => render_tree_explanation(cmd),
    CalcitCommand::Docs(cmd) => render_docs_explanation(cmd),
    CalcitCommand::Analyze(cmd) => render_analyze_explanation(cmd),
    CalcitCommand::Cirru(cmd) => render_cirru_explanation(cmd),
    _ => None,
  }
}

fn describe_leaf_calcit_type(code: &str) -> String {
  let leaf = Cirru::leaf(code);
  match code_to_calcit(&leaf, "@echo", "@echo", vec![]) {
    Ok(calcit) => calcit.describe_type(),
    Err(e) => format!("*invalid Cirru: {e}*"),
  }
}

fn describe_code_input(parts: &CodeInputParts<'_>) -> String {
  let mut desc = String::new();
  if let Some(code) = parts.code {
    if parts.leaf {
      desc.push_str(&format!("--code value is {} (via --leaf). ", describe_leaf_calcit_type(code)));
    } else if parts.json_input {
      desc.push_str("--code value is a **JSON-encoded AST** (with --json-input). ");
    } else {
      desc.push_str("--code is a **Cirru expression snippet**. ");
    }
  }
  if let Some(_file) = parts.file {
    desc.push_str("--file provides Cirru source from a file. ");
  }
  if let Some(json) = parts.json {
    desc.push_str(&format!("--json provides **JSON-encoded AST**: `{json}`. "));
  }
  desc
}

fn render_query_explanation(cmd: &QueryCommand) -> Option<String> {
  Some(match &cmd.subcommand {
    QuerySubcommand::Search(opts) => {
      let mut desc = format!("searches for `{}` in AST leaf nodes", opts.pattern);
      if let Some(filter) = &opts.filter {
        desc.push_str(&format!(", filtered by definitions matching `{filter}`"));
      }
      if opts.exact {
        desc.push_str(" (exact match)");
      } else if !opts.regex {
        desc.push_str(" (case-insensitive contains, default)");
      }
      if opts.regex {
        desc.push_str(" (regex match)");
      }
      if opts.max_depth > 0 {
        desc.push_str(&format!(", max tree depth={}", opts.max_depth));
      }
      desc
    }
    QuerySubcommand::SearchExpr(opts) => {
      let mut desc = format!("searches for Cirru expression `{}` in definitions", opts.pattern);
      if let Some(filter) = &opts.filter {
        desc.push_str(&format!(", filtered by `{filter}`"));
      }
      if opts.json {
        desc.push_str(" (JSON output)");
      }
      desc
    }
    QuerySubcommand::Find(opts) => {
      let mut desc = format!("finds all references to symbol `{}`", opts.symbol);
      if opts.deps {
        desc.push_str(" (including dependencies)");
      }
      if opts.exact {
        desc.push_str(" (exact match)");
      }
      desc
    }
    QuerySubcommand::Def(opts) => {
      let mut desc = format!("displays definition `{}` (namespace/definition format)", opts.target);
      if opts.json {
        desc.push_str(" (JSON output)");
      }
      if opts.raw {
        desc.push_str(" (raw, no chunking)");
      }
      desc
    }
    QuerySubcommand::Peek(opts) => format!("previews definition `{}`", opts.target),
    QuerySubcommand::Examples(opts) => format!("shows usage examples for `{}`", opts.target),
    QuerySubcommand::Schema(opts) => format!("shows type schema for `{}`", opts.target),
    QuerySubcommand::Usages(opts) => {
      let mut desc = format!("finds all usages of `{}` in codebase", opts.target);
      if opts.deps {
        desc.push_str(" (including dependencies)");
      }
      desc
    }
    QuerySubcommand::Ns(opts) => {
      let mut desc = "lists all namespaces".to_string();
      if let Some(ns) = &opts.namespace {
        desc.push_str(&format!(" matching `{ns}`"));
      }
      if opts.deps {
        desc.push_str(" (including dependency namespaces)");
      }
      desc
    }
    QuerySubcommand::Defs(opts) => {
      let mut desc = format!("lists all definitions in namespace `{}`", opts.namespace);
      if let Some(tag) = &opts.tag {
        desc.push_str(&format!(" filtered by tag `{tag}`"));
      }
      desc
    }
    QuerySubcommand::Pkg(_) => "shows package metadata".to_string(),
    QuerySubcommand::Config(_) => "shows project configuration".to_string(),
    QuerySubcommand::Error(_) => "shows recent build errors".to_string(),
    QuerySubcommand::Modules(_) => "lists loaded modules".to_string(),
    QuerySubcommand::HostProcs(opts) => {
      let mut desc = "lists host procedures (FFI)".to_string();
      if let Some(tag) = &opts.tag {
        desc.push_str(&format!(" filtered by tag `{tag}`"));
      }
      desc
    }
  })
}

fn render_edit_explanation(cmd: &EditCommand) -> Option<String> {
  Some(match &cmd.subcommand {
    EditSubcommand::Format(_) => "rewrites snapshot file in canonical format".to_string(),
    EditSubcommand::Def(opts) => {
      let desc = format!("adds/updates definition `{}`", opts.target);
      format!(
        "{}; {}",
        desc,
        describe_code_input(&CodeInputParts::new(
          opts.file.as_deref(),
          opts.code.as_deref(),
          opts.json.as_deref(),
          opts.json_input,
          opts.leaf
        ))
      )
    }
    EditSubcommand::MvDef(opts) => format!("moves/renames definition from `{}` to `{}`", opts.source, opts.target),
    EditSubcommand::RmDef(opts) => format!("deletes definition `{}`", opts.target),
    EditSubcommand::Doc(opts) => format!("updates documentation for `{}`", opts.target),
    EditSubcommand::Schema(opts) => {
      let mut desc = format!("updates type schema for `{}`", opts.target);
      if opts.clear {
        desc.push_str(" (clears existing schema)");
      }
      desc
    }
    EditSubcommand::Examples(opts) => {
      let mut desc = format!("sets usage examples for `{}`", opts.target);
      if opts.clear {
        desc.push_str(" (clears existing examples)");
      }
      desc
    }
    EditSubcommand::AddExample(opts) => format!(
      "adds example to `{}`{}",
      opts.target,
      opts.at.as_ref().map_or(String::new(), |at| format!(" at position `{at}`"))
    ),
    EditSubcommand::RmExample(opts) => format!("removes example at index {} from `{}`", opts.index, opts.target),
    EditSubcommand::Tags(opts) => {
      let mut desc = format!("views/updates tags for `{}`", opts.target);
      if let Some(tags) = &opts.tags {
        desc.push_str(&format!(" to `{tags}`"));
      }
      desc
    }
    EditSubcommand::AddNs(opts) => {
      let desc = format!("adds namespace `{}`", opts.namespace);
      format!(
        "{}; {}",
        desc,
        describe_code_input(&CodeInputParts::new(
          opts.file.as_deref(),
          opts.code.as_deref(),
          opts.json.as_deref(),
          opts.json_input,
          opts.leaf
        ))
      )
    }
    EditSubcommand::RmNs(opts) => format!("deletes namespace `{}`", opts.namespace),
    EditSubcommand::Imports(opts) => {
      let desc = format!("replaces all imports in namespace `{}`", opts.namespace);
      format!(
        "{}; {}",
        desc,
        describe_code_input(&CodeInputParts::new(
          opts.file.as_deref(),
          opts.code.as_deref(),
          opts.json.as_deref(),
          opts.json_input,
          opts.leaf
        ))
      )
    }
    EditSubcommand::AddImport(opts) => {
      let desc = format!("adds import rule to namespace `{}`", opts.namespace);
      format!(
        "{}; {} {}",
        desc,
        describe_code_input(&CodeInputParts::new(
          opts.file.as_deref(),
          opts.code.as_deref(),
          opts.json.as_deref(),
          opts.json_input,
          opts.leaf
        )),
        if opts.overwrite { "(overwrites existing)" } else { "" }
      )
    }
    EditSubcommand::RmImport(opts) => format!("removes import `{}` from namespace `{}`", opts.source_ns, opts.namespace),
    EditSubcommand::NsDoc(opts) => format!("updates documentation for namespace `{}`", opts.namespace),
    EditSubcommand::Inc(_) => "describes incremental code changes (added/removed/changed defs)".to_string(),
    EditSubcommand::Cp(opts) => format!(
      "copies AST node from path `{}` to `{}` in `{}` (position: {})",
      opts.from, opts.path, opts.target, opts.at
    ),
    EditSubcommand::Mv(opts) => format!(
      "moves AST node from path `{}` to `{}` in `{}` (position: {})",
      opts.from, opts.path, opts.target, opts.at
    ),
    EditSubcommand::Rename(opts) => format!("renames `{}` to `{}` (same namespace)", opts.source, opts.new_name),
    EditSubcommand::SplitDef(opts) => format!(
      "extracts sub-expression at path `{}` from `{}` into new definition `{}`",
      opts.path, opts.target, opts.new_name
    ),
  })
}

fn render_tree_explanation(cmd: &TreeCommand) -> Option<String> {
  Some(match &cmd.subcommand {
    TreeSubcommand::Show(opts) => {
      let mut desc = format!("displays AST subtree of `{}`", opts.target);
      if let Some(path) = &opts.path {
        desc.push_str(&format!(" at path `{path}`"));
      }
      if opts.json {
        desc.push_str(" (JSON output)");
      }
      if opts.raw {
        desc.push_str(" (raw, no chunking)");
      }
      desc
    }
    TreeSubcommand::Replace(opts) => {
      let code_parts = CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      );
      let mut desc = format!("replaces AST node at path `{}` in `{}`", opts.path, opts.target);
      let code_desc = describe_code_input(&code_parts);
      if !code_desc.is_empty() {
        desc.push_str(&format!("; {code_desc}"));
      }
      desc
    }
    TreeSubcommand::ReplaceLeaf(opts) => {
      let code_parts = CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      );
      let mode = if opts.regex { "regex match" } else { "exact match" };
      let mut desc = format!("replaces all leaf nodes matching `{}` in `{}` ({mode})", opts.pattern, opts.target);
      let code_desc = describe_code_input(&code_parts);
      if !code_desc.is_empty() {
        desc.push_str(&format!("; {code_desc}"));
      }
      desc
    }
    TreeSubcommand::SearchReplace(opts) => {
      let code_parts = CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      );
      let mut desc = format!("finds unique leaf `{}` in `{}` and replaces it", opts.pattern, opts.target);
      let code_desc = describe_code_input(&code_parts);
      if !code_desc.is_empty() {
        desc.push_str(&format!("; {code_desc}"));
      }
      desc
    }
    TreeSubcommand::Delete(opts) => format!("removes AST node at path `{}` from `{}`", opts.path, opts.target),
    TreeSubcommand::InsertBefore(opts) => format!("inserts code before path `{}` in `{}`", opts.path, opts.target),
    TreeSubcommand::InsertAfter(opts) => format!("inserts code after path `{}` in `{}`", opts.path, opts.target),
    TreeSubcommand::InsertChild(opts) => format!("inserts child node at path `{}` in `{}`", opts.path, opts.target),
    TreeSubcommand::AppendChild(opts) => format!("appends child node at path `{}` in `{}`", opts.path, opts.target),
    TreeSubcommand::SwapNext(opts) => format!("swaps node at path `{}` with its next sibling in `{}`", opts.path, opts.target),
    TreeSubcommand::SwapPrev(opts) => format!("swaps node at path `{}` with its previous sibling in `{}`", opts.path, opts.target),
    TreeSubcommand::Unwrap(opts) => format!(
      "removes list wrapping at path `{}` in `{}`, promotes children up",
      opts.path, opts.target
    ),
    TreeSubcommand::Raise(opts) => format!("raises node at path `{}` one level up in `{}`", opts.path, opts.target),
    TreeSubcommand::Wrap(opts) => {
      let code_parts = CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      );
      let mut desc = format!("wraps node at path `{}` in a new list in `{}`", opts.path, opts.target);
      let code_desc = describe_code_input(&code_parts);
      if !code_desc.is_empty() {
        desc.push_str(&format!("; {code_desc}"));
      }
      desc
    }
    TreeSubcommand::Rewrite(opts) => {
      let mut desc = format!("structural rewrite at path `{}` in `{}`", opts.path, opts.target);
      if !opts.with.is_empty() {
        desc.push_str(&format!("; binds: {}", opts.with.join(", ")));
      }
      desc
    }
    TreeSubcommand::BatchDelete(opts) => format!(
      "deletes {} path(s) from `{}` (sorted highest-to-lowest)",
      opts.paths.len(),
      opts.target
    ),
  })
}

fn render_docs_explanation(cmd: &DocsCommand) -> Option<String> {
  Some(match &cmd.subcommand {
    DocsSubcommand::Scopes(_) => "lists available documentation scopes".to_string(),
    DocsSubcommand::RemoteLibs(_) => "searches remote library documentation".to_string(),
    DocsSubcommand::Search(opts) => format!("searches documentation for `{}`", opts.keyword),
    DocsSubcommand::List(opts) => {
      let mut desc = "lists available documentation files".to_string();
      if let Some(module) = &opts.module {
        desc.push_str(&format!(" in module `{module}`"));
      }
      desc
    }
    DocsSubcommand::Sections(opts) => format!("shows sections in `{}`", opts.filename),
    DocsSubcommand::Read(opts) => format!("reads documentation from `{}`", opts.filename),
    DocsSubcommand::Agents(_) => "reads agent/developer guide".to_string(),
    DocsSubcommand::ReadLines(opts) => format!("reads specific lines from `{}`", opts.filename),
    DocsSubcommand::CheckMd(opts) => format!("validates code snippets in `{}`", opts.file),
  })
}

fn render_analyze_explanation(cmd: &AnalyzeCommand) -> Option<String> {
  Some(match &cmd.subcommand {
    AnalyzeSubcommand::CallGraph(opts) => {
      let mut desc = "analyzes function call graph".to_string();
      if let Some(root) = &opts.root {
        desc.push_str(&format!(" from root `{root}`"));
      }
      if let Some(prefix) = &opts.ns_prefix {
        desc.push_str(&format!(", namespace prefix `{prefix}`"));
      }
      desc
    }
    AnalyzeSubcommand::CallGraphDiff(opts) => format!("compares call graph changes since `{}`", opts.git_ref),
    AnalyzeSubcommand::CountCalls(opts) => {
      let mut desc = "counts function call frequencies".to_string();
      if let Some(root) = &opts.root {
        desc.push_str(&format!(" from root `{root}`"));
      }
      desc
    }
    AnalyzeSubcommand::ProgramDiff(opts) => format!("compares AST structure since `{}`", opts.git_ref),
    AnalyzeSubcommand::CheckExamples(_) => "validates all code examples in definitions".to_string(),
    AnalyzeSubcommand::CheckTypes(opts) => {
      let mut desc = "performs type checking".to_string();
      if let Some(ns) = &opts.ns {
        desc.push_str(&format!(" in namespace `{ns}`"));
      }
      desc
    }
    AnalyzeSubcommand::WeakTypes(opts) => {
      let mut desc = "analyzes weak/untyped definitions".to_string();
      if let Some(ns) = &opts.ns {
        desc.push_str(&format!(" in namespace `{ns}`"));
      }
      desc
    }
    AnalyzeSubcommand::EffectsGraph(opts) => {
      let mut desc = "builds effects dependency graph".to_string();
      if let Some(root) = &opts.root {
        desc.push_str(&format!(" from root `{root}`"));
      }
      desc
    }
    AnalyzeSubcommand::JsEscape(opts) => format!("escapes Calcit symbol `{}` to JS-safe name", opts.symbol),
    AnalyzeSubcommand::JsUnescape(opts) => format!("unescapes JS name back to Calcit symbol `{}`", opts.symbol),
  })
}

fn render_cirru_explanation(cmd: &CirruCommand) -> Option<String> {
  Some(match &cmd.subcommand {
    CirruSubcommand::Parse(opts) => format!("parses Cirru code `{}` into AST", opts.code),
    CirruSubcommand::Format(opts) => format!("formats JSON-encoded Cirru AST: `{}`", opts.json),
    CirruSubcommand::ParseEdn(opts) => format!("parses EDN data: `{}`", opts.edn),
    CirruSubcommand::ShowGuide(_) => "displays Cirru language syntax guide".to_string(),
  })
}

fn push_query(tokens: &mut Vec<String>, cmd: &QueryCommand) {
  match &cmd.subcommand {
    QuerySubcommand::Schema(opts) => echo_items!(tokens, pos "target" => &opts.target, switch "json" => opts.json),
    QuerySubcommand::Ns(opts) => {
      echo_items!(tokens, opt "namespace" => opts.namespace.as_deref(); default "all", switch "deps" => opts.deps)
    }
    QuerySubcommand::Defs(opts) => {
      echo_items!(tokens, pos "namespace" => &opts.namespace, opt "tag" => opts.tag.as_deref(); default "none")
    }
    QuerySubcommand::Pkg(_) | QuerySubcommand::Config(_) | QuerySubcommand::Error(_) | QuerySubcommand::Modules(_) => {}
    QuerySubcommand::Def(opts) => echo_items!(
      tokens,
      pos "target" => &opts.target,
      switch "json" => opts.json,
      value "chunk-target-nodes" => opts.chunk_target_nodes; default "56",
      value "chunk-max-nodes" => opts.chunk_max_nodes; default "68",
      value "chunk-trigger-nodes" => opts.chunk_trigger_nodes; default "88",
      switch "raw" => opts.raw
    ),
    QuerySubcommand::Peek(opts) => echo_items!(tokens, pos "target" => &opts.target),
    QuerySubcommand::Examples(opts) => echo_items!(tokens, pos "target" => &opts.target),
    QuerySubcommand::Find(opts) => echo_items!(
      tokens,
      pos "symbol" => &opts.symbol,
      switch "deps" => opts.deps,
      switch "exact" => opts.exact,
      value "limit" => opts.limit; default "20",
      value "detail-offset" => opts.detail_offset; default "0"
    ),
    QuerySubcommand::Usages(opts) => echo_items!(
      tokens,
      pos "target" => &opts.target,
      switch "deps" => opts.deps,
      value "detail-offset" => opts.detail_offset; default "0"
    ),
    QuerySubcommand::Search(opts) => echo_items!(
      tokens,
      pos "pattern" => &opts.pattern,
      opt "filter" => opts.filter.as_deref(); default "none",
      switch "exact" => opts.exact,
      switch "regex" => opts.regex,
      value "max-depth" => opts.max_depth; default "0",
      opt "start-path" => opts.start_path.as_deref(); default "none",
      opt "entry" => opts.entry.as_deref(); default "none",
      value "detail-offset" => opts.detail_offset; default "0"
    ),
    QuerySubcommand::SearchExpr(opts) => echo_items!(
      tokens,
      pos "pattern" => &opts.pattern,
      opt "filter" => opts.filter.as_deref(); default "none",
      switch "exact" => opts.exact,
      value "max-depth" => opts.max_depth; default "0",
      switch "json" => opts.json,
      opt "entry" => opts.entry.as_deref(); default "none",
      value "detail-offset" => opts.detail_offset; default "0"
    ),
    QuerySubcommand::HostProcs(opts) => {
      echo_items!(tokens, opt "tag" => opts.tag.as_deref(); default "none")
    }
  }
}

fn push_docs(tokens: &mut Vec<String>, cmd: &DocsCommand) {
  match &cmd.subcommand {
    DocsSubcommand::Scopes(_) => {}
    DocsSubcommand::RemoteLibs(opts) => {
      if let Some(subcommand) = opts.subcommand.as_ref() {
        push_libs(tokens, subcommand);
      }
    }
    DocsSubcommand::Search(opts) => echo_items!(
      tokens,
      pos "keyword" => &opts.keyword,
      value "context" => opts.context; default "5",
      opt "filename" => opts.filename.as_deref(); default "none",
      opt "module" => opts.module.as_deref(); default "none"
    ),
    DocsSubcommand::List(opts) => echo_items!(tokens, opt "module" => opts.module.as_deref(); default "none"),
    DocsSubcommand::Sections(opts) => echo_items!(
      tokens,
      pos "filename" => &opts.filename,
      opt "module" => opts.module.as_deref(); default "none",
      switch "with-lines" => opts.with_lines
    ),
    DocsSubcommand::Read(opts) => echo_items!(
      tokens,
      pos "filename" => &opts.filename,
      list "heading" => &opts.headings,
      switch "no-subheadings" => opts.no_subheadings,
      switch "full" => opts.full,
      switch "with-lines" => opts.with_lines,
      opt "module" => opts.module.as_deref(); default "none"
    ),
    DocsSubcommand::Agents(opts) => echo_items!(
      tokens,
      list "heading" => &opts.headings,
      switch "no-subheadings" => opts.no_subheadings,
      switch "full" => opts.full,
      switch "with-lines" => opts.with_lines,
      switch "refresh" => opts.refresh
    ),
    DocsSubcommand::ReadLines(opts) => echo_items!(
      tokens,
      pos "filename" => &opts.filename,
      value "start" => opts.start; default "0",
      value "lines" => opts.lines; default "80",
      opt "module" => opts.module.as_deref(); default "none"
    ),
    DocsSubcommand::CheckMd(opts) => {
      echo_items!(tokens, pos "file" => &opts.file, value "entry" => &opts.entry; default "calcit.cirru", list "dep" => &opts.dep)
    }
  }
}

fn push_libs(tokens: &mut Vec<String>, subcommand: &LibsSubcommand) {
  match subcommand {
    LibsSubcommand::Readme(opts) => echo_items!(
      tokens,
      pos "package" => &opts.package,
      list "heading" => &opts.headings,
      opt "file" => opts.file.as_deref(); default "none",
      switch "no-subheadings" => opts.no_subheadings,
      switch "full" => opts.full,
      switch "with-lines" => opts.with_lines
    ),
    LibsSubcommand::Search(opts) => echo_items!(tokens, pos "keyword" => &opts.keyword),
    LibsSubcommand::ScanMd(opts) => echo_items!(tokens, pos "module" => &opts.module),
  }
}

fn push_cirru(tokens: &mut Vec<String>, cmd: &CirruCommand) {
  match &cmd.subcommand {
    CirruSubcommand::Parse(opts) => {
      echo_items!(tokens, pos "code" => &opts.code, switch "expr-one" => opts.expr_one_liner, switch "validate" => opts.validate)
    }
    CirruSubcommand::Format(opts) => echo_items!(tokens, pos "json" => &opts.json),
    CirruSubcommand::ParseEdn(opts) => echo_items!(tokens, pos "edn" => &opts.edn),
    CirruSubcommand::ShowGuide(_) => {}
  }
}

fn push_analyze(tokens: &mut Vec<String>, cmd: &AnalyzeCommand) {
  match &cmd.subcommand {
    AnalyzeSubcommand::CallGraph(opts) => echo_items!(
      tokens,
      opt "root" => opts.root.as_deref(); default "config.init-fn",
      opt "ns-prefix" => opts.ns_prefix.as_deref(); default "none",
      switch "include-core" => opts.include_core,
      value "max-depth" => opts.max_depth; default "0",
      switch "show-unused" => opts.show_unused,
      value "format" => &opts.format; default "text"
    ),
    AnalyzeSubcommand::CallGraphDiff(opts) => echo_items!(
      tokens,
      pos "git-ref" => &opts.git_ref,
      opt "root" => opts.root.as_deref(); default "config.init-fn",
      opt "ns-prefix" => opts.ns_prefix.as_deref(); default "none",
      switch "include-core" => opts.include_core,
      value "max-depth" => opts.max_depth; default "0"
    ),
    AnalyzeSubcommand::CountCalls(opts) => echo_items!(
      tokens,
      opt "root" => opts.root.as_deref(); default "config.init-fn",
      opt "ns-prefix" => opts.ns_prefix.as_deref(); default "none",
      switch "include-core" => opts.include_core,
      value "format" => &opts.format; default "text",
      value "sort" => &opts.sort; default "count"
    ),
    AnalyzeSubcommand::ProgramDiff(opts) => echo_items!(tokens, pos "git-ref" => &opts.git_ref,
      opt "def" => opts.def.as_deref(); default "none"
    ),
    AnalyzeSubcommand::CheckExamples(opts) => echo_items!(tokens, value "ns" => &opts.ns),
    AnalyzeSubcommand::CheckTypes(opts) => echo_items!(
      tokens,
      opt "ns" => opts.ns.as_deref(); default "none",
      opt "ns-prefix" => opts.ns_prefix.as_deref(); default "none",
      opt "only" => opts.only.as_deref(); default "all",
      switch "deps" => opts.deps
    ),
    AnalyzeSubcommand::WeakTypes(opts) => echo_items!(
      tokens,
      opt "ns" => opts.ns.as_deref(); default "none",
      opt "ns-prefix" => opts.ns_prefix.as_deref(); default "none",
      opt "only" => opts.only.as_deref(); default "all",
      switch "deps" => opts.deps
    ),
    AnalyzeSubcommand::EffectsGraph(opts) => echo_items!(
      tokens,
      opt "root" => opts.root.as_deref(); default "config.init-fn",
      opt "ns-prefix" => opts.ns_prefix.as_deref(); default "none",
      switch "include-core" => opts.include_core,
      value "max-depth" => opts.max_depth; default "2",
      value "format" => &opts.format; default "tree",
      value "detail" => &opts.detail; default "summary",
      value "color" => opts.color; default "true"
    ),
    AnalyzeSubcommand::JsEscape(opts) => echo_items!(tokens, pos "symbol" => &opts.symbol),
    AnalyzeSubcommand::JsUnescape(opts) => echo_items!(tokens, pos "symbol" => &opts.symbol),
  }
}

fn push_edit(tokens: &mut Vec<String>, cmd: &EditCommand) {
  match &cmd.subcommand {
    EditSubcommand::Format(_) => {}
    EditSubcommand::Def(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, code_input opts, switch "overwrite" => opts.overwrite)
    }
    EditSubcommand::MvDef(opts) => echo_items!(tokens, pos "source" => &opts.source, pos "target" => &opts.target),
    EditSubcommand::RmDef(opts) => echo_items!(tokens, pos "target" => &opts.target),
    EditSubcommand::Doc(opts) => echo_items!(tokens, pos "target" => &opts.target, pos "doc" => &opts.doc),
    EditSubcommand::Schema(opts) => echo_items!(tokens, pos "target" => &opts.target, code_input opts, switch "clear" => opts.clear),
    EditSubcommand::Examples(opts) => echo_items!(tokens, pos "target" => &opts.target, code_input opts, switch "clear" => opts.clear),
    EditSubcommand::AddExample(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, opt_owned "at" => opts.at.map(|v| v.to_string()); default "append", code_input opts)
    }
    EditSubcommand::RmExample(opts) => echo_items!(tokens, pos "target" => &opts.target, pos "index" => &opts.index.to_string()),
    EditSubcommand::Tags(opts) => {
      echo_items!(
        tokens,
        pos "target" => &opts.target,
        opt "tags" => opts.tags.as_deref(); default "none"
      )
    }
    EditSubcommand::AddNs(opts) => echo_items!(tokens, pos "namespace" => &opts.namespace, code_input opts),
    EditSubcommand::RmNs(opts) => echo_items!(tokens, pos "namespace" => &opts.namespace),
    EditSubcommand::Imports(opts) => echo_items!(tokens, pos "namespace" => &opts.namespace, code_input opts),
    EditSubcommand::AddImport(opts) => {
      echo_items!(tokens, pos "namespace" => &opts.namespace, code_input opts, switch "overwrite" => opts.overwrite)
    }
    EditSubcommand::RmImport(opts) => echo_items!(tokens, pos "namespace" => &opts.namespace, pos "source-ns" => &opts.source_ns),
    EditSubcommand::NsDoc(opts) => echo_items!(tokens, pos "namespace" => &opts.namespace, pos "doc" => &opts.doc),
    EditSubcommand::Inc(opts) => {
      echo_items!(tokens, list "added-ns" => &opts.added_ns, list "removed-ns" => &opts.removed_ns, list "ns-updated" => &opts.ns_updated, list "added" => &opts.added, list "removed" => &opts.removed, list "changed" => &opts.changed)
    }
    EditSubcommand::Cp(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "from" => &opts.from, value "path" => &opts.path, value "at" => &opts.at; default "after")
    }
    EditSubcommand::Mv(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "from" => &opts.from, value "path" => &opts.path, value "at" => &opts.at; default "after")
    }
    EditSubcommand::Rename(opts) => echo_items!(tokens, pos "source" => &opts.source, pos "new-name" => &opts.new_name),
    EditSubcommand::SplitDef(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "path" => &opts.path, value "name" => &opts.new_name)
    }
  }
}

fn push_tree(tokens: &mut Vec<String>, cmd: &TreeCommand) {
  match &cmd.subcommand {
    TreeSubcommand::Show(opts) => echo_items!(
      tokens,
      pos "target" => &opts.target,
      opt "path" => opts.path.as_deref(); default "none",
      value "depth" => opts.depth; default "2",
      switch "json" => opts.json,
      value "chunk-target-nodes" => opts.chunk_target_nodes; default "56",
      value "chunk-max-nodes" => opts.chunk_max_nodes; default "68",
      value "chunk-trigger-nodes" => opts.chunk_trigger_nodes; default "88",
      value "chunk-expand-depth" => opts.chunk_expand_depth; default "1",
      switch "raw" => opts.raw
    ),
    TreeSubcommand::Replace(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "path" => &opts.path, code_input opts, value "depth" => opts.depth; default "2")
    }
    TreeSubcommand::ReplaceLeaf(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "pattern" => &opts.pattern, switch "regex" => opts.regex, code_input opts, value "depth" => opts.depth; default "2")
    }
    TreeSubcommand::Delete(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "path" => &opts.path, value "depth" => opts.depth; default "2")
    }
    TreeSubcommand::InsertBefore(opts) => push_tree_insert(
      tokens,
      &opts.target,
      &opts.path,
      CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      ),
      opts.depth,
    ),
    TreeSubcommand::InsertAfter(opts) => push_tree_insert(
      tokens,
      &opts.target,
      &opts.path,
      CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      ),
      opts.depth,
    ),
    TreeSubcommand::InsertChild(opts) => push_tree_insert(
      tokens,
      &opts.target,
      &opts.path,
      CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      ),
      opts.depth,
    ),
    TreeSubcommand::AppendChild(opts) => push_tree_insert(
      tokens,
      &opts.target,
      &opts.path,
      CodeInputParts::new(
        opts.file.as_deref(),
        opts.code.as_deref(),
        opts.json.as_deref(),
        opts.json_input,
        opts.leaf,
      ),
      opts.depth,
    ),
    TreeSubcommand::SwapNext(opts) => push_tree_path_depth(tokens, &opts.target, &opts.path, opts.depth),
    TreeSubcommand::SwapPrev(opts) => push_tree_path_depth(tokens, &opts.target, &opts.path, opts.depth),
    TreeSubcommand::Unwrap(opts) => push_tree_path_depth(tokens, &opts.target, &opts.path, opts.depth),
    TreeSubcommand::Raise(opts) => push_tree_path_depth(tokens, &opts.target, &opts.path, opts.depth),
    TreeSubcommand::Wrap(opts) => {
      push_tree_path_depth(tokens, &opts.target, &opts.path, opts.depth);
      echo_items!(tokens, code_input opts);
    }
    TreeSubcommand::SearchReplace(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, value "pattern" => &opts.pattern, switch "regex" => opts.regex, code_input opts, value "depth" => opts.depth; default "2")
    }
    TreeSubcommand::Rewrite(opts) => {
      push_tree_path_depth(tokens, &opts.target, &opts.path, opts.depth);
      echo_items!(tokens, code_input opts, list "with" => &opts.with);
    }
    TreeSubcommand::BatchDelete(opts) => {
      echo_items!(tokens, pos "target" => &opts.target, list "paths" => &opts.paths, value "depth" => opts.depth; default "2");
    }
  }
}

struct CodeInputParts<'a> {
  file: Option<&'a str>,
  code: Option<&'a str>,
  json: Option<&'a str>,
  json_input: bool,
  leaf: bool,
}

impl<'a> CodeInputParts<'a> {
  fn new(file: Option<&'a str>, code: Option<&'a str>, json: Option<&'a str>, json_input: bool, leaf: bool) -> Self {
    Self {
      file,
      code,
      json,
      json_input,
      leaf,
    }
  }
}

fn push_tree_insert(tokens: &mut Vec<String>, target: &str, path: &str, code_input: CodeInputParts<'_>, depth: usize) {
  push_tree_path_depth(tokens, target, path, depth);
  push_code_input(tokens, &code_input);
}

fn push_tree_path_depth(tokens: &mut Vec<String>, target: &str, path: &str, depth: usize) {
  push_positional(tokens, "target", target);
  push_value(tokens, "path", path, None);
  push_value(tokens, "depth", &depth.to_string(), Some("2"));
}

fn push_code_input(tokens: &mut Vec<String>, code_input: &CodeInputParts<'_>) {
  push_optional(tokens, "file", code_input.file, "none");
  push_optional(tokens, "code", code_input.code, "none");
  push_optional(tokens, "json", code_input.json, "none");
  push_switch(tokens, "json-input", code_input.json_input);
  push_switch(tokens, "leaf", code_input.leaf);
}

fn push_switch(tokens: &mut Vec<String>, name: &str, enabled: bool) {
  tokens.push(format!("--{name}={}", if enabled { "ON" } else { "OFF" }));
}

fn push_value(tokens: &mut Vec<String>, name: &str, value: &str, default: Option<&str>) {
  let display_value = format_atom(value);

  match default {
    Some(default_value) if default_value == value => {
      tokens.push(format!("--{name}=({})", format_atom(default_value)));
    }
    _ => tokens.push(format!("--{name}={display_value}")),
  }
}

fn push_optional(tokens: &mut Vec<String>, name: &str, value: Option<&str>, default_label: &str) {
  match value {
    Some(value) => tokens.push(format!("--{name}={}", format_atom(value))),
    None => tokens.push(format!("--{name}=({})", format_atom(default_label))),
  }
}

fn push_optional_owned(tokens: &mut Vec<String>, name: &str, value: Option<String>, default_label: &str) {
  match value {
    Some(value) => tokens.push(format!("--{name}={}", format_atom(&value))),
    None => tokens.push(format!("--{name}=({})", format_atom(default_label))),
  }
}

fn push_list(tokens: &mut Vec<String>, name: &str, values: &[String]) {
  if values.is_empty() {
    tokens.push(format!("--{name}=(none)"));
  } else {
    for value in values {
      tokens.push(format!("--{name}={}", format_atom(value)));
    }
  }
}

fn push_positional(tokens: &mut Vec<String>, name: &str, value: &str) {
  tokens.push(format!("{name}={}", format_atom(value)));
}

fn format_atom(value: &str) -> String {
  if value.is_empty() {
    return String::from("''");
  }

  if value.chars().any(char::is_whitespace) {
    return format!("{value:?}");
  }

  value.to_owned()
}

fn query_name(subcommand: &QuerySubcommand) -> &'static str {
  match subcommand {
    QuerySubcommand::Ns(_) => "ns",
    QuerySubcommand::Defs(_) => "defs",
    QuerySubcommand::Pkg(_) => "pkg",
    QuerySubcommand::Config(_) => "config",
    QuerySubcommand::Error(_) => "error",
    QuerySubcommand::Modules(_) => "modules",
    QuerySubcommand::Def(_) => "def",
    QuerySubcommand::Peek(_) => "peek",
    QuerySubcommand::Examples(_) => "examples",
    QuerySubcommand::Find(_) => "find",
    QuerySubcommand::Usages(_) => "usages",
    QuerySubcommand::Search(_) => "search",
    QuerySubcommand::SearchExpr(_) => "search-expr",
    QuerySubcommand::Schema(_) => "schema",
    QuerySubcommand::HostProcs(_) => "host-procs",
  }
}

fn docs_name(subcommand: &DocsSubcommand) -> &'static str {
  match subcommand {
    DocsSubcommand::Scopes(_) => "scopes",
    DocsSubcommand::RemoteLibs(_) => "remote-libs",
    DocsSubcommand::Search(_) => "search",
    DocsSubcommand::List(_) => "list",
    DocsSubcommand::Sections(_) => "sections",
    DocsSubcommand::Read(_) => "read",
    DocsSubcommand::Agents(_) => "agents",
    DocsSubcommand::ReadLines(_) => "read-lines",
    DocsSubcommand::CheckMd(_) => "check-md",
  }
}

fn libs_name(subcommand: &LibsSubcommand) -> &'static str {
  match subcommand {
    LibsSubcommand::Readme(_) => "readme",
    LibsSubcommand::Search(_) => "search",
    LibsSubcommand::ScanMd(_) => "scan-md",
  }
}

fn cirru_name(subcommand: &CirruSubcommand) -> &'static str {
  match subcommand {
    CirruSubcommand::Parse(_) => "parse",
    CirruSubcommand::Format(_) => "format",
    CirruSubcommand::ParseEdn(_) => "parse-edn",
    CirruSubcommand::ShowGuide(_) => "show-guide",
  }
}

fn analyze_name(subcommand: &AnalyzeSubcommand) -> &'static str {
  match subcommand {
    AnalyzeSubcommand::CallGraph(_) => "call-graph",
    AnalyzeSubcommand::CallGraphDiff(_) => "call-graph-diff",
    AnalyzeSubcommand::CountCalls(_) => "count-calls",
    AnalyzeSubcommand::ProgramDiff(_) => "program-diff",
    AnalyzeSubcommand::CheckExamples(_) => "check-examples",
    AnalyzeSubcommand::CheckTypes(_) => "check-types",
    AnalyzeSubcommand::WeakTypes(_) => "weak-types",
    AnalyzeSubcommand::EffectsGraph(_) => "effects-graph",
    AnalyzeSubcommand::JsEscape(_) => "js-escape",
    AnalyzeSubcommand::JsUnescape(_) => "js-unescape",
  }
}

fn edit_name(subcommand: &EditSubcommand) -> &'static str {
  match subcommand {
    EditSubcommand::Format(_) => "format",
    EditSubcommand::Def(_) => "def",
    EditSubcommand::MvDef(_) => "mv-def",
    EditSubcommand::RmDef(_) => "rm-def",
    EditSubcommand::Doc(_) => "doc",
    EditSubcommand::Schema(_) => "schema",
    EditSubcommand::Examples(_) => "examples",
    EditSubcommand::AddExample(_) => "add-example",
    EditSubcommand::RmExample(_) => "rm-example",
    EditSubcommand::Tags(_) => "tags",
    EditSubcommand::AddNs(_) => "add-ns",
    EditSubcommand::RmNs(_) => "rm-ns",
    EditSubcommand::Imports(_) => "imports",
    EditSubcommand::AddImport(_) => "add-import",
    EditSubcommand::RmImport(_) => "rm-import",
    EditSubcommand::NsDoc(_) => "ns-doc",
    EditSubcommand::Inc(_) => "inc",
    EditSubcommand::Cp(_) => "cp",
    EditSubcommand::Mv(_) => "mv",
    EditSubcommand::Rename(_) => "rename",
    EditSubcommand::SplitDef(_) => "split-def",
  }
}

fn tree_name(subcommand: &TreeSubcommand) -> &'static str {
  match subcommand {
    TreeSubcommand::Show(_) => "show",
    TreeSubcommand::Replace(_) => "replace",
    TreeSubcommand::ReplaceLeaf(_) => "replace-leaf",
    TreeSubcommand::Delete(_) => "delete",
    TreeSubcommand::InsertBefore(_) => "insert-before",
    TreeSubcommand::InsertAfter(_) => "insert-after",
    TreeSubcommand::InsertChild(_) => "insert-child",
    TreeSubcommand::AppendChild(_) => "append-child",
    TreeSubcommand::SwapNext(_) => "swap-next",
    TreeSubcommand::SwapPrev(_) => "swap-prev",
    TreeSubcommand::Unwrap(_) => "unwrap",
    TreeSubcommand::Raise(_) => "raise",
    TreeSubcommand::Wrap(_) => "wrap",
    TreeSubcommand::SearchReplace(_) => "search-replace",
    TreeSubcommand::Rewrite(_) => "rewrite",
    TreeSubcommand::BatchDelete(_) => "batch-delete",
  }
}

fn config_name(subcommand: &ConfigSubcommand) -> &'static str {
  match subcommand {
    ConfigSubcommand::Show(_) => "show",
    ConfigSubcommand::Modules(_) => "modules",
    ConfigSubcommand::Version(_) => "version",
    ConfigSubcommand::Set(_) => "set",
    ConfigSubcommand::AddModule(_) => "add-module",
    ConfigSubcommand::RmModule(_) => "rm-module",
  }
}

fn push_config(tokens: &mut Vec<String>, cmd: &ConfigCommand) {
  match &cmd.subcommand {
    ConfigSubcommand::Show(opts) => echo_items!(tokens, opt "entry" => opts.entry.as_deref(); default "none"),
    ConfigSubcommand::Modules(opts) => echo_items!(tokens, opt "entry" => opts.entry.as_deref(); default "none"),
    ConfigSubcommand::Version(opts) => echo_items!(tokens, opt "value" => opts.value.as_deref(); default "none"),
    ConfigSubcommand::Set(opts) => {
      echo_items!(tokens, opt "entry" => opts.entry.as_deref(); default "none");
      echo_items!(tokens, pos "key" => &opts.key, pos "value" => &opts.value);
    }
    ConfigSubcommand::AddModule(opts) => {
      echo_items!(tokens, opt "entry" => opts.entry.as_deref(); default "none");
      echo_items!(tokens, pos "module_path" => &opts.module_path);
    }
    ConfigSubcommand::RmModule(opts) => {
      echo_items!(tokens, opt "entry" => opts.entry.as_deref(); default "none");
      echo_items!(tokens, pos "module_path" => &opts.module_path);
    }
  }
}

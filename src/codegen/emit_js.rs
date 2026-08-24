mod args;
mod deps;
pub mod gen_stack;
mod helpers;
mod internal_states;
mod paths;
mod runtime;
use std::fmt::Write;
mod snippets;
mod symbols;
mod tags;

use im_ternary_tree::TernaryTreeList;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::builtins::meta::{js_gensym, reset_js_gensym_index};
use crate::builtins::syntax::get_raw_args_fn;
use crate::builtins::{is_js_syntax_procs, is_proc_name};
use crate::calcit::data_shape::{DataShapeGraph, DataShapeNode};
use crate::calcit::{self, CalcitArgLabel, CalcitFnArgs, CalcitImport, CalcitList, CalcitLocal, CalcitProc, MethodKind};
use crate::calcit::{Calcit, CalcitSyntax, ImportInfo};
use crate::call_stack::StackKind;
use crate::codegen::skip_arity_check;
use crate::program;
use crate::util::string::{has_ns_part, matches_js_var, wrap_js_str};
use args::{gen_args_code, gen_call_args_with_temps};
use deps::{contains_symbol, sort_compiled_defs_by_deps};
use helpers::{cirru_to_js, is_js_unavailable_procs, write_file_if_changed};
use paths::{to_js_import_name, to_mjs_filename};
use runtime::{get_proc_prefix, is_cirru_string};
use symbols::{escape_cirru_str, escape_var};

pub fn escape_symbol_for_js(name: &str) -> String {
  escape_var(name)
}

pub fn unescape_symbol_from_js(name: &str) -> String {
  symbols::unescape_var(name)
}

thread_local! {
  static INLINE_ALL_ARGS: Cell<bool> = const { Cell::new(false) };
}

struct ImportsDict(HashSet<CalcitImport>);

impl ImportsDict {
  fn new() -> Self {
    ImportsDict(HashSet::new())
  }

  fn insert(&mut self, item: CalcitImport) {
    // println!("insert import: {:?}", item);
    self.0.insert(item);
  }

  fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

fn escape_ns(name: &str) -> String {
  // use `$` to tell namespace from normal variables, thus able to use same token like clj
  let piece = if is_cirru_string(name) {
    name[1..].replace('@', "_AT_").replace('/', "_SLSH_").replace('.', "_DOT_") // TODO
  } else {
    name.to_owned()
  };
  format!("${}", escape_var(&piece))
}

fn external_js_property_name(type_hint: &Arc<calcit::CalcitTypeAnnotation>, name: &str) -> String {
  let trait_defs: Vec<Arc<calcit::CalcitTrait>> = match type_hint.as_ref() {
    calcit::CalcitTypeAnnotation::Trait(trait_def) => vec![trait_def.clone()],
    calcit::CalcitTypeAnnotation::TraitSet(traits) => traits.as_ref().clone(),
    calcit::CalcitTypeAnnotation::Optional(inner) => match inner.as_ref() {
      calcit::CalcitTypeAnnotation::Trait(trait_def) => vec![trait_def.clone()],
      calcit::CalcitTypeAnnotation::TraitSet(traits) => traits.as_ref().clone(),
      _ => vec![],
    },
    _ => vec![],
  };
  for trait_def in trait_defs.iter().rev() {
    let Some(def_ref) = trait_def.definition_ref.as_deref() else {
      continue;
    };
    let Some((ns, def)) = def_ref.rsplit_once('/') else { continue };
    let Some(ffi) = program::lookup_def_ffi(ns, def) else { continue };
    let names = match ffi {
      cirru_edn::Edn::Struct(value) => value
        .pairs
        .iter()
        .find(|(key, _)| key.ref_str() == "names")
        .map(|(_, value)| value.clone()),
      cirru_edn::Edn::Map(value) => value.get(&cirru_edn::Edn::Tag(EdnTag::new("names"))).cloned(),
      _ => None,
    };
    let Some(names) = names else { continue };
    let mapped = match names {
      cirru_edn::Edn::Map(value) => value.get(&cirru_edn::Edn::Tag(EdnTag::new(name))).cloned(),
      _ => None,
    };
    if let Some(mapped) = mapped {
      match mapped {
        cirru_edn::Edn::Str(value) | cirru_edn::Edn::Symbol(value) => return value.to_string(),
        cirru_edn::Edn::Tag(value) => return value.ref_str().to_owned(),
        _ => {}
      }
    }
  }
  default_external_js_member_name(name)
}

/// Convert the common Calcit member spelling to the JavaScript convention.
///
/// `:names` in external-object FFI metadata always takes precedence. This
/// fallback keeps ordinary `kebab-case`, predicates, and mutating method names
/// ergonomic while retaining an exact escape hatch for APIs with unusual keys.
fn default_external_js_member_name(name: &str) -> String {
  let original_name = name;
  let name = name.trim_end_matches(['?', '!']);
  let mut output = String::with_capacity(name.len());
  let mut upper_next = false;
  for ch in name.chars() {
    if ch == '-' {
      upper_next = true;
    } else if upper_next {
      output.extend(ch.to_uppercase());
      upper_next = false;
    } else {
      output.push(ch);
    }
  }
  if output.is_empty() { original_name.to_owned() } else { output }
}

// code generated from calcit.core.cirru may not be faster enough,
// possible way to use code from calcit.procs.ts
fn is_preferred_js_proc(name: &str) -> bool {
  matches!(
    name,
    "number?"
      | "tag?"
      | "map?"
      | "nil?"
      | "list?"
      | "set?"
      | "string?"
      | "fn?"
      | "bool?"
      | "ref?"
      | "struct?"
      | "enum?"
      | "starts-with?"
      | "ends-with?"
  )
}

fn is_quote_head(value: &Calcit) -> bool {
  matches!(value, Calcit::Syntax(CalcitSyntax::Quote, _))
    || matches!(value, Calcit::Symbol { sym, .. } if sym.as_ref() == "quote")
    || matches!(value, Calcit::Import(CalcitImport { ns, def, .. }) if &**ns == calcit::CORE_NS && &**def == "quote")
}

fn is_runtime_placeholder_form(value: &Calcit) -> bool {
  matches!(value, Calcit::Symbol { sym, .. } if sym.as_ref() == "&runtime-implementation")
}

fn is_runtime_placeholder_quote(value: &Calcit) -> bool {
  let Calcit::List(items) = value else {
    return false;
  };
  items.len() == 2 && items.first().is_some_and(is_quote_head) && items.get(1).is_some_and(is_runtime_placeholder_form)
}

fn should_skip_core_def_codegen(def: &str, compiled_def: &program::CompiledDef) -> bool {
  if CalcitSyntax::is_valid(def) || is_proc_name(def) || is_js_syntax_procs(def) {
    return true;
  }

  compiled_def.source_code.as_ref().is_some_and(is_runtime_placeholder_quote)
}

fn quote_to_js(xs: &Calcit, var_prefix: &str, tags: &RefCell<HashSet<EdnTag>>) -> Result<String, String> {
  match xs {
    Calcit::Symbol { sym, .. } => Ok(format!("new {var_prefix}CalcitSymbol({})", escape_cirru_str(sym))),
    Calcit::Str(s) => Ok(escape_cirru_str(s)),
    Calcit::Bool(b) => Ok(b.to_string()),
    Calcit::Number(n) => Ok(n.to_string()),
    Calcit::Nil => Ok(String::from("null")),
    Calcit::Unit => Ok(String::from("void 0")),
    // mainly for methods, which are recognized during reading
    Calcit::Proc(p) => Ok(format!("new {var_prefix}CalcitSymbol({})", escape_cirru_str(p.as_ref()))),
    Calcit::List(ys) => {
      let mut chunk = String::from("");
      ys.traverse_result::<String>(&mut |y| {
        if !chunk.is_empty() {
          chunk.push_str(", ");
        }
        chunk.push_str(&quote_to_js(y, var_prefix, tags)?);
        Ok(())
      })?;
      Ok(format!("new {var_prefix}CalcitSliceList([{chunk}])"))
    }
    Calcit::Tag(s) => {
      let mut tags = tags.borrow_mut();
      tags.insert(s.to_owned());
      Ok(tags::tag_access(s.ref_str()))
    }
    Calcit::CirruQuote(code) => Ok(format!("new {var_prefix}CalcitCirruQuote({})", cirru_to_js(code)?)),
    Calcit::Method(name, kind) => {
      let code = match kind {
        MethodKind::Access => ".-",
        MethodKind::InvokeNative => ".!",
        MethodKind::Invoke(_) => ".",
        MethodKind::TagAccess => ".:",
        MethodKind::ExternalAccess(_) => ".:",
        MethodKind::ExternalGet(_) => "js-get:",
        MethodKind::ExternalSet(_) => "js-set:",
        MethodKind::ExternalInvoke(_) => ".",
        MethodKind::AccessOptional => ".?-",
        MethodKind::InvokeNativeOptional => ".?!",
      };
      Ok(format!("new {var_prefix}CalcitSymbol(\"{code}{}\")", name.escape_default()))
    }
    Calcit::Syntax(s, _) => Ok(format!("new {var_prefix}CalcitSymbol('{}')", s.to_string().escape_default())),
    _ => unreachable!("Unexpected data in quote for js: {}", xs),
  }
}

fn make_let_with_bind(left: &str, right: &str, body: &str, has_await: bool) -> String {
  let (await_mark, async_mark) = if has_await { ("await ", "async ") } else { ("", "") };
  let body = indent_block(body, "  ");
  format!("{await_mark}({async_mark}function __bind__({left}){{\n{body}\n}})({right})")
}

fn make_let_with_wrapper(left: &str, right: &str, body: &str, has_await: bool) -> String {
  let (await_mark, async_mark) = if has_await { ("await ", "async ") } else { ("", "") };
  let body = indent_block(&format!("let {left} = {right};\n{body}"), "  ");
  format!("{await_mark}({async_mark}function __let__(){{\n{body}\n}})()")
}

fn make_fn_wrapper(body: &str, is_async: bool) -> String {
  let body = indent_block(body, "  ");
  if is_async {
    format!("await (async function _async_fn_(){{\n{body}\n}})()")
  } else {
    format!("(function _fn_(){{\n{body}\n}})()")
  }
}

fn indent_block(body: &str, indent: &str) -> String {
  body
    .lines()
    .map(|line| {
      if line.trim().is_empty() {
        String::from("")
      } else {
        format!("{indent}{line}")
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Detects verbatim `&raw-code` segments anywhere in an expression tree. These
/// are emitted byte-for-byte, so line-based indentation must not touch them
/// (a multiline template literal would otherwise change value).
fn contains_raw_code(x: &Calcit) -> bool {
  match x {
    Calcit::RawCode(..) => true,
    Calcit::List(xs) => xs.iter().any(contains_raw_code),
    _ => false,
  }
}

fn raw_syntax_codegen_error(syntax: &CalcitSyntax) -> String {
  format!(
    "invalid JS codegen: raw syntax node `{syntax}` cannot be emitted as a standalone JS value. LLM hint: special forms must start an expression, for example `(if cond a b)`, or appear at the beginning of a line / after `$`, instead of being left as a separate argument node."
  )
}

fn to_js_code(
  xs: &Calcit,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  return_label: Option<&str>,
) -> Result<String, String> {
  // println!("to js code handle: {} {:?}", xs, xs);
  if let Calcit::List(ys) = xs {
    gen_call_code(ys, ns, local_defs, xs, file_imports, tags, return_label)
  } else {
    let ret = match xs {
      Calcit::Symbol { sym, info, .. } => {
        let passed_defs = PassedDefs {
          ns,
          local_defs,
          file_imports,
        };

        gen_symbol_code(sym, &info.at_ns, &info.at_def, xs, &passed_defs)
      }
      Calcit::Import(item @ CalcitImport { def, info, .. }) => {
        match &**info {
          ImportInfo::Core { at_ns } => {
            if &**at_ns == calcit::CORE_NS {
              // functions under core uses built $clt module entry
              Ok(escape_var(def))
            } else {
              Ok(format!("$clt.{}", escape_var(def)))
            }
          }
          ImportInfo::NsAs { .. } => {
            file_imports.borrow_mut().insert(item.to_owned());
            Ok(format!("{}.{}", escape_ns(&item.ns), escape_var(def)))
          }
          ImportInfo::JsDefault { alias, .. } => {
            // println!("Js Default: {:?}", info);
            file_imports.borrow_mut().insert(item.to_owned());
            Ok(escape_var(alias))
          }
          _ => {
            file_imports.borrow_mut().insert(item.to_owned());
            Ok(escape_var(def))
          }
        }
      }
      Calcit::Local(CalcitLocal { sym, .. }) => Ok(escape_var(sym)),
      // A bare `{}` is represented as the NativeMap proc in the preprocessed
      // tree. Unlike ordinary proc values it must be evaluated here.
      Calcit::Proc(CalcitProc::NativeMap) => {
        let proc_prefix = get_proc_prefix(ns);
        Ok(format!("{proc_prefix}{}()", escape_var(CalcitProc::NativeMap.as_ref())))
      }
      Calcit::Proc(s) => {
        let proc_prefix = get_proc_prefix(ns);
        // println!("gen proc {} under {}", s, ns,);
        // let resolved = Some(ResolvedDef(String::from(primes::CORE_NS), s.to_owned()));
        // gen_symbol_code(s, primes::CORE_NS, &resolved, ns, xs, local_defs)
        Ok(format!("{proc_prefix}{}", escape_var(s.as_ref())))
      }
      Calcit::Registered(alias) => {
        let proc_prefix = get_proc_prefix(ns);
        Ok(format!("{proc_prefix}{}", escape_var(alias)))
      }
      Calcit::Method(name, kind) => {
        let proc_prefix = get_proc_prefix(ns);
        if matches!(kind, MethodKind::Invoke(_)) {
          Ok(format!("{proc_prefix}invoke_method_closure({})", escape_cirru_str(name)))
        } else {
          Err(format!("Does not expect native method as closure: {kind}"))
        }
      }
      Calcit::Fn { info, .. } => {
        let passed_defs = PassedDefs {
          ns,
          local_defs,
          file_imports,
        };
        if let Some(def_ref) = info.def_ref.as_ref() {
          let is_local_def = passed_defs.local_defs.contains(&def_ref.def_name);
          let has_top_level_def = program::has_def_code(def_ref.def_ns.as_ref(), def_ref.def_name.as_ref());
          if def_ref.is_macro_gen || (!is_local_def && !has_top_level_def) {
            return Err(format!(
              "cannot emit JS for function literal without resolvable def: {}/{} (used_in_impl: {})",
              info.def_ns, info.name, info.usage.used_in_impl
            ));
          }
          return gen_symbol_code(
            def_ref.def_name.as_ref(),
            def_ref.def_ns.as_ref(),
            def_ref.def_name.as_ref(),
            xs,
            &passed_defs,
          );
        }
        Err(format!(
          "cannot emit JS for function literal without def reference: {}/{} (used_in_impl: {})",
          info.def_ns, info.name, info.usage.used_in_impl
        ))
      }
      Calcit::Syntax(s, ..) => Err(raw_syntax_codegen_error(s)),
      Calcit::Str(s) => Ok(escape_cirru_str(s)),
      Calcit::Bool(b) => Ok(b.to_string()),
      Calcit::Number(n) => Ok(n.to_string()),
      Calcit::Nil => Ok(String::from("null")),
      Calcit::Unit => Ok(String::from("void 0")),
      Calcit::Tag(s) => {
        let mut tags = tags.borrow_mut();
        tags.insert(s.to_owned());
        Ok(tags::tag_access(s.ref_str()))
      }
      Calcit::List(_) => unreachable!("[Error] list handled in another branch"),
      Calcit::CirruQuote(code) => {
        let proc_prefix = get_proc_prefix(ns);
        Ok(format!("new {proc_prefix}CalcitCirruQuote({})", cirru_to_js(code)?))
      }
      Calcit::RawCode(_, code) => Ok((**code).to_owned()),
      a => unreachable!("[Error] unknown kind to gen js code: {}", a),
    };

    match (return_label, &ret) {
      (Some(label), Ok(code)) => Ok(format!("{label}{code}")),
      (_, _) => ret,
    }
  }
}

fn to_js_code_inline(
  xs: &Calcit,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  return_label: Option<&str>,
) -> Result<String, String> {
  INLINE_ALL_ARGS.with(|flag| {
    let previous = flag.replace(true);
    let result = to_js_code(xs, ns, local_defs, file_imports, tags, return_label);
    flag.set(previous);
    result
  })
}

fn gen_call_code(
  ys: &CalcitList,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  xs: &Calcit,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  return_label: Option<&str>,
) -> Result<String, String> {
  let return_code = return_label.unwrap_or("");
  let var_prefix = if ns == calcit::CORE_NS { "" } else { "$clt." };
  let proc_prefix = get_proc_prefix(ns);
  let inline_all = INLINE_ALL_ARGS.with(|flag| flag.get());
  if ys.is_empty() {
    eprintln!("[Warn] Unexpected empty list inside {xs}");
    return Ok(String::from("()"));
  }

  let head = ys[0].to_owned();
  let body = ys.drop_left();
  match &head {
    Calcit::Syntax(s, ..) => {
      match &s {
        CalcitSyntax::If => gen_if_code(&body, local_defs, xs, ns, file_imports, tags, return_label),
        CalcitSyntax::CoreLet => gen_let_code(&body, local_defs, xs, ns, file_imports, tags, return_label),

        CalcitSyntax::Quote => match body.first() {
          Some(item) => quote_to_js(item, var_prefix, tags),
          None => Err(format!("quote expected a node, got nothing from {body}")),
        },
        CalcitSyntax::Defatom => match (body.first(), body.get(1)) {
          _ if body.len() > 2 => Err(format!("defatom expected name and value, got too many: {body}")),
          (Some(Calcit::Symbol { sym, .. }), Some(v)) | (Some(Calcit::Import(CalcitImport { def: sym, .. })), Some(v)) => {
            let ref_path = wrap_js_str(&format!("{ns}/{sym}"));
            gen_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &[]);
            let value_code = &to_js_code(v, ns, local_defs, file_imports, tags, None)?;
            gen_stack::pop_call_stack();
            Ok(format!(
              "\n({}peekDefatom({}) ?? {}defatom({}, {value_code}))\n",
              var_prefix, ref_path, var_prefix, ref_path
            ))
          }
          (_, _) => Err(format!("defatom expected name and value, got: {body}")),
        },

        CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport => match (body.first(), body.get(1)) {
          (Some(Calcit::Symbol { sym, .. }), Some(Calcit::List(ys))) => {
            let func_body = body.skip(2)?;
            gen_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &[]);
            let passed_defs = PassedDefs {
              ns,
              local_defs,
              file_imports,
            };
            let arg_types = extract_fn_param_types(ys);
            let raw_args = get_raw_args_fn(ys)?;
            let ret = gen_js_func(
              sym,
              JsFnParams {
                args: &raw_args,
                arg_types: &arg_types,
              },
              &func_body.to_vec(),
              &passed_defs,
              false,
              tags,
              ns,
            );
            gen_stack::pop_call_stack();
            match ret {
              Ok(code) => Ok(format!("{return_code}{code}")),
              _ => ret,
            }
          }
          (_, _) => Err(format!("defn expected name arguments, got: {}", Calcit::from(body))),
        },
        CalcitSyntax::Try => match (body.first(), body.get(1)) {
          (Some(expr), Some(handler)) => {
            gen_stack::push_call_stack(ns, "try", StackKind::Codegen, xs.to_owned(), &[]);
            let next_return_label = return_label.unwrap_or("return ");
            let try_code = to_js_code(expr, ns, local_defs, file_imports, tags, Some(next_return_label))?;
            let err_var = js_gensym("errMsg");
            let handler = to_js_code(handler, ns, local_defs, file_imports, tags, None)?;

            gen_stack::pop_call_stack();
            let code = snippets::tmpl_try(err_var, try_code, handler, next_return_label);
            match return_label {
              Some(_) => Ok(code),
              None => Ok(snippets::tmpl_fn_wrapper(code)),
            }
          }
          (_, _) => Err(format!("try expected 2 nodes, got: {body}")),
        },
        CalcitSyntax::Eval => {
          let (prelude, args_code) =
            gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
          let call_code = format!("{proc_prefix}{}({args_code})", escape_var("eval"));
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        }
        CalcitSyntax::Reset => {
          let (prelude, args_code) =
            gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
          let call_code = format!("{proc_prefix}{}({args_code})", escape_var("reset!"));
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        }
        // for `&call-spread`, just translate as normal call
        CalcitSyntax::CallSpread => gen_call_code(&body, ns, local_defs, xs, file_imports, tags, return_label),
        CalcitSyntax::HintFn => Ok(format!("{return_code}null")),
        CalcitSyntax::AssertType => Ok(format!("{return_code}null")),
        CalcitSyntax::UnsafeCoerce => match body.first() {
          Some(value) => Ok(format!(
            "{return_code}{}",
            to_js_code(value, ns, local_defs, file_imports, tags, None)?
          )),
          None => Err(String::from("unsafe-coerce expected a value")),
        },
        CalcitSyntax::ParseCirruEdnAs => match (body.first(), body.get(1)) {
          (Some(text), Some(type_form)) if body.len() == 2 || body.len() == 3 => {
            let graph = match body.get(2).and_then(DataShapeGraph::from_calcit_handle) {
              Some(graph) => graph,
              None => {
                let target = calcit::CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_form, &[]);
                Arc::new(DataShapeGraph::build(target.as_ref(), ns).map_err(|error| error.to_string())?)
              }
            };
            let graph_code = data_shape_graph_to_js(graph.as_ref(), ns, file_imports)?;
            let text_code = to_js_code(text, ns, local_defs, file_imports, tags, None)?;
            let call_code = format!("{}parse_cirru_edn_as({text_code}, {graph_code})", get_proc_prefix(ns));
            Ok(wrap_call_with_prelude(String::new(), call_code, return_label, detect_await(&body)))
          }
          _ => Err(format!("parse-cirru-edn-as expected a string and a type expression, got: {body}")),
        },
        CalcitSyntax::DecodeMapAs => match (body.first(), body.get(1)) {
          (Some(value), Some(type_form)) if body.len() == 2 || body.len() == 3 => {
            let graph = match body.get(2).and_then(DataShapeGraph::from_calcit_handle) {
              Some(graph) => graph,
              None => {
                let target = calcit::CalcitTypeAnnotation::parse_type_annotation_form_with_generics(type_form, &[]);
                Arc::new(DataShapeGraph::build_open(target.as_ref(), ns).map_err(|error| error.to_string())?)
              }
            };
            let graph_code = data_shape_graph_to_js(graph.as_ref(), ns, file_imports)?;
            let value_code = to_js_code(value, ns, local_defs, file_imports, tags, None)?;
            let call_code = format!("{}decode_map_as({value_code}, {graph_code})", get_proc_prefix(ns));
            Ok(wrap_call_with_prelude(String::new(), call_code, return_label, detect_await(&body)))
          }
          _ => Err(format!("decode-map-as expected a value and a type expression, got: {body}")),
        },
        CalcitSyntax::AssertTraits => Ok(format!("{return_code}null")),
        CalcitSyntax::Match => gen_match_code(&body, local_defs, xs, ns, file_imports, tags, return_label),
        _ => {
          let (prelude, args_code) =
            gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
          let call_code = format!("{}({})", to_js_code(&head, ns, local_defs, file_imports, tags, None)?, args_code);
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        }
      }
    }
    Calcit::Proc(CalcitProc::Raise) => {
      // not core syntax, but treat as macro for better debugging experience
      match body.first() {
        Some(m) => {
          let message: String = to_js_code(m, ns, local_defs, file_imports, tags, None)?;
          let has_await = detect_await(&body);
          let data_code = match body.get(1) {
            Some(d) => to_js_code(d, ns, local_defs, file_imports, tags, None)?,
            None => String::from("null"),
          };
          let err_var = js_gensym("err");
          let ret = format!("let {err_var} = new Error({message});\n{err_var}.data = {data_code};\nthrow {err_var};");
          // println!("inside raise: {:?} {}", return_label, xs);
          match return_label {
            Some(_) => Ok(ret),
            _ => Ok(make_fn_wrapper(&ret, has_await)),
          }
        }
        None => Err(format!("raise expected 1~2 arguments, got: {body}")),
      }
    }
    Calcit::Proc(CalcitProc::Todo) => {
      if body.len() > 1 {
        return Err(format!("todo! expects 0~1 arguments, got {}", body.len()));
      }
      let message = match body.first() {
        Some(Calcit::Str(message)) => serde_json::to_string(message.as_ref()).map_err(|error| error.to_string())?,
        Some(_) => return Err("todo! expects an optional static String message".to_owned()),
        None => String::from("\"implementation is pending\""),
      };
      let has_await = detect_await(&body);
      let ret = format!("throw new Error(`TODO: ${{{message}}}`);");
      match return_label {
        Some(_) => Ok(ret),
        _ => Ok(make_fn_wrapper(&ret, has_await)),
      }
    }
    // deftype-slot is preprocessing-only; it has no JS runtime effect.
    Calcit::Proc(CalcitProc::DeftypeSlot) => Ok(format!("{return_code}null")),
    Calcit::Proc(CalcitProc::WithTypeSlot) => Err("internal compiler error: with-type-slot escaped preprocessing".to_owned()),
    // &struct:nth: typed calls carry the expected field tag so stale schema
    // metadata fails loudly while the JS runtime reads by precomputed index.
    Calcit::Proc(CalcitProc::NativeStructNth) => {
      if body.len() == 3 {
        let record_code = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
        let idx_code = to_js_code(&body[1], ns, local_defs, file_imports, tags, None)?;
        let tag_code = to_js_code(&body[2], ns, local_defs, file_imports, tags, None)?;
        Ok(format!("{return_code}{record_code}.nthAt({idx_code}, {tag_code})"))
      } else if body.len() == 2 {
        let record_code = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
        let idx_code = to_js_code(&body[1], ns, local_defs, file_imports, tags, None)?;
        Ok(format!("{return_code}{record_code}.values[{idx_code}]"))
      } else {
        Err(format!("&struct:nth expected 2-3 arguments, got: {body}"))
      }
    }
    // &struct:assoc-at: direct indexed update with a stale-metadata tag check.
    Calcit::Proc(CalcitProc::NativeStructAssocAt) => {
      if body.len() == 4 {
        let record_code = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
        let idx_code = to_js_code(&body[1], ns, local_defs, file_imports, tags, None)?;
        let tag_code = to_js_code(&body[2], ns, local_defs, file_imports, tags, None)?;
        let value_code = to_js_code(&body[3], ns, local_defs, file_imports, tags, None)?;
        Ok(format!("{return_code}{record_code}.assocAt({idx_code}, {tag_code}, {value_code})"))
      } else {
        Err(format!("&struct:assoc-at expected 4 arguments, got: {body}"))
      }
    }
    // &struct:with-at: direct indexed batch update with tag checks.
    Calcit::Proc(CalcitProc::NativeStructWithAt) => {
      if body.len() >= 3 && (body.len() - 1).is_multiple_of(3) {
        let record_code = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
        let triple_count = (body.len() - 1) / 3;
        let mut all_args = vec![];
        for i in 0..triple_count {
          let base = 1 + i * 3;
          let idx_code = to_js_code(&body[base], ns, local_defs, file_imports, tags, None)?;
          let tag_code = to_js_code(&body[base + 1], ns, local_defs, file_imports, tags, None)?;
          let value_code = to_js_code(&body[base + 2], ns, local_defs, file_imports, tags, None)?;
          all_args.push(idx_code);
          all_args.push(tag_code);
          all_args.push(value_code);
        }
        Ok(format!("{return_code}{record_code}.withAt({})", all_args.join(", ")))
      } else {
        Err(format!(
          "&struct:with-at expected (struct, idx, tag, val, ...) triples, got: {body}"
        ))
      }
    }
    Calcit::Proc(_) => {
      let (prelude, args_code) =
        gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
      // `to_js_code(NativeMap)` evaluates a bare map. In call position we need
      // the constructor itself, otherwise map literals with entries become a
      // call of an already-created empty map.
      let callee = if matches!(head, Calcit::Proc(CalcitProc::NativeMap)) {
        format!("{proc_prefix}{}", escape_var(CalcitProc::NativeMap.as_ref()))
      } else {
        to_js_code(&head, ns, local_defs, file_imports, tags, None)?
      };
      let call_code = format!("{callee}({args_code})");
      Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
    }
    Calcit::Symbol { sym: s, .. } | Calcit::Registered(s) => {
      match &**s {
        ";" => Ok(format!("(/* {body} */ null)")),
        "hint-fn" => Ok(format!("{return_code}null")),

        "echo" | "println" => {
          // not core syntax, but treat as macro for better debugging experience
          let args = ys.drop_left();
          let args_code = gen_args_code(&args, ns, local_defs, file_imports, tags)?;
          Ok(format!("console.log({proc_prefix}printable({args_code}))"))
        }
        "eprintln" => {
          // not core syntax, but treat as macro for better debugging experience
          let args = ys.drop_left();
          let args_code = gen_args_code(&args, ns, local_defs, file_imports, tags)?;
          Ok(format!("console.error({proc_prefix}printable({args_code}))"))
        }
        "exists?" => {
          // not core syntax, but treat as macro for availability
          match body.first() {
            Some(Calcit::Symbol { .. }) | Some(Calcit::RawCode(..)) => {
              let target = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?; // TODO could be simpler
              Ok(format!("{return_code}(typeof {target} !== 'undefined')"))
            }
            Some(a) => Err(format!("exists? expected a symbol, got: {a}")),
            None => Err(format!("exists? expected 1 node, got: {body}")),
          }
        }
        "new" => match body.first() {
          Some(ctor) => {
            let args = body.drop_left();
            let (prelude, args_code) =
              gen_call_args_with_temps(&args, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
            let call_code = format!("new {}({})", to_js_code(ctor, ns, local_defs, file_imports, tags, None)?, args_code);
            Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&args)))
          }
          None => Err(format!("`new` expected constructor, got nothing, {xs}")),
        },
        "js-await" => match body.first() {
          Some(body) => Ok(format!(
            "{}(await {})",
            return_code,
            to_js_code(body, ns, local_defs, file_imports, tags, None)?
          )),
          None => Err(format!("`new` expected constructor, got nothing, {xs}")),
        },
        "instance?" => match (body.first(), body.get(1)) {
          (Some(ctor), Some(v)) => Ok(format!(
            "{}({} instanceof {})",
            return_code,
            to_js_code(v, ns, local_defs, file_imports, tags, None)?,
            to_js_code(ctor, ns, local_defs, file_imports, tags, None)?
          )),
          (_, _) => Err(format!("instance? expected 2 arguments, got: {body}")),
        },
        "set!" => match (body.first(), body.get(1)) {
          (Some(target), Some(v)) => Ok(format!(
            "{} = {}",
            to_js_code(target, ns, local_defs, file_imports, tags, None)?,
            to_js_code(v, ns, local_defs, file_imports, tags, None)?
          )),
          (_, _) => Err(format!("set! expected 2 nodes, got: {body}")),
        },
        "&raw-code" => match body.first() {
          Some(Calcit::Str(s)) => Ok(format!("{}{}", return_label.unwrap_or(""), s)),
          Some(a) => Err(format!("&raw-code expected a string, got: {a}")),
          None => Err(format!("&raw-code expected 1 node, got: {body}")),
        },
        _ => {
          // TODO
          let (prelude, args_code) =
            gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
          let call_code = format!("{}({})", to_js_code(&head, ns, local_defs, file_imports, tags, None)?, args_code);
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        }
      }
    }
    Calcit::Method(name, kind) => match kind {
      MethodKind::Access => {
        if body.len() == 1 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          if matches_js_var(name) {
            Ok(format!("{return_code}{obj}.{name}"))
          } else {
            Ok(format!("{return_code}{obj}[{}]", escape_cirru_str(name)))
          }
        } else {
          Err(format!("accessor takes only 1 argument, {xs}"))
        }
      }
      MethodKind::AccessOptional => {
        if body.len() == 1 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          if matches_js_var(name) {
            Ok(format!("{return_code}{obj}?.{name}"))
          } else {
            Ok(format!("{return_code}{obj}?.[{}]", escape_cirru_str(name)))
          }
        } else {
          Err(format!("optional accessor takes only 1 argument, {xs}"))
        }
      }
      MethodKind::InvokeNative => {
        if !body.is_empty() {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let (prelude, args_code) = gen_call_args_with_temps(
            &body.skip(1).expect("get args"),
            ns,
            local_defs,
            file_imports,
            tags,
            return_label.is_some(),
            inline_all,
          )?;

          let caller = if matches_js_var(name) {
            format!("{obj}.{name}")
          } else {
            format!("{obj}[{}]", escape_cirru_str(name))
          };
          let call_code = format!("{caller}({args_code})");
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        } else {
          Err(format!("invoke-native expected at least 1 object, got: {xs}"))
        }
      }
      MethodKind::InvokeNativeOptional => {
        if !body.is_empty() {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let (prelude, args_code) = gen_call_args_with_temps(
            &body.skip(1).expect("get args"),
            ns,
            local_defs,
            file_imports,
            tags,
            return_label.is_some(),
            inline_all,
          )?;

          let caller = if matches_js_var(name) {
            format!("{obj}.{name}")
          } else {
            format!("{obj}[{}]", escape_cirru_str(name))
          };
          let call_code = format!("{caller}?.({args_code})");
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        } else {
          Err(format!("invoke-native-optional expected at least 1 object, got: {xs}"))
        }
      }
      MethodKind::Invoke(_) => {
        let proc_prefix = get_proc_prefix(ns);
        if !body.is_empty() {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let (prelude, args_code) = gen_call_args_with_temps(
            &body.skip(1).expect("get args"),
            ns,
            local_defs,
            file_imports,
            tags,
            return_label.is_some(),
            inline_all,
          )?;

          let call_code = format!("{}invoke_method({},{},{})", proc_prefix, escape_cirru_str(name), obj, args_code);
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        } else {
          Err(format!("expected at least 1 object, got: {xs}"))
        }
      }
      MethodKind::TagAccess => {
        if body.len() == 1 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let tag = tags::tag_access(name);
          Ok(format!("{obj}.get({tag})"))
        } else {
          Err(format!("tag-accessor takes only 1 argument, {xs}"))
        }
      }
      MethodKind::ExternalAccess(type_hint) => {
        if body.len() == 1 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let property = external_js_property_name(type_hint, name.as_ref());
          Ok(format!("{obj}[{}]", escape_cirru_str(&property)))
        } else {
          Err(format!("external-access takes only 1 argument, {xs}"))
        }
      }
      MethodKind::ExternalGet(type_hint) => {
        if body.len() == 1 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let property = external_js_property_name(type_hint, name.as_ref());
          Ok(format!("{return_code}{obj}[{}]", escape_cirru_str(&property)))
        } else {
          Err(format!("external-get takes only 1 argument, {xs}"))
        }
      }
      MethodKind::ExternalSet(type_hint) => {
        if body.len() == 2 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let value = to_js_code(&body[1], ns, local_defs, file_imports, tags, None)?;
          let property = external_js_property_name(type_hint, name.as_ref());
          Ok(format!("{return_code}({obj}[{}] = {value})", escape_cirru_str(&property)))
        } else {
          Err(format!("external-set takes 2 arguments, {xs}"))
        }
      }
      MethodKind::ExternalInvoke(type_hint) => {
        if !body.is_empty() {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
          let (prelude, args_code) = gen_call_args_with_temps(
            &body.skip(1).expect("get args"),
            ns,
            local_defs,
            file_imports,
            tags,
            return_label.is_some(),
            inline_all,
          )?;
          let property = external_js_property_name(type_hint, name.as_ref());
          let call_code = format!("{obj}[{}]({args_code})", escape_cirru_str(&property));
          Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
        } else {
          Err(format!("external-invoke expected at least 1 object, {xs}"))
        }
      }
    },
    _ => {
      let (prelude, args_code) =
        gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
      let call_code = format!("{}({})", to_js_code(&head, ns, local_defs, file_imports, tags, None)?, args_code);
      Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
    }
  }
}

fn data_shape_graph_to_js(graph: &DataShapeGraph, current_ns: &str, file_imports: &RefCell<ImportsDict>) -> Result<String, String> {
  let mut nodes = Vec::with_capacity(graph.nodes.len());
  for node in &graph.nodes {
    let code = match node {
      DataShapeNode::Dynamic => String::from("{kind:\"dynamic\"}"),
      DataShapeNode::Unit => String::from("{kind:\"unit\"}"),
      DataShapeNode::Bool => String::from("{kind:\"bool\"}"),
      DataShapeNode::Number => String::from("{kind:\"number\"}"),
      DataShapeNode::String => String::from("{kind:\"string\"}"),
      DataShapeNode::Symbol => String::from("{kind:\"symbol\"}"),
      DataShapeNode::Tag => String::from("{kind:\"tag\"}"),
      DataShapeNode::Buffer => String::from("{kind:\"buffer\"}"),
      DataShapeNode::CirruQuote => String::from("{kind:\"cirru-quote\"}"),
      DataShapeNode::Optional(inner) => format!("{{kind:\"optional\",inner:{inner}}}"),
      DataShapeNode::MapOption { nominal_path, inner, .. } => {
        let nominal = nominal_ref_to_js(nominal_path.as_ref(), current_ns, file_imports)?;
        format!("{{kind:\"map-option\",nominal:{nominal},inner:{inner}}}")
      }
      DataShapeNode::List(inner) => format!("{{kind:\"list\",inner:{inner}}}"),
      DataShapeNode::Set(inner) => format!("{{kind:\"set\",inner:{inner}}}"),
      DataShapeNode::Map { key, value } => format!("{{kind:\"map\",key:{key},value:{value}}}"),
      DataShapeNode::Ref(inner) => format!("{{kind:\"ref\",inner:{inner}}}"),
      DataShapeNode::Struct { nominal_path, fields, .. } => {
        let nominal = nominal_ref_to_js(nominal_path.as_ref(), current_ns, file_imports)?;
        let fields = fields
          .iter()
          .map(|(field, node_id)| format!("[{},{}]", escape_cirru_str(field.ref_str()), node_id))
          .collect::<Vec<_>>()
          .join(",");
        format!("{{kind:\"struct\",nominal:{nominal},fields:[{fields}]}}")
      }
      DataShapeNode::Enum {
        nominal_path, variants, ..
      } => {
        let nominal = nominal_ref_to_js(nominal_path.as_ref(), current_ns, file_imports)?;
        let variants = variants
          .iter()
          .map(|(tag, payloads)| {
            let payloads = payloads.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
            format!("{{tag:{},payload:[{payloads}]}}", escape_cirru_str(tag.ref_str()))
          })
          .collect::<Vec<_>>()
          .join(",");
        format!("{{kind:\"enum\",nominal:{nominal},variants:[{variants}]}}")
      }
    };
    nodes.push(code);
  }
  Ok(format!(
    "{{version:{},root:{},fingerprint:{},nodes:[{}]}}",
    graph.abi_version(),
    graph.root,
    escape_cirru_str(graph.fingerprint()),
    nodes.join(",")
  ))
}

fn nominal_ref_to_js(
  path: Option<&(Arc<str>, Arc<str>)>,
  current_ns: &str,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  let Some((target_ns, target_def)) = path else {
    return Err(String::from(
      "parse-cirru-edn-as cannot emit JS for a nominal type without a namespace/definition path",
    ));
  };
  if target_ns.as_ref() == current_ns {
    return Ok(escape_var(target_def));
  }
  if target_ns.as_ref() == calcit::CORE_NS {
    return Ok(format!("{}{}", get_proc_prefix(current_ns), escape_var(target_def)));
  }

  file_imports.borrow_mut().insert(CalcitImport {
    ns: target_ns.clone(),
    def: target_def.clone(),
    info: Arc::new(ImportInfo::NsAs {
      at_ns: Arc::from(current_ns),
      at_def: Arc::from("parse-cirru-edn-as"),
      alias: target_ns.clone(),
    }),
    def_id: None,
  });
  Ok(format!("{}.{}", escape_ns(target_ns), escape_var(target_def)))
}

/// a group of arguments related to scopes
struct PassedDefs<'a> {
  ns: &'a str,
  local_defs: &'a HashSet<Arc<str>>,
  file_imports: &'a RefCell<ImportsDict>,
}

fn gen_symbol_code(s: &str, def_ns: &str, at_def: &str, xs: &Calcit, passed_defs: &PassedDefs) -> Result<String, String> {
  // println!("gen symbol: {} {} {} {:?}", s, def_ns, ns, resolved);
  let var_prefix = if passed_defs.ns == calcit::CORE_NS { "" } else { "$clt." };
  if has_ns_part(s) {
    unreachable!("unknown feature: {s} {def_ns} {at_def} {xs}");
  }
  if is_js_syntax_procs(s) || is_proc_name(s) || CalcitSyntax::is_valid(s) {
    // return Ok(format!("{}{}", var_prefix, escape_var(s)));
    let proc_prefix = get_proc_prefix(passed_defs.ns);
    Ok(format!("{proc_prefix}{}", escape_var(s)))
  } else if passed_defs.local_defs.contains(s) {
    Ok(escape_var(s))
  } else if def_ns == calcit::CORE_NS {
    if !program::has_def_code(calcit::CORE_NS, s) {
      eprintln!(
        "[Warn] unresolved core symbol `{s}` during JS codegen in {}/{at_def}",
        passed_defs.ns
      );
    }
    Ok(format!("{var_prefix}{}", escape_var(s)))
  } else if def_ns.is_empty() {
    Err(format!("Unexpected ns at symbol, {xs}"))
  } else if def_ns != passed_defs.ns {
    // probably via macro
    // TODO dirty code collecting imports

    Ok(escape_var(s))
  } else if def_ns == passed_defs.ns {
    eprintln!("[Warn] detected unresolved variable `{s}` in {}/{at_def}", passed_defs.ns);
    Ok(escape_var(s))
  } else {
    eprintln!("[Warn] Unexpected case, code gen for `{s}` in {}/{at_def}", passed_defs.ns);
    Ok(format!("{var_prefix}{}", escape_var(s)))
  }
}

fn detect_await(xs: &CalcitList) -> bool {
  xs.iter().any(detect_await_node)
}

fn detect_await_node(x: &Calcit) -> bool {
  match x {
    Calcit::List(al) => {
      if matches!(
        al.first(),
        Some(Calcit::Syntax(
          CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport,
          _
        ))
      ) {
        // a nested function has its own scope deciding if it's async
        false
      } else {
        detect_await(al)
      }
    }
    Calcit::Symbol { sym, .. } if &**sym == "js-await" => true,
    _ => false,
  }
}

fn gen_let_code(
  body: &CalcitList,
  local_defs: &HashSet<Arc<str>>,
  xs: &Calcit,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  base_return_label: Option<&str>,
) -> Result<String, String> {
  let mut let_def_body = body.to_owned();
  let return_label = base_return_label.unwrap_or("return ");
  let has_await = detect_await(body);

  // defined new local variable
  let mut scoped_defs = local_defs.to_owned();
  let mut defs_code = String::from("");
  let mut body_part = String::from("");

  // break unless nested &let is found
  loop {
    if let_def_body.len() <= 1 {
      return Err(format!("&let expected body, but got empty, {}", xs.lisp_str()));
    }
    let pair = let_def_body[0].to_owned();
    let content = let_def_body.drop_left();

    match &let_def_body[0] {
      Calcit::Nil => {
        for (idx, x) in content.iter().enumerate() {
          if idx == content.len() - 1 {
            body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(return_label))?);
            body_part.push('\n');
          } else {
            let line = to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(""))?;
            body_part.push_str("{\n");
            body_part.push_str(&line);
            body_part.push_str(";\n}\n");
          }
        }
        break;
      }
      Calcit::List(xs) if xs.is_empty() => {
        // non content defs_code

        for (idx, x) in content.iter().enumerate() {
          if idx == content.len() - 1 {
            body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(return_label))?);
            body_part.push('\n');
          } else {
            let line = to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(""))?;
            body_part.push_str("{\n");
            body_part.push_str(&line);
            body_part.push_str(";\n}\n");
          }
        }
        break;
      }
      Calcit::List(xs) if xs.len() == 2 => {
        let def_name = xs[0].to_owned();
        let def_code = xs[1].to_owned();

        match &def_name {
          Calcit::Local(CalcitLocal { sym, .. }) => {
            // TODO `let` inside expressions makes syntax error
            let left = escape_var(sym);
            let right = to_js_code(&def_code, ns, &scoped_defs, file_imports, tags, None)?;
            writeln!(defs_code, "let {left} = {right};").expect("write");

            if scoped_defs.contains(sym) {
              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  // normally, last item of function body returns as return value(even in recursion)
                  if local_defs.contains(sym) {
                    // however, to shallow a conflicted variable, we need to return explicitly
                    body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, tags, Some("return "))?);
                  } else {
                    body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(return_label))?);
                  }
                  body_part.push('\n');
                } else {
                  let line = to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(""))?;
                  body_part.push_str("{\n");
                  body_part.push_str(&line);
                  body_part.push_str(";\n}\n");
                }
              }

              // first variable is using conflicted name
              let ret = if local_defs.contains(sym) {
                make_let_with_bind(&left, &right, &body_part, has_await)
              } else {
                make_let_with_wrapper(&left, &right, &body_part, has_await)
              };
              return match base_return_label {
                Some(label) => Ok(format!("{label}{ret}")),
                None => Ok(ret),
              };
            } else {
              // track variable
              scoped_defs.insert(sym.to_owned());

              if content.len() == 1 {
                match &content[0] {
                  Calcit::List(ys) if ys.len() > 2 => match (&ys[0], &ys[1]) {
                    (Calcit::Syntax(sym, _ns), Calcit::List(zs)) if *sym == CalcitSyntax::CoreLet && zs.len() == 2 => match &zs[0] {
                      Calcit::Symbol { sym: s2, .. } if !scoped_defs.contains(s2) => {
                        let_def_body = ys.drop_left();
                        continue;
                      }
                      _ => (),
                    },
                    _ => (),
                  },
                  _ => (),
                }
              }

              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, tags, Some(return_label))?);
                  body_part.push('\n');
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, tags, None)?);
                  body_part.push_str(";\n");
                }
              }

              break;
            }
          }
          _ => return Err(format!("Expected symbol in &let binding, got: {}", pair)),
        }
      }
      Calcit::List(_xs) => return Err(format!("expected pair of length 2, got: {}", pair)),
      _ => return Err(format!("expected pair of a list of length 2, got: {pair}")),
    }
  }
  if base_return_label.is_some() {
    Ok(format!("{defs_code}{body_part}"))
  } else {
    Ok(make_fn_wrapper(&format!("{defs_code}{body_part}"), has_await))
  }
}

/// Generate JS code for `match` syntax.
/// After preprocessing, `body` is: [<value>, (<pattern1> <body1>), (<pattern2> <body2>), ...]
/// Generated JS is an IIFE with if-else chain checking enum tag and arity.
fn gen_match_code(
  body: &CalcitList,
  local_defs: &HashSet<Arc<str>>,
  _xs: &Calcit,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  base_return_label: Option<&str>,
) -> Result<String, String> {
  if body.is_empty() {
    return Err("match expected value and branches".to_owned());
  }

  if let (Some(Calcit::EnumDef(_)), Some(Calcit::List(table))) = (body.get(1), body.get(2)) {
    return gen_indexed_match_code(&body[0], table, local_defs, ns, file_imports, tags, base_return_label);
  }

  let has_await = detect_await(body);
  let return_label = base_return_label.unwrap_or("return ");
  let proc_prefix = get_proc_prefix(ns);

  let value_code = to_js_code(&body[0], ns, local_defs, file_imports, tags, None)?;
  let val_var = js_gensym("match_v");
  let tag_var = js_gensym("match_t");

  let mut chunk = String::new();
  writeln!(chunk, "let {val_var} = {value_code};").expect("write");
  writeln!(chunk, "let {tag_var} = {proc_prefix}_$n_enum_$o_nth({val_var}, 0);").expect("write");

  let mut first = true;
  for branch_idx in 1..body.len() {
    let branch = match &body[branch_idx] {
      Calcit::List(xs) if xs.len() == 2 => xs,
      other => return Err(format!("match branch expected a pair, got: {other}")),
    };

    let pattern = &branch[0];
    let branch_body = &branch[1];

    match pattern {
      // Wildcard
      Calcit::Symbol { sym, .. } if sym.as_ref() == "_" => {
        let body_code = to_js_code(branch_body, ns, local_defs, file_imports, tags, Some(return_label))?;
        if first {
          writeln!(chunk, "{{ {body_code} }}").expect("write");
        } else {
          writeln!(chunk, " else {{ {body_code} }}").expect("write");
        }
        first = false;
      }
      Calcit::Local(CalcitLocal { sym, .. }) if sym.as_ref() == "_" => {
        let body_code = to_js_code(branch_body, ns, local_defs, file_imports, tags, Some(return_label))?;
        if first {
          writeln!(chunk, "{{ {body_code} }}").expect("write");
        } else {
          writeln!(chunk, " else {{ {body_code} }}").expect("write");
        }
        first = false;
      }
      // Tag pattern: (:tag binding1 binding2 ...)
      Calcit::List(pat_xs) if !pat_xs.is_empty() => {
        let tag_name = match &pat_xs[0] {
          Calcit::Tag(t) => t,
          other => return Err(format!("match pattern expected tag, got: {other}")),
        };

        tags.borrow_mut().insert(tag_name.to_owned());
        let tag_code = tags::tag_access(tag_name.ref_str());
        let arity = pat_xs.len(); // includes tag, so total enum item count

        let else_mark = if first { "" } else { " else " };
        write!(
          chunk,
          "{else_mark}if ({tag_var} === {tag_code} && {proc_prefix}_$n_enum_$o_count({val_var}) === {arity}) {{"
        )
        .expect("write");

        // Generate binding code
        let mut scoped_defs = local_defs.to_owned();
        for (i, binding) in pat_xs.iter().skip(1).enumerate() {
          let bind_name = match binding {
            Calcit::Local(CalcitLocal { sym, .. }) => escape_var(sym),
            Calcit::Symbol { sym, .. } => escape_var(sym),
            other => return Err(format!("match binding expected symbol, got: {other}")),
          };
          if let Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } = binding {
            scoped_defs.insert(sym.to_owned());
          }
          write!(chunk, "\nlet {bind_name} = {proc_prefix}_$n_enum_$o_nth({val_var}, {});", i + 1).expect("write");
        }

        let body_code = to_js_code(branch_body, ns, &scoped_defs, file_imports, tags, Some(return_label))?;
        writeln!(chunk, "\n{body_code} }}").expect("write");
        first = false;
      }
      other => return Err(format!("match unexpected pattern: {other}")),
    }
  }

  // Add fallthrough error if no wildcard
  if !body.iter().skip(1).any(|b| {
    matches!(b,
      Calcit::List(xs) if xs.len() == 2 && matches!(&xs[0],
        Calcit::Symbol { sym, .. } | Calcit::Local(CalcitLocal { sym, .. }) if sym.as_ref() == "_"
      )
    )
  }) {
    write!(
      chunk,
      " else {{ throw new Error(\"match: no matching branch for tag \" + {tag_var}); }}"
    )
    .expect("write");
  }

  if base_return_label.is_some() {
    Ok(chunk)
  } else {
    Ok(make_fn_wrapper(&chunk, has_await))
  }
}

fn gen_indexed_match_code(
  value: &Calcit,
  table: &CalcitList,
  local_defs: &HashSet<Arc<str>>,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  base_return_label: Option<&str>,
) -> Result<String, String> {
  let Some(wildcard_idx) = table.len().checked_sub(1) else {
    return Err("indexed match table has invalid length".to_owned());
  };

  let has_await = detect_await_node(value) || detect_await(table);
  let return_label = "return ";
  let proc_prefix = get_proc_prefix(ns);
  let value_code = to_js_code(value, ns, local_defs, file_imports, tags, None)?;
  let val_var = js_gensym("match_v");
  let tag_var = js_gensym("match_t");
  let mut chunk = String::new();
  writeln!(chunk, "let {val_var} = {value_code};").expect("write");
  writeln!(chunk, "let {tag_var} = {proc_prefix}_$n_enum_$o_nth({val_var}, 0);").expect("write");
  writeln!(chunk, "switch ({tag_var}.idx) {{").expect("write");

  for branch in table.iter().take(wildcard_idx) {
    let Calcit::List(pair) = branch else {
      if matches!(branch, Calcit::Nil) {
        continue;
      }
      return Err(format!("indexed match slot expected a branch pair, got: {branch}"));
    };
    if pair.len() != 2 {
      return Err(format!("indexed match branch expected a pair, got: {branch}"));
    }
    let Calcit::List(pattern) = &pair[0] else {
      return Err(format!("indexed match pattern expected a list, got: {}", pair[0]));
    };
    let Some(Calcit::Tag(tag)) = pattern.first() else {
      return Err(format!("indexed match pattern expected a tag, got: {}", pair[0]));
    };
    tags.borrow_mut().insert(tag.to_owned());
    let tag_code = tags::tag_access(tag.ref_str());
    let arity = pattern.len();
    writeln!(chunk, "case {tag_code}.idx:").expect("write");
    writeln!(chunk, "if ({proc_prefix}_$n_enum_$o_count({val_var}) === {arity}) {{").expect("write");

    let mut scoped_defs = local_defs.to_owned();
    for (idx, binding) in pattern.iter().skip(1).enumerate() {
      let bind_name = match binding {
        Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. } => {
          scoped_defs.insert(sym.to_owned());
          escape_var(sym)
        }
        other => return Err(format!("match binding expected symbol, got: {other}")),
      };
      writeln!(chunk, "let {bind_name} = {proc_prefix}_$n_enum_$o_nth({val_var}, {});", idx + 1).expect("write");
    }
    let body_code = to_js_code(&pair[1], ns, &scoped_defs, file_imports, tags, Some(return_label))?;
    writeln!(chunk, "{body_code}").expect("write");
    writeln!(chunk, "}}").expect("write");
    writeln!(chunk, "break;").expect("write");
  }
  writeln!(chunk, "}}").expect("write");

  match &table[wildcard_idx] {
    Calcit::Nil => {
      write!(chunk, "throw new Error(\"match: no matching branch for tag \" + {tag_var});").expect("write");
    }
    Calcit::List(pair) if pair.len() == 2 => {
      let body_code = to_js_code(&pair[1], ns, local_defs, file_imports, tags, Some(return_label))?;
      write!(chunk, "{body_code}").expect("write");
    }
    other => return Err(format!("indexed match wildcard slot expected a branch pair, got: {other}")),
  }

  let wrapped = make_fn_wrapper(&chunk, has_await);
  match base_return_label {
    Some(label) => Ok(format!("{label}{wrapped}")),
    None => Ok(wrapped),
  }
}

fn gen_if_code(
  body: &CalcitList,
  local_defs: &HashSet<Arc<str>>,
  _xs: &Calcit,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
  base_return_label: Option<&str>,
) -> Result<String, String> {
  if body.len() < 2 || body.len() > 3 {
    Err(format!("if expected 2~3 nodes, got: {}", Calcit::from(body.to_owned())))
  } else {
    let mut chunk: String = String::from("");
    let mut cond_node = body[0].to_owned();
    let mut true_node = body[1].to_owned();
    let mut some_false_node = body.get(2);
    let mut need_else = false;
    let has_await = detect_await(body);

    let return_label = base_return_label.unwrap_or("return ");

    if base_return_label.is_none() && !has_await {
      let mut expr = String::from("");
      let mut depth = 0;
      loop {
        let cond_code = to_js_code(&cond_node, ns, local_defs, file_imports, tags, None)?;
        let true_code = to_js_code_inline(&true_node, ns, local_defs, file_imports, tags, None)?;
        write!(expr, "({cond_code} ? {true_code} : ").expect("write");
        depth += 1;

        if let Some(false_node) = some_false_node {
          if let Calcit::List(ys) = false_node
            && let Some(Calcit::Syntax(syn, _ns)) = ys.first()
            && syn == &CalcitSyntax::If
          {
            if ys.len() < 3 || ys.len() > 4 {
              return Err(format!("if expected 2~3 nodes, got: {}", Calcit::List(ys.to_owned())));
            }
            ys[1].clone_into(&mut cond_node);
            ys[2].clone_into(&mut true_node);
            some_false_node = ys.get(3);
            continue;
          }

          let false_code = to_js_code_inline(false_node, ns, local_defs, file_imports, tags, None)?;
          expr.push_str(&false_code);
        } else {
          expr.push_str("null");
        }

        for _ in 0..depth {
          expr.push(')');
        }
        break;
      }

      return Ok(expr);
    }

    loop {
      let cond_code = to_js_code(&cond_node, ns, local_defs, file_imports, tags, None)?;
      let true_code = to_js_code(&true_node, ns, local_defs, file_imports, tags, Some(return_label))?;
      let else_mark = if need_else { " else " } else { "" };

      write!(chunk, "\n{else_mark}if ({cond_code}) {{ {true_code} }}").expect("write");

      if let Some(false_node) = some_false_node {
        if let Calcit::List(ys) = false_node
          && let Some(Calcit::Syntax(syn, _ns)) = ys.first()
          && syn == &CalcitSyntax::If
        {
          if ys.len() < 3 || ys.len() > 4 {
            return Err(format!("if expected 2~3 nodes, got: {}", Calcit::List(ys.to_owned())));
          }
          ys[1].clone_into(&mut cond_node);
          ys[2].clone_into(&mut true_node);
          some_false_node = ys.get(3);
          need_else = true;
          continue;
        }

        let false_code = to_js_code(false_node, ns, local_defs, file_imports, tags, Some(return_label))?;
        write!(chunk, " else {{ {false_code} }}").expect("write");
      } else {
        write!(chunk, " else {{ {return_label} null; }}").expect("write");
      }
      break;
    }

    if base_return_label.is_some() {
      Ok(chunk)
    } else {
      Ok(make_fn_wrapper(&chunk, has_await))
    }
  }
}

fn wrap_call_with_prelude(prelude: String, call_code: String, return_label: Option<&str>, has_await: bool) -> String {
  if prelude.is_empty() {
    match return_label {
      Some(label) => format!("{label}{call_code}"),
      None => call_code,
    }
  } else {
    match return_label {
      Some(label) => format!("{prelude}{label}{call_code}"),
      None => make_fn_wrapper(&format!("{prelude}return {call_code};"), has_await),
    }
  }
}

fn list_to_js_code(
  xs: &TernaryTreeList<Calcit>,
  ns: &str,
  local_defs: HashSet<Arc<str>>,
  return_label: &str,
  file_imports: &RefCell<ImportsDict>,
  tags: &RefCell<HashSet<EdnTag>>,
) -> Result<String, String> {
  // TODO default returnLabel="return "
  let mut result = String::from("");
  for (idx, x) in xs.into_iter().enumerate() {
    // result = result & "// " & $x & "\n"
    if idx == xs.len() - 1 {
      let line = to_js_code(x, ns, &local_defs, file_imports, tags, Some(return_label))?;
      result.push_str(&line);
      result.push('\n');
    } else {
      let line = to_js_code(x, ns, &local_defs, file_imports, tags, Some(""))?;
      result.push_str("{\n");
      if contains_raw_code(x) {
        result.push_str(&line);
      } else {
        result.push_str(&indent_block(&line, "  "));
      }
      result.push_str(";\n}\n");
    }
  }
  Ok(result)
}

fn uses_recur(xs: &Calcit) -> bool {
  match xs {
    Calcit::Symbol { sym: s, .. } => &**s == "recur",
    Calcit::Proc(s) => *s == CalcitProc::Recur,
    Calcit::List(ys) => match &ys.first() {
      Some(Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _)) => false,
      Some(Calcit::Symbol { sym, .. }) if matches!(sym.as_ref(), "defn" | "defwasm-export" | "defwasm-import") => false,
      _ => {
        for y in &**ys {
          if uses_recur(y) {
            return true;
          }
        }
        false
      }
    },
    _ => false,
  }
}

struct JsFnParams<'a> {
  args: &'a CalcitFnArgs,
  arg_types: &'a [Arc<calcit::CalcitTypeAnnotation>],
}

fn gen_js_func(
  name: &str,
  params: JsFnParams<'_>,
  raw_body: &[Calcit],
  passed_defs: &PassedDefs,
  exported: bool,
  tags: &RefCell<HashSet<EdnTag>>,
  at_ns: &str,
) -> Result<String, String> {
  let args = params.args;
  let arg_types = params.arg_types;
  let var_prefix = if passed_defs.ns == "calcit.core" { "" } else { "$clt." };
  let mut local_defs = passed_defs.local_defs.to_owned();
  let mut spreading_code = String::from(""); // js list and calcit-js list are different, need to convert
  let mut args_code = String::from("");
  let mut spreading = false;
  let mut has_optional = false;
  let mut args_count = 0;
  let mut optional_count = 0;
  let mut recur_arg_names: Vec<String> = vec![];
  let mut fixed_arg_names: Vec<String> = vec![];
  let has_rest_param =
    matches!(args, CalcitFnArgs::MarkedArgs(labels) if labels.iter().any(|label| matches!(label, CalcitArgLabel::RestMark)));
  let trailing_option_count = if has_rest_param {
    0
  } else {
    calcit::trailing_option_arg_count(arg_types, args.param_len())
  };

  match args {
    CalcitFnArgs::MarkedArgs(args) => {
      for sym in args {
        if spreading {
          if let CalcitArgLabel::Idx(idx) = sym {
            if !args_code.is_empty() {
              args_code.push_str(", ");
            }
            let sym = CalcitLocal::read_name(*idx);
            let rest_used = raw_body.iter().any(|line| contains_symbol(line, &sym));
            let arg_name = escape_var(&sym);
            local_defs.insert(sym.into());
            args_code.push_str("...");
            args_code.push_str(&arg_name);
            // js list and calcit-js are different in spreading
            // only convert when rest arg is actually referenced in function body
            if rest_used {
              write!(spreading_code, "\n{arg_name} = {var_prefix}arrayToList({arg_name});").expect("write");
            }
            break; // no more args after spreading argument
          } else {
            return Err(format!("unexpected argument after spreading: {sym}"));
          }
        } else if has_optional {
          if let CalcitArgLabel::Idx(idx) = sym {
            if !args_code.is_empty() {
              args_code.push_str(", ");
            }
            let sym = CalcitLocal::read_name(*idx);
            let arg_name = escape_var(&sym);
            args_code.push_str(&arg_name);
            fixed_arg_names.push(arg_name);
            local_defs.insert(sym.into());
            optional_count += 1;
          } else {
            return Err(format!("unexpected argument after optional: {sym}"));
          }
        } else {
          match sym {
            CalcitArgLabel::RestMark => {
              spreading = true;
            }
            CalcitArgLabel::OptionalMark => {
              has_optional = true;
            }
            CalcitArgLabel::Idx(idx) => {
              if !args_code.is_empty() {
                args_code.push_str(", ");
              }
              let sym = CalcitLocal::read_name(*idx);
              let arg_name = escape_var(&sym);
              args_code.push_str(&arg_name);
              fixed_arg_names.push(arg_name.clone());
              recur_arg_names.push(arg_name);
              local_defs.insert(sym.into());
              args_count += 1;
            }
          }
        }
      }
    }
    CalcitFnArgs::Args(args) => {
      for idx in args {
        if !args_code.is_empty() {
          args_code.push_str(", ");
        }
        let sym = CalcitLocal::read_name(*idx);
        let arg_name = escape_var(&sym);
        args_code.push_str(&arg_name);
        fixed_arg_names.push(arg_name.clone());
        recur_arg_names.push(arg_name);
        local_defs.insert(sym.into());
        args_count += 1;
      }
    }
  }

  let check_args = if skip_arity_check() {
    "".into()
  } else if spreading {
    snippets::tmpl_args_fewer_than(name, args_count, at_ns)
  } else if trailing_option_count > 0 {
    let option_required = args.param_len() - trailing_option_count;
    let required = if has_optional {
      args_count.min(option_required)
    } else {
      option_required
    };
    snippets::tmpl_args_between(name, required, args.param_len(), at_ns)
  } else if has_optional {
    snippets::tmpl_args_between(name, args_count, args_count + optional_count, at_ns)
  } else {
    snippets::tmpl_args_exact(name, args_count, at_ns)
  };

  let option_fill_code = if trailing_option_count == 0 {
    String::new()
  } else {
    let option_start = args.param_len() - trailing_option_count;
    let none_call = format!("{var_prefix}{}()", escape_var("%none"));
    fixed_arg_names
      .iter()
      .enumerate()
      .skip(option_start)
      .map(|(idx, arg_name)| format!("if (arguments.length >= {option_start} && arguments.length <= {idx}) {arg_name} = {none_call};"))
      .collect::<Vec<_>>()
      .join("\n")
  };
  let entry_check_code = [check_args.trim(), option_fill_code.trim()]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("\n");

  let recur_assign_code_template = if !spreading && !has_optional {
    let mut code = String::new();
    for (idx, arg_name) in recur_arg_names.iter().enumerate() {
      if idx > 0 {
        code.push('\n');
      }
      write!(code, "{arg_name} = {{ret_var}}.args[{idx}];").expect("write");
    }
    code
  } else {
    format!("[ {args_code} ] = {{ret_var}}.args;")
  };

  let mut body: TernaryTreeList<Calcit> = TernaryTreeList::Empty;
  let mut async_prefix: String = String::from("");

  for line in raw_body {
    if let Calcit::List(xs) = line {
      let is_hint = match xs.first() {
        Some(Calcit::Syntax(sym, _ns)) => sym == &CalcitSyntax::HintFn,
        Some(Calcit::Symbol { sym, .. }) => sym.as_ref() == "hint-fn",
        _ => false,
      };
      if is_hint {
        if hinted_async(xs) {
          async_prefix = String::from("async ")
        } else if xs.len() > 1 && !xs.iter().skip(1).any(is_schema_map_form) {
          eprintln!(
            "[Warn] hint-fn args not in recognized schema map form in {}/{name}; correct usage: `hint-fn $ {{}} (:async true)`",
            passed_defs.ns
          );
        }
        continue;
      }
    }
    if line == &Calcit::Nil {
      continue;
    }
    body = body.push_right(line.to_owned());
  }

  if !body.is_empty() && uses_recur(&body[body.len() - 1]) {
    let return_var = js_gensym("return_mark");
    let body = list_to_js_code(
      &body,
      passed_defs.ns,
      local_defs,
      &format!("%%{return_var}%% ="),
      passed_defs.file_imports,
      tags,
    )?;
    let fn_def = snippets::tmpl_tail_recursion(
      /* name = */ escape_var(name),
      /* args_code = */ args_code,
      /* check_args = */ entry_check_code,
      /* spreading_code = */ spreading_code,
      /* recur_assign_code_template = */ recur_assign_code_template,
      /* body = */
      body, // dirty trick
      snippets::RecurPrefixes {
        var_prefix: var_prefix.to_owned(),
        async_prefix,
        return_mark: format!("%%{return_var}%%"),
      },
    );

    let export_mark = if exported {
      format!("export let {} = ", escape_var(name))
    } else {
      String::from("")
    };
    Ok(format!("{export_mark}{fn_def}\n"))
  } else {
    let body_code = list_to_js_code(&body, passed_defs.ns, local_defs, "return ", passed_defs.file_imports, tags)?;
    // `check_args` and `spreading_code` both contribute to the prologue; keep
    // each on its own line without leaving a leading blank line.
    let mut header_parts: Vec<&str> = vec![];
    if !entry_check_code.trim().is_empty() {
      header_parts.push(entry_check_code.trim());
    }
    if !spreading_code.trim().is_empty() {
      header_parts.push(spreading_code.trim());
    }
    let header = header_parts.join("\n");
    let full_body = if header.is_empty() {
      body_code
    } else {
      format!("{header}\n{body_code}")
    };
    let body_code = if raw_body.iter().any(contains_raw_code) {
      // Raw-code segments are emitted verbatim; indenting them could change
      // multiline template literals, so keep the body unindented.
      full_body
    } else {
      // Cheap, line-based indentation only (no AST-aware pretty-printing) so
      // generated code stays readable without adding real compile cost.
      indent_block(&full_body, "  ")
    };
    let fn_definition = format!("{}function {}({}) {{\n{}\n}}", async_prefix, escape_var(name), args_code, body_code);
    let export_mark = if exported { "export " } else { "" };
    Ok(format!("{export_mark}{fn_definition}\n"))
  }
}

/// this is a very rough implementation for now
fn hinted_async(xs: &CalcitList) -> bool {
  fn is_async_key(form: &Calcit) -> bool {
    match form {
      Calcit::Tag(tag) => tag.ref_str().trim_start_matches(':') == "async",
      Calcit::Symbol { sym, .. } => {
        let raw = sym.as_ref();
        raw == "async" || raw.trim_start_matches(':') == "async"
      }
      Calcit::Str(text) => text.as_ref() == "async",
      _ => false,
    }
  }

  fn is_truthy(form: &Calcit) -> bool {
    !matches!(form, Calcit::Nil | Calcit::Unit | Calcit::Bool(false))
  }

  fn schema_marks_async(form: &Calcit) -> bool {
    match form {
      Calcit::Map(map) => map.iter().any(|(key, value)| is_async_key(key) && is_truthy(value)),
      Calcit::List(list) => {
        let is_map_literal = matches!(list.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "{}")
          || matches!(list.first(), Some(Calcit::Proc(CalcitProc::NativeMap)));
        if !is_map_literal {
          return false;
        }

        list.iter().skip(1).any(|entry| {
          let Calcit::List(pair) = entry else {
            return false;
          };
          if pair.len() < 2 {
            return false;
          }
          match (pair.get(0), pair.get(1)) {
            (Some(key), Some(value)) => is_async_key(key) && is_truthy(value),
            _ => false,
          }
        })
      }
      _ => false,
    }
  }

  xs.iter().skip(1).any(schema_marks_async)
}

/// Returns true when a value is in the schema map form recognised by hint-fn:
/// either an already-evaluated `Calcit::Map` or a list-literal starting with
/// `{}` / `NativeMap`.  Used to distinguish valid schema annotations (which
/// should never warn) from malformed async hints.
fn is_schema_map_form(form: &Calcit) -> bool {
  match form {
    Calcit::Map(_) => true,
    Calcit::List(list) => {
      matches!(list.first(), Some(Calcit::Symbol { sym, .. }) if sym.as_ref() == "{}")
        || matches!(list.first(), Some(Calcit::Proc(CalcitProc::NativeMap)))
    }
    _ => false,
  }
}

fn extract_fn_param_types(args: &CalcitList) -> Vec<Arc<calcit::CalcitTypeAnnotation>> {
  args
    .iter()
    .filter_map(|arg| match arg {
      Calcit::Local(local) => Some(local.type_info.clone()),
      _ => None,
    })
    .collect()
}

struct PreprocessedFnParts {
  args: CalcitFnArgs,
  arg_types: Vec<Arc<calcit::CalcitTypeAnnotation>>,
  body: Vec<Calcit>,
}

fn extract_preprocessed_fn_parts(code: &Calcit) -> Result<PreprocessedFnParts, String> {
  let Calcit::List(items) = code else {
    return Err(format!("expected preprocessed defn list, got: {code}"));
  };

  match (items.first(), items.get(1), items.get(2)) {
    (
      Some(Calcit::Syntax(CalcitSyntax::Defn | CalcitSyntax::DefWasmExport | CalcitSyntax::DefWasmImport, _)),
      Some(Calcit::Symbol { .. }),
      Some(Calcit::List(args)),
    ) => {
      let raw_args = get_raw_args_fn(args)?;
      Ok(PreprocessedFnParts {
        args: raw_args,
        arg_types: extract_fn_param_types(args),
        body: items.drop_left().drop_left().drop_left().to_vec(),
      })
    }
    _ => Err(format!("expected preprocessed defn form, got: {code}")),
  }
}

pub fn emit_js(entry_ns: &str, emit_path: &str) -> Result<(), String> {
  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let mut unchanged_ns: HashSet<Arc<str>> = HashSet::new();

  let program = program::clone_compiled_program_snapshot()?;
  for (ns, file) in program.iter() {
    // println!("\nstart handling: {}\n", ns);
    // side-effects, reset tracking state

    let file_imports: RefCell<ImportsDict> = RefCell::new(ImportsDict::new());
    let collected_tags: RefCell<HashSet<EdnTag>> = RefCell::new(HashSet::new());

    let mut defs_in_current: HashSet<Arc<str>> = HashSet::new();
    for k in file.keys() {
      defs_in_current.insert(k.to_owned());
    }

    if !internal_states::is_first_compilation() {
      let app_pkg_name = entry_ns.split('.').collect::<Vec<&str>>()[0];
      let pkg_name = ns.split('.').collect::<Vec<&str>>()[0]; // TODO simpler
      if app_pkg_name != pkg_name {
        match internal_states::lookup_prev_ns_cache(ns) {
          Some(v) if v == defs_in_current => {
            // same as last time, skip
            continue;
          }
          _ => (),
        }
      }
    }
    // remember defs of each ns for comparing
    internal_states::write_as_ns_cache(ns, defs_in_current);

    // reset index each file
    reset_js_gensym_index();

    let core_lib = to_js_import_name("calcit.core", true);

    let mut defs_code = String::from(""); // code generated by functions
    let mut vals_code = String::from(""); // code generated by thunks
    let mut direct_code = String::from(""); // dirty code to run directly
    let mut tags_code = String::new();

    let mut import_code = if &**ns == "calcit.core" {
      snippets::tmpl_import_procs(wrap_js_str("@calcit/procs"))
    } else {
      format!("\nimport * as $clt from {core_lib};")
    };

    let mut def_names: HashSet<Arc<str>> = HashSet::new(); // multiple parts of scoped defs need to be tracked

    // tracking top level scope definitions
    for def in file.keys() {
      def_names.insert(def.to_owned());
    }

    let deps_in_order = sort_compiled_defs_by_deps(file);
    // println!("deps order: {:?}", deps_in_order);

    for def in deps_in_order {
      let compiled_def = file.get(&def).expect("compiled def for codegen");

      if &**ns == calcit::CORE_NS {
        if should_skip_core_def_codegen(&def, compiled_def) {
          continue;
        }
        // some defs from core can be replaced by calcit.procs
        if is_js_unavailable_procs(&def) {
          continue;
        }
        if is_preferred_js_proc(&def) {
          writeln!(defs_code, "\nvar {} = $procs.{};", escape_var(&def), escape_var(&def)).expect("write");
          continue;
        }
      }

      match &compiled_def.kind {
        // probably not work here
        program::CompiledDefKind::Proc => {
          writeln!(defs_code, "\nvar {} = $procs.{};", escape_var(&def), escape_var(&def)).expect("write");
        }
        program::CompiledDefKind::Fn => {
          let fn_parts = extract_preprocessed_fn_parts(&compiled_def.preprocessed_code)?;
          gen_stack::push_call_stack(ns, &def, StackKind::Codegen, compiled_def.preprocessed_code.to_owned(), &[]);
          let passed_defs = PassedDefs {
            ns,
            local_defs: &def_names,
            file_imports: &file_imports,
          };
          // Blank line between generated functions is a cheap, no-cost readability win.
          if !defs_code.is_empty() {
            defs_code.push('\n');
          }
          defs_code.push_str(&gen_js_func(
            &def,
            JsFnParams {
              args: &fn_parts.args,
              arg_types: &fn_parts.arg_types,
            },
            &fn_parts.body,
            &passed_defs,
            true,
            &collected_tags,
            ns,
          )?);
          gen_stack::pop_call_stack();
        }
        program::CompiledDefKind::LazyValue => {
          // TODO need topological sorting for accuracy
          // values are called directly, put them after fns
          gen_stack::push_call_stack(ns, &def, StackKind::Codegen, compiled_def.codegen_form.to_owned(), &[]);
          writeln!(
            vals_code,
            "\nexport var {} = {};",
            escape_var(&def),
            to_js_code(&compiled_def.codegen_form, ns, &def_names, &file_imports, &collected_tags, None)?
          )
          .expect("write");
          gen_stack::pop_call_stack()
        }
        // macro are not traced in codegen since already expanded
        program::CompiledDefKind::Macro => {}
        program::CompiledDefKind::Syntax => {
          // should he handled inside compiler
        }
        program::CompiledDefKind::Value if matches!(&compiled_def.codegen_form, Calcit::Bool(_) | Calcit::Number(_)) => {
          eprintln!(
            "[Warn] expected thunk, got macro. skipped `{ns}/{def} {}`",
            compiled_def.codegen_form
          )
        }
        program::CompiledDefKind::Value => {
          eprintln!("[Warn] expected thunk for js, skipped `{ns}/{def} {}`", compiled_def.codegen_form)
        }
      }
    }
    if &**ns == calcit::CORE_NS {
      // add at end of file to register builtin classes
      direct_code.push_str(&snippets::tmpl_classes_registering())
    }

    let collected_imports = file_imports.borrow();
    if !collected_imports.is_empty() {
      let mut xs = collected_imports.0.iter().to_owned().collect::<Vec<_>>();
      xs.sort();
      for item in &xs {
        // println!("import item: {:?}", item);
        match &*item.info {
          ImportInfo::NsAs { .. } => {
            let import_target = if is_cirru_string(&item.ns) {
              wrap_js_str(&item.ns[1..])
            } else {
              to_js_import_name(&item.ns, true)
            };
            write!(import_code, "\nimport * as {} from {import_target};", escape_ns(&item.ns)).expect("write");
          }
          ImportInfo::JsDefault { alias, at_ns, .. } => {
            if is_cirru_string(&item.ns) {
              let import_target = wrap_js_str(&item.ns[1..]);
              write!(import_code, "\nimport {} from {import_target};", escape_var(alias)).expect("write");
            } else {
              unreachable!("only js import leads to default ns, but got: {}", at_ns)
            }
          }
          ImportInfo::NsReferDef { .. } => {
            let import_target = if is_cirru_string(&item.ns) {
              wrap_js_str(&item.ns[1..])
            } else {
              to_js_import_name(&item.ns, true)
            };
            write!(import_code, "\nimport {{ {} }} from {import_target};", escape_var(&item.def)).expect("write");
          }
          ImportInfo::Core { at_ns } => {
            if at_ns == &item.ns {
              continue;
            }
            write!(import_code, "\nimport {{ {} }} from {core_lib};", escape_var(&item.def)).expect("write");
          }
          ImportInfo::SameFile { .. } => {
            // nothing to do
          }
        }
      }
    }

    let tag_prefix = if &**ns == "calcit.core" { "" } else { "$clt." };
    let mut tag_arr = String::from("[");
    let mut ordered_tags: Vec<EdnTag> = vec![];
    for k in collected_tags.borrow().iter() {
      ordered_tags.push(k.to_owned());
    }
    // need to maintain a stable order to reduce redundant reloads
    ordered_tags.sort();

    for s in ordered_tags {
      let name = escape_cirru_str(s.ref_str());
      write!(tag_arr, "{name},").expect("write");
    }
    tag_arr.push(']');
    tags_code.push_str(&snippets::tmpl_tags_init(&tag_arr, tag_prefix));

    let js_file_path = code_emit_path.join(to_mjs_filename(ns));
    let wrote_new = write_file_if_changed(
      &js_file_path,
      &format!("{import_code}{tags_code}\n{defs_code}\n\n{vals_code}\n{direct_code}"),
    )?;
    if wrote_new {
      println!("emitted: {}", js_file_path.to_str().expect("exptract path"));
    } else {
      unchanged_ns.insert(ns.to_owned());
    }
  }

  if !unchanged_ns.is_empty() {
    println!("\n... and {} files not changed.", unchanged_ns.len());
  }

  let _ = internal_states::finish_compilation();

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::CalcitSymbolInfo;
  use std::collections::HashMap;

  fn external_trait_with_names() -> Arc<calcit::CalcitTrait> {
    let ns = "tests.emit-js-external";
    let def = "HostElement";
    program::PROGRAM_CODE_DATA.write().expect("open program code").insert(
      Arc::from(ns),
      program::ProgramFileData {
        import_map: HashMap::new(),
        defs: HashMap::from([(
          Arc::from(def),
          program::ProgramDefEntry {
            code: Calcit::Nil,
            schema: calcit::DYNAMIC_TYPE.clone(),
            doc: Arc::from(""),
            examples: vec![],
            ffi: Some(cirru_edn::Edn::map_from_iter([
              (cirru_edn::Edn::tag("backend"), cirru_edn::Edn::tag("js")),
              (cirru_edn::Edn::tag("kind"), cirru_edn::Edn::tag("external-object")),
              (
                cirru_edn::Edn::tag("names"),
                cirru_edn::Edn::map_from_iter([(cirru_edn::Edn::tag("inner-text"), cirru_edn::Edn::str("textContent"))]),
              ),
            ])),
          },
        )]),
      },
    );
    Arc::new(calcit::CalcitTrait {
      runtime_id: None,
      definition_ref: Some(Arc::from(format!("{ns}/{def}"))),
      name: EdnTag::new(def),
      methods: Arc::new(vec![]),
      defaults: Arc::new(vec![]),
      method_types: Arc::new(vec![]),
      member_kinds: Arc::new(vec![]),
      requires: Arc::new(vec![]),
    })
  }

  fn runtime_placeholder_quote() -> Calcit {
    Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(calcit::CORE_NS)),
      symbol("&runtime-implementation"),
    ])))
  }

  fn compiled_def_for_codegen_test(kind: program::CompiledDefKind, source_code: Option<Calcit>) -> program::CompiledDef {
    program::CompiledDef {
      def_id: program::DefId(0),
      version_id: 0,
      kind,
      preprocessed_code: Calcit::Nil,
      codegen_form: Calcit::Nil,
      deps: vec![],
      type_summary: None,
      source_code,
      schema: calcit::DYNAMIC_TYPE.clone(),
      doc: Arc::from(""),
      examples: vec![],
    }
  }

  fn symbol(name: &str) -> Calcit {
    Calcit::Symbol {
      sym: Arc::from(name),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.emit-js"),
        at_def: Arc::from("hinted-async"),
      }),
      location: None,
    }
  }

  #[test]
  fn indexed_enum_match_codegen_uses_numeric_switch_dispatch() {
    let branch = |tag: &str, value: f64| Calcit::from(vec![Calcit::from(vec![Calcit::Tag(EdnTag::from(tag))]), Calcit::Number(value)]);
    let wildcard = Calcit::from(vec![symbol("_"), Calcit::Number(-1.0)]);
    let table = CalcitList::Vector(vec![branch("idle", 0.0), branch("running", 1.0), branch("done", 2.0), wildcard]);
    let local_defs = HashSet::from([Arc::from("state")]);
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let code = gen_indexed_match_code(&symbol("state"), &table, &local_defs, "tests.emit-js", &file_imports, &tags, None)
      .expect("indexed match codegen");

    assert!(code.contains("switch ("), "{code}");
    assert!(code.contains("case _t_.idle.idx:"), "{code}");
    assert!(code.contains("case _t_.running.idx:"), "{code}");
    assert!(code.contains("case _t_.done.idx:"), "{code}");
    assert!(!code.contains(" else if "), "{code}");
  }

  #[test]
  fn indexed_enum_match_codegen_wraps_non_return_contexts() {
    let branch = Calcit::from(vec![Calcit::from(vec![Calcit::Tag(EdnTag::from("idle"))]), Calcit::Number(0.0)]);
    let table = CalcitList::Vector(vec![branch, Calcit::Nil]);
    let local_defs = HashSet::from([Arc::from("state")]);
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let code = gen_indexed_match_code(
      &symbol("state"),
      &table,
      &local_defs,
      "tests.emit-js",
      &file_imports,
      &tags,
      Some("result = "),
    )
    .expect("indexed match codegen");

    assert!(code.starts_with("result = (function _fn_()"), "{code}");
    assert!(code.contains("return 0"), "{code}");
  }

  #[test]
  fn indexed_enum_match_codegen_detects_await_in_matched_value() {
    let branch = Calcit::from(vec![Calcit::from(vec![Calcit::Tag(EdnTag::from("idle"))]), Calcit::Number(0.0)]);
    let table = CalcitList::Vector(vec![branch, Calcit::Nil]);
    let value = Calcit::from(vec![symbol("js-await"), symbol("state-promise")]);
    let local_defs = HashSet::from([Arc::from("state-promise")]);
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let code = gen_indexed_match_code(&value, &table, &local_defs, "tests.emit-js", &file_imports, &tags, None)
      .expect("async indexed match codegen");

    assert!(code.starts_with("await (async function _async_fn_()"), "{code}");
    assert!(code.contains("await state_promise"), "{code}");
  }

  #[test]
  fn hinted_async_accepts_schema_map_literal() {
    let schema = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Tag(EdnTag::from("async")),
        Calcit::Bool(true),
      ]))),
    ])));
    let hint = CalcitList::from(&[Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests")), schema]);
    assert!(hinted_async(&hint));
  }

  #[test]
  fn hinted_async_ignores_false_schema_marker() {
    let schema = Calcit::List(Arc::new(CalcitList::from(&[
      symbol("{}"),
      Calcit::List(Arc::new(CalcitList::from(&[
        Calcit::Tag(EdnTag::from("async")),
        Calcit::Bool(false),
      ]))),
    ])));
    let hint = CalcitList::from(&[Calcit::Syntax(CalcitSyntax::HintFn, Arc::from("tests")), schema]);
    assert!(!hinted_async(&hint));
  }

  #[test]
  fn external_member_defaults_follow_calcit_naming_conventions() {
    assert_eq!(default_external_js_member_name("text-content"), "textContent");
    assert_eq!(default_external_js_member_name("matches?"), "matches");
    assert_eq!(default_external_js_member_name("set-item!"), "setItem");
    assert_eq!(default_external_js_member_name("!"), "!");
  }

  #[test]
  fn typed_external_field_codegen_uses_ffi_name_overrides() {
    let type_hint = Arc::new(calcit::CalcitTypeAnnotation::Trait(external_trait_with_names()));
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let get_form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Method(Arc::from("inner-text"), MethodKind::ExternalGet(type_hint.clone())),
      symbol("element"),
    ])));
    let set_form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Method(Arc::from("inner-text"), MethodKind::ExternalSet(type_hint)),
      symbol("element"),
      Calcit::Str(Arc::from("ready")),
    ])));

    assert_eq!(
      to_js_code(&get_form, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("external get should compile"),
      "element[\"textContent\"]"
    );
    assert_eq!(
      to_js_code(&set_form, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("external set should compile"),
      "(element[\"textContent\"] = \"ready\")"
    );
  }

  #[test]
  fn raw_code_body_is_kept_verbatim() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let passed_defs = PassedDefs {
      ns: "tests.emit-js",
      local_defs: &local_defs,
      file_imports: &file_imports,
    };
    let args = CalcitFnArgs::Args(vec![]);
    let raw_body = vec![Calcit::RawCode(calcit::RawCodeType::Js, Arc::from("let t = `line1\nline2`;"))];

    let code = gen_js_func(
      "demo",
      JsFnParams {
        args: &args,
        arg_types: &[],
      },
      &raw_body,
      &passed_defs,
      true,
      &tags,
      "tests.emit-js",
    )
    .expect("raw-code body should compile");
    // Multiline raw-code must stay byte-for-byte: the second line is not indented.
    assert!(code.contains("let t = `line1\nline2`;"), "raw-code changed:\n{code}");
    assert!(code.contains("\nline2`;"), "raw-code newline content indented:\n{code}");
  }

  #[test]
  fn spreading_prologue_has_no_leading_blank_line() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let passed_defs = PassedDefs {
      ns: "tests.emit-js",
      local_defs: &local_defs,
      file_imports: &file_imports,
    };
    let sym: Arc<str> = Arc::from("xs");
    let idx = CalcitLocal::track_sym(&sym);
    let args = CalcitFnArgs::MarkedArgs(vec![CalcitArgLabel::RestMark, CalcitArgLabel::Idx(idx)]);
    let raw_body = vec![Calcit::Symbol {
      sym: sym.clone(),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.emit-js"),
        at_def: Arc::from("demo"),
      }),
      location: None,
    }];

    let prev = crate::codegen::skip_arity_check();
    crate::codegen::set_code_gen_skip_arity_check(true);
    let code = gen_js_func(
      "demo",
      JsFnParams {
        args: &args,
        arg_types: &[],
      },
      &raw_body,
      &passed_defs,
      true,
      &tags,
      "tests.emit-js",
    )
    .expect("spreading body should compile");
    crate::codegen::set_code_gen_skip_arity_check(prev);

    assert!(!code.contains("{\n\n"), "no leading blank line expected:\n{code}");
    assert!(
      code.contains("{\n  xs = $clt.arrayToList(xs);"),
      "spreading should be the first indented line:\n{code}"
    );
  }

  #[test]
  fn trailing_option_params_are_filled_with_none() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let passed_defs = PassedDefs {
      ns: "tests.emit-js",
      local_defs: &local_defs,
      file_imports: &file_imports,
    };
    let first = CalcitLocal::track_sym(&Arc::from("required"));
    let second = CalcitLocal::track_sym(&Arc::from("optional"));
    let args = CalcitFnArgs::Args(vec![first, second]);
    let option_number = Arc::new(calcit::CalcitTypeAnnotation::TypeRef(
      Arc::from("Option"),
      Arc::new(vec![Arc::new(calcit::CalcitTypeAnnotation::Number)]),
    ));
    let arg_types = vec![Arc::new(calcit::CalcitTypeAnnotation::Number), option_number];
    let raw_body = vec![Calcit::Local(CalcitLocal {
      idx: second,
      sym: Arc::from("optional"),
      info: Arc::new(CalcitSymbolInfo {
        at_ns: Arc::from("tests.emit-js"),
        at_def: Arc::from("demo"),
      }),
      location: None,
      type_info: arg_types[1].clone(),
    })];

    let code = gen_js_func(
      "demo",
      JsFnParams {
        args: &args,
        arg_types: &arg_types,
      },
      &raw_body,
      &passed_defs,
      true,
      &tags,
      "tests.emit-js",
    )
    .expect("Option defaults should compile");
    assert!(
      code.contains("optional = $clt._PCT_none();"),
      "omitted Option should be initialized with %none:\n{code}"
    );
  }

  #[test]
  fn core_codegen_skips_runtime_placeholder_defs() {
    let compiled = compiled_def_for_codegen_test(program::CompiledDefKind::LazyValue, Some(runtime_placeholder_quote()));
    assert!(should_skip_core_def_codegen("range", &compiled));
  }

  #[test]
  fn core_codegen_skips_syntax_names_even_without_runtime_placeholder_source() {
    let compiled = compiled_def_for_codegen_test(program::CompiledDefKind::LazyValue, None);
    assert!(should_skip_core_def_codegen("eval", &compiled));
  }

  #[test]
  fn raw_syntax_nodes_fail_js_codegen_with_llm_hint() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let failure = to_js_code(
      &Calcit::Syntax(CalcitSyntax::If, Arc::from("tests.emit-js")),
      "tests.emit-js",
      &local_defs,
      &file_imports,
      &tags,
      None,
    )
    .expect_err("raw syntax should be rejected in JS codegen");

    assert!(failure.contains("raw syntax node `if`"), "unexpected error: {failure}");
    assert!(failure.contains("LLM hint"), "unexpected error: {failure}");
  }

  #[test]
  fn reset_syntax_call_codegen_uses_runtime_proc() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Reset, Arc::from("tests.emit-js")),
      symbol("state"),
      Calcit::Number(1.0),
    ])));

    let code = to_js_code(&form, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("reset! should compile");

    assert_eq!(code, "$clt.reset_$x_(state, 1)");
  }

  #[test]
  fn eval_syntax_call_codegen_uses_runtime_proc() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Eval, Arc::from("tests.emit-js")),
      symbol("code"),
    ])));

    let code = to_js_code(&form, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("eval should compile");

    assert_eq!(code, "$clt.eval(code)");
  }

  #[test]
  fn bare_empty_map_uses_the_runtime_constructor() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let code = to_js_code(
      &Calcit::Proc(CalcitProc::NativeMap),
      "tests.emit-js",
      &local_defs,
      &file_imports,
      &tags,
      None,
    )
    .expect("bare empty map should compile");

    assert_eq!(code, "$clt._$n__$M_()");
  }

  #[test]
  fn explicit_unit_codegen_is_distinct_from_nil() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());

    let unit = to_js_code(&Calcit::Unit, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("unit should compile");
    let nil = to_js_code(&Calcit::Nil, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("nil should compile");

    assert_eq!(unit, "void 0");
    assert_eq!(nil, "null");
  }

  #[test]
  fn map_literal_with_entries_keeps_the_runtime_constructor_as_callee() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let form = Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Proc(CalcitProc::NativeMap),
      Calcit::Tag(EdnTag::from("value")),
      Calcit::Number(1.0),
    ])));

    let code = to_js_code(&form, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("map literal should compile");

    assert_eq!(code, "$clt._$n__$M_(_t_.value, 1)");
  }

  #[test]
  fn typed_struct_codegen_keeps_precomputed_indices() {
    let local_defs: HashSet<Arc<str>> = HashSet::new();
    let file_imports = RefCell::new(ImportsDict::new());
    let tags = RefCell::new(HashSet::new());
    let nth = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeStructNth),
      symbol("person"),
      Calcit::Number(0.0),
      Calcit::Tag(EdnTag::from("name")),
    ]);
    let assoc = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeStructAssocAt),
      symbol("person"),
      Calcit::Number(0.0),
      Calcit::Tag(EdnTag::from("name")),
      Calcit::Str(Arc::from("Ada")),
    ]);
    let with = Calcit::from(vec![
      Calcit::Proc(CalcitProc::NativeStructWithAt),
      symbol("person"),
      Calcit::Number(0.0),
      Calcit::Tag(EdnTag::from("name")),
      Calcit::Str(Arc::from("Ada")),
      Calcit::Number(1.0),
      Calcit::Tag(EdnTag::from("score")),
      Calcit::Number(9.0),
    ]);

    assert_eq!(
      to_js_code(&nth, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("indexed read should compile"),
      "person.nthAt(0, _t_.name)"
    );
    assert_eq!(
      to_js_code(&assoc, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("indexed assoc should compile"),
      "person.assocAt(0, _t_.name, \"Ada\")"
    );
    assert_eq!(
      to_js_code(&with, "tests.emit-js", &local_defs, &file_imports, &tags, None).expect("indexed batch update should compile"),
      "person.withAt(0, _t_.name, \"Ada\", 1, _t_.score, 9)"
    );
  }
}

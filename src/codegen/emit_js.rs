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
      | "record?"
      | "tuple?"
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
  matches!(value, Calcit::Symbol { sym, .. } if sym.as_ref() == "&runtime-inplementation")
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
      Calcit::Syntax(s, ..) => {
        let proc_prefix = get_proc_prefix(ns);
        Ok(format!("{proc_prefix}{}", escape_var(s.as_ref())))
      }
      Calcit::Str(s) => Ok(escape_cirru_str(s)),
      Calcit::Bool(b) => Ok(b.to_string()),
      Calcit::Number(n) => Ok(n.to_string()),
      Calcit::Nil => Ok(String::from("null")),
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
              &var_prefix, &ref_path, &var_prefix, &ref_path
            ))
          }
          (_, _) => Err(format!("defatom expected name and value, got: {body}")),
        },

        CalcitSyntax::Defn => match (body.first(), body.get(1)) {
          (Some(Calcit::Symbol { sym, .. }), Some(Calcit::List(ys))) => {
            let func_body = body.skip(2)?;
            gen_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &[]);
            let passed_defs = PassedDefs {
              ns,
              local_defs,
              file_imports,
            };
            let ret = gen_js_func(sym, &get_raw_args_fn(ys)?, &func_body.to_vec(), &passed_defs, false, tags, ns);
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
        // for `&call-spread`, just translate as normal call
        CalcitSyntax::CallSpread => gen_call_code(&body, ns, local_defs, xs, file_imports, tags, return_label),
        CalcitSyntax::HintFn => Ok(format!("{return_code}null")),
        CalcitSyntax::AssertType => Ok(format!("{return_code}null")),
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
          let ret = format!("let {err_var} = new Error({message});\n {err_var}.data = {data_code};\n throw {err_var};");
          // println!("inside raise: {:?} {}", return_label, xs);
          match return_label {
            Some(_) => Ok(ret),
            _ => Ok(make_fn_wrapper(&ret, has_await)),
          }
        }
        None => Err(format!("raise expected 1~2 arguments, got: {body}")),
      }
    }
    Calcit::Proc(_) => {
      let (prelude, args_code) =
        gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
      let call_code = format!("{}({})", to_js_code(&head, ns, local_defs, file_imports, tags, None)?, args_code);
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
    },
    _ => {
      let (prelude, args_code) =
        gen_call_args_with_temps(&body, ns, local_defs, file_imports, tags, return_label.is_some(), inline_all)?;
      let call_code = format!("{}({})", to_js_code(&head, ns, local_defs, file_imports, tags, None)?, args_code);
      Ok(wrap_call_with_prelude(prelude, call_code, return_label, detect_await(&body)))
    }
  }
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
  for x in xs {
    match x {
      Calcit::List(al) => {
        if let Some(Calcit::Syntax(CalcitSyntax::Defn, _s)) = al.get(0) {
          // a nested function has its own scope deciding if it's async
          return false;
        } else if detect_await(al) {
          return true;
        }
      }
      Calcit::Symbol { sym, .. } => {
        if &**sym == "js-await" {
          return true;
        }
      }
      _ => {}
    }
  }
  false
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
          _ => return Err(format!("Expected symbol in &let binding, got: {}", &pair)),
        }
      }
      Calcit::List(_xs) => return Err(format!("expected pair of length 2, got: {}", &pair)),
      _ => return Err(format!("expected pair of a list of length 2, got: {pair}")),
    }
  }
  if base_return_label.is_some() {
    Ok(format!("{defs_code}{body_part}"))
  } else {
    Ok(make_fn_wrapper(&format!("{defs_code}{body_part}"), has_await))
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
      result.push_str(&line);
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
      Some(Calcit::Syntax(CalcitSyntax::Defn, _)) => false,
      Some(Calcit::Symbol { sym, .. }) if &**sym == "defn" => false,
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

fn gen_js_func(
  name: &str,
  args: &CalcitFnArgs,
  raw_body: &[Calcit],
  passed_defs: &PassedDefs,
  exported: bool,
  tags: &RefCell<HashSet<EdnTag>>,
  at_ns: &str,
) -> Result<String, String> {
  let var_prefix = if passed_defs.ns == "calcit.core" { "" } else { "$clt." };
  let mut local_defs = passed_defs.local_defs.to_owned();
  let mut spreading_code = String::from(""); // js list and calcit-js list are different, need to convert
  let mut args_code = String::from("");
  let mut spreading = false;
  let mut has_optional = false;
  let mut args_count = 0;
  let mut optional_count = 0;
  let mut recur_arg_names: Vec<String> = vec![];

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
            args_code.push_str(&escape_var(&sym));
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
  } else if has_optional {
    snippets::tmpl_args_between(name, args_count, args_count + optional_count, at_ns)
  } else {
    snippets::tmpl_args_exact(name, args_count, at_ns)
  };

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
      /* check_args = */ check_args,
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
    let fn_definition = format!(
      "{}function {}({}) {{ {}{}\n{}}}",
      async_prefix,
      escape_var(name),
      args_code,
      check_args,
      spreading_code,
      list_to_js_code(&body, passed_defs.ns, local_defs, "return ", passed_defs.file_imports, tags)?
    );
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
    !matches!(form, Calcit::Nil | Calcit::Bool(false))
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

fn extract_preprocessed_fn_parts(code: &Calcit) -> Result<(CalcitFnArgs, Vec<Calcit>), String> {
  let Calcit::List(items) = code else {
    return Err(format!("expected preprocessed defn list, got: {code}"));
  };

  match (items.first(), items.get(1), items.get(2)) {
    (Some(Calcit::Syntax(CalcitSyntax::Defn, _)), Some(Calcit::Symbol { .. }), Some(Calcit::List(args))) => {
      let raw_args = get_raw_args_fn(args)?;
      Ok((raw_args, items.drop_left().drop_left().drop_left().to_vec()))
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
          let (raw_args, raw_body) = extract_preprocessed_fn_parts(&compiled_def.preprocessed_code)?;
          gen_stack::push_call_stack(ns, &def, StackKind::Codegen, compiled_def.preprocessed_code.to_owned(), &[]);
          let passed_defs = PassedDefs {
            ns,
            local_defs: &def_names,
            file_imports: &file_imports,
          };
          defs_code.push_str(&gen_js_func(&def, &raw_args, &raw_body, &passed_defs, true, &collected_tags, ns)?);
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

  fn runtime_placeholder_quote() -> Calcit {
    Calcit::List(Arc::new(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from(calcit::CORE_NS)),
      symbol("&runtime-inplementation"),
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
  fn core_codegen_skips_runtime_placeholder_defs() {
    let compiled = compiled_def_for_codegen_test(program::CompiledDefKind::LazyValue, Some(runtime_placeholder_quote()));
    assert!(should_skip_core_def_codegen("range", &compiled));
  }

  #[test]
  fn core_codegen_skips_syntax_names_even_without_runtime_placeholder_source() {
    let compiled = compiled_def_for_codegen_test(program::CompiledDefKind::LazyValue, None);
    assert!(should_skip_core_def_codegen("eval", &compiled));
  }
}

pub mod gen_stack;
mod internal_states;
use std::fmt::Write;
mod snippets;

use cirru_parser::Cirru;
use im_ternary_tree::TernaryTreeList;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cirru_edn::EdnKwd;

use crate::builtins::meta::{js_gensym, reset_js_gensym_index};
use crate::builtins::syntax::get_raw_args;
use crate::builtins::{is_js_syntax_procs, is_proc_name};
use crate::call_stack::StackKind;
use crate::primes::{self, CalcitProc, MethodKind};
use crate::primes::{Calcit, CalcitItems, CalcitSyntax, ImportRule, SymbolResolved::*};
use crate::program;
use crate::util::string::{has_ns_part, matches_js_var, wrap_js_str};

type ImportsDict = BTreeMap<Arc<str>, ImportedTarget>;

#[derive(Debug, PartialEq, Clone, Eq)]
pub enum ImportedTarget {
  AsNs(Arc<str>),
  DefaultNs(Arc<str>),
  ReferNs(Arc<str>),
}

fn to_js_import_name(ns: &str, mjs_mode: bool) -> String {
  let mut xs: String = String::from("./");
  xs.push_str(ns);
  if mjs_mode {
    xs.push_str(".mjs");
  }
  // currently use `import "./ns.name"`
  wrap_js_str(&xs)
}

fn to_mjs_filename(ns: &str) -> String {
  let mut xs: String = String::from(ns);
  xs.push_str(".mjs");
  xs
}

fn escape_var(name: &str) -> String {
  if has_ns_part(name) {
    unreachable!("Invalid variable name `{}`, use `escape_ns_var` instead", name);
  }
  match name {
    "if" => String::from("_IF_"),
    "do" => String::from("_DO_"),
    "else" => String::from("_ELSE_"),
    "let" => String::from("_LET_"),
    "case" => String::from("_CASE_"),
    "-" => String::from("_SUB_"),
    _ => name
      .replace('-', "_")
      // dot might be part of variable `\.`. not confused with syntax
      .replace('.', "_DOT_")
      .replace('?', "_$q_")
      .replace('+', "_ADD_")
      .replace('^', "_CRT_")
      .replace('*', "_$s_")
      .replace('&', "_$n_")
      .replace("{}", "_$M_")
      .replace("[]", "_$L_")
      .replace('{', "_CURL_")
      .replace('}', "_CURR_")
      .replace('\'', "_SQUO_")
      .replace('[', "_SQRL_")
      .replace(']', "_SQRR_")
      .replace('!', "_$x_")
      .replace('%', "_PCT_")
      .replace('/', "_SLSH_")
      .replace('=', "_$e_")
      .replace('>', "_GT_")
      .replace('<', "_LT_")
      .replace(':', "_$o_")
      .replace(';', "_SCOL_")
      .replace('#', "_SHA_")
      .replace('\\', "_BSL_"),
  }
}

fn escape_ns(name: &str) -> String {
  // use `$` to tell namespace from normal variables, thus able to use same token like clj
  let piece = if is_cirru_string(name) {
    name[1..].to_owned() // TODO
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
      | "keyword?"
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

fn escape_cirru_str(s: &str) -> String {
  let mut result = String::from('"');
  for c in s.chars() {
    match c {
      // disabled since not sure if useful for Cirru
      // of '\0'..'\31', '\127'..'\255':
      //   add(result, "\\x")
      //   add(result, toHex(ord(c), 2))
      '\\' => result.push_str("\\\\"),
      '\"' => result.push_str("\\\""),
      '\n' => result.push_str("\\n"),
      '\t' => result.push_str("\\t"),
      _ => result.push(c),
    }
  }
  result.push('"');
  result
}

fn quote_to_js(xs: &Calcit, var_prefix: &str, keywords: &RefCell<HashSet<EdnKwd>>) -> Result<String, String> {
  match xs {
    Calcit::Symbol { sym, .. } => Ok(format!("new {var_prefix}CalcitSymbol({})", escape_cirru_str(sym))),
    Calcit::Str(s) => Ok(escape_cirru_str(s)),
    Calcit::Bool(b) => Ok(b.to_string()),
    Calcit::Number(n) => Ok(n.to_string()),
    Calcit::Nil => Ok(String::from("null")),
    // mainly for methods, which are recognized during reading
    Calcit::Proc(p) => Ok(format!("new {var_prefix}CalcitSymbol({})", escape_cirru_str(&p.to_string()))),
    Calcit::List(ys) => {
      let mut chunk = String::from("");
      for y in ys {
        if !chunk.is_empty() {
          chunk.push_str(", ");
        }
        chunk.push_str(&quote_to_js(y, var_prefix, keywords)?);
      }
      Ok(format!("new {var_prefix}CalcitSliceList([{chunk}])"))
    }
    Calcit::Keyword(s) => {
      let mut kwds = keywords.borrow_mut();
      kwds.insert(s.to_owned());
      Ok(format!("_kwd[{}]", escape_cirru_str(&s.to_string())))
    }
    Calcit::CirruQuote(code) => Ok(format!("new {var_prefix}CalcitCirruQuote({})", cirru_to_js(code)?)),
    Calcit::Method(name, kind) => {
      let code = match kind {
        MethodKind::Access => ".-",
        MethodKind::InvokeNative => ".!",
        MethodKind::Invoke => ".",
      };
      Ok(format!("new {var_prefix}CalcitSymbol(\"{code}{}\")", name.escape_default()))
    }
    _ => unreachable!("Unexpected data in quote for js: {}", xs),
  }
}

fn make_let_with_bind(left: &str, right: &str, body: &str) -> String {
  format!("(function __bind__({left}){{\n{body} }})({right})")
}

fn make_let_with_wrapper(left: &str, right: &str, body: &str) -> String {
  format!("(function __let__(){{ \nlet {left} = {right};\n {body} }})()")
}

fn make_fn_wrapper(body: &str) -> String {
  format!("(function __fn__(){{\n{body}\n}})()")
}

fn to_js_code(
  xs: &Calcit,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  file_imports: &RefCell<ImportsDict>,
  keywords: &RefCell<HashSet<EdnKwd>>,
  return_label: Option<&str>,
) -> Result<String, String> {
  if let Calcit::List(ys) = xs {
    gen_call_code(ys, ns, local_defs, xs, file_imports, keywords, return_label)
  } else {
    let ret = match xs {
      Calcit::Symbol {
        sym,
        ns: def_ns,
        at_def,
        resolved,
        ..
      } => {
        let resolved_info = resolved.to_owned().map(|v| (*v).to_owned());
        let passed_defs = PassedDefs {
          ns,
          local_defs,
          file_imports,
        };

        gen_symbol_code(sym, def_ns, at_def, resolved_info, xs, &passed_defs)
      }
      Calcit::Proc(s) => {
        let proc_prefix = get_proc_prefix(ns);
        // println!("gen proc {} under {}", s, ns,);
        // let resolved = Some(ResolvedDef(String::from(primes::CORE_NS), s.to_owned()));
        // gen_symbol_code(s, primes::CORE_NS, &resolved, ns, xs, local_defs)
        Ok(format!("{proc_prefix}{}", escape_var(&s.to_string())))
      }
      Calcit::Method(name, kind) => {
        let proc_prefix = get_proc_prefix(ns);
        if *kind == MethodKind::Invoke {
          Ok(format!("{proc_prefix}invoke_method_closure({})", escape_cirru_str(name)))
        } else {
          Err(format!("Does not expect native method as closure: {kind}"))
        }
      }
      Calcit::Syntax(s, ..) => {
        let proc_prefix = get_proc_prefix(ns);
        Ok(format!("{proc_prefix}{}", escape_var(&s.to_string())))
      }
      Calcit::Str(s) => Ok(escape_cirru_str(s)),
      Calcit::Bool(b) => Ok(b.to_string()),
      Calcit::Number(n) => Ok(n.to_string()),
      Calcit::Nil => Ok(String::from("null")),
      Calcit::Keyword(s) => {
        let mut kwds = keywords.borrow_mut();
        kwds.insert(s.to_owned());
        Ok(format!("_kwd[{}]", wrap_js_str(&s.to_string())))
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

fn gen_call_code(
  ys: &CalcitItems,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  xs: &Calcit,
  file_imports: &RefCell<ImportsDict>,
  keywords: &RefCell<HashSet<EdnKwd>>,
  return_label: Option<&str>,
) -> Result<String, String> {
  let return_code = return_label.unwrap_or("");
  let var_prefix = if ns == primes::CORE_NS { "" } else { "$calcit." };
  let proc_prefix = get_proc_prefix(ns);
  if ys.is_empty() {
    println!("[Warn] Unexpected empty list inside {xs}");
    return Ok(String::from("()"));
  }

  let head = ys[0].to_owned();
  let body = ys.drop_left();
  match &head {
    Calcit::Syntax(s, ..) => {
      match s {
        CalcitSyntax::If => {
          if let Some(Calcit::List(ys)) = body.get(2) {
            if let Some(Calcit::Syntax(syn, ..)) = ys.get(0) {
              if syn == &CalcitSyntax::If {
                return gen_if_code(&body, local_defs, xs, ns, file_imports, keywords, return_label);
              }
            }
          }
          if return_label.is_some() {
            return gen_if_code(&body, local_defs, xs, ns, file_imports, keywords, return_label);
          }
          return match (body.get(0), body.get(1)) {
            (Some(condition), Some(true_branch)) => {
              gen_stack::push_call_stack(ns, "if", StackKind::Codegen, xs.to_owned(), &TernaryTreeList::Empty);
              let false_code = match body.get(2) {
                Some(fal) => to_js_code(fal, ns, local_defs, file_imports, keywords, None)?,
                None => String::from("null"),
              };
              let cond_code = to_js_code(condition, ns, local_defs, file_imports, keywords, None)?;
              let true_code = to_js_code(true_branch, ns, local_defs, file_imports, keywords, None)?;
              gen_stack::pop_call_stack();
              Ok(format!("{return_code}( {cond_code} ? {true_code} : {false_code} )"))
            }
            (_, _) => Err(format!("if expected 2~3 nodes, got: {body:?}")),
          };
        }
        CalcitSyntax::CoreLet => gen_let_code(&body, local_defs, xs, ns, file_imports, keywords, return_label),

        CalcitSyntax::Quote => match body.get(0) {
          Some(item) => quote_to_js(item, var_prefix, keywords),
          None => Err(format!("quote expected a node, got nothing from {body:?}")),
        },
        CalcitSyntax::Defatom => {
          match (body.get(0), body.get(1)) {
            _ if body.len() > 2 => Err(format!("defatom expected name and value, got too many: {body:?}")),
            (Some(Calcit::Symbol { sym, .. }), Some(v)) => {
              // let _name = escape_var(sym); // TODO
              let ref_path = wrap_js_str(&format!("{ns}/{sym}"));
              gen_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &TernaryTreeList::Empty);
              let value_code = &to_js_code(v, ns, local_defs, file_imports, keywords, None)?;
              gen_stack::pop_call_stack();
              Ok(format!(
                "\n({}peekDefatom({}) ?? {}defatom({}, {value_code}))\n",
                &var_prefix, &ref_path, &var_prefix, &ref_path
              ))
            }
            (_, _) => Err(format!("defatom expected name and value, got: {body:?}")),
          }
        }

        CalcitSyntax::Defn => match (body.get(0), body.get(1)) {
          (Some(Calcit::Symbol { sym, .. }), Some(Calcit::List(ys))) => {
            let func_body = body.skip(2)?;
            gen_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &TernaryTreeList::Empty);
            let passed_defs = PassedDefs {
              ns,
              local_defs,
              file_imports,
            };
            let ret = gen_js_func(sym, &get_raw_args(ys)?, &func_body, &passed_defs, false, keywords);
            gen_stack::pop_call_stack();
            match ret {
              Ok(code) => Ok(format!("{return_code}{code}")),
              _ => ret,
            }
          }
          (_, _) => Err(format!("defn expected name arguments, got: {body:?}")),
        },

        CalcitSyntax::Defmacro => Ok(format!("/* Unexpected macro {xs} */")),
        CalcitSyntax::Quasiquote => Ok(format!("(/* Unexpected quasiquote {} */ null)", xs.lisp_str())),
        CalcitSyntax::Try => match (body.get(0), body.get(1)) {
          (Some(expr), Some(handler)) => {
            gen_stack::push_call_stack(ns, "try", StackKind::Codegen, xs.to_owned(), &TernaryTreeList::Empty);
            let next_return_label = return_label.unwrap_or("return ");
            let try_code = to_js_code(expr, ns, local_defs, file_imports, keywords, Some(next_return_label))?;
            let err_var = js_gensym("errMsg");
            let handler = to_js_code(handler, ns, local_defs, file_imports, keywords, None)?;

            gen_stack::pop_call_stack();
            let code = snippets::tmpl_try(err_var, try_code, handler, next_return_label);
            match return_label {
              Some(_) => Ok(code),
              None => Ok(snippets::tmpl_fn_wrapper(code)),
            }
          }
          (_, _) => Err(format!("try expected 2 nodes, got {body:?}")),
        },
        _ => {
          let args_code = gen_args_code(&body, ns, local_defs, file_imports, keywords)?;
          Ok(format!(
            "{}{}({})",
            return_code,
            to_js_code(&head, ns, local_defs, file_imports, keywords, None)?,
            args_code
          ))
        }
      }
    }
    Calcit::Proc(p) => {
      match p {
        CalcitProc::Raise => {
          // not core syntax, but treat as macro for better debugging experience
          match body.get(0) {
            Some(m) => {
              let message: String = to_js_code(m, ns, local_defs, file_imports, keywords, None)?;
              let data_code = match body.get(1) {
                Some(d) => to_js_code(d, ns, local_defs, file_imports, keywords, None)?,
                None => String::from("null"),
              };
              let err_var = js_gensym("err");
              let ret = format!(
                "let {} = new Error({});\n {}.data = {};\n throw {};",
                err_var, message, err_var, data_code, err_var
              );
              // println!("inside raise: {:?} {}", return_label, xs);
              match return_label {
                Some(_) => Ok(ret),
                _ => Ok(make_fn_wrapper(&ret)),
              }
            }
            None => Err(format!("raise expected 1~2 arguments, got {body:?}")),
          }
        }
        _ => Err(format!("unexpected proc: {p}")),
      }
    }
    Calcit::Symbol { sym: s, .. } => {
      match &**s {
        ";" => Ok(format!("(/* {} */ null)", Calcit::List(body))),

        "echo" | "println" => {
          // not core syntax, but treat as macro for better debugging experience
          let args = ys.drop_left();
          let args_code = gen_args_code(&args, ns, local_defs, file_imports, keywords)?;
          Ok(format!("console.log({proc_prefix}printable({args_code}))"))
        }
        "eprintln" => {
          // not core syntax, but treat as macro for better debugging experience
          let args = ys.drop_left();
          let args_code = gen_args_code(&args, ns, local_defs, file_imports, keywords)?;
          Ok(format!("console.error({proc_prefix}printable({args_code}))"))
        }
        "exists?" => {
          // not core syntax, but treat as macro for availability
          match body.get(0) {
            Some(Calcit::Symbol { .. }) | Some(Calcit::RawCode(..)) => {
              let target = to_js_code(&body[0], ns, local_defs, file_imports, keywords, None)?; // TODO could be simpler
              Ok(format!("{return_code}(typeof {target} !== 'undefined')"))
            }
            Some(a) => Err(format!("exists? expected a symbol, got {a}")),
            None => Err(format!("exists? expected 1 node, got {body:?}")),
          }
        }
        "new" => match body.get(0) {
          Some(ctor) => {
            let args = body.drop_left();
            let args_code = gen_args_code(&args, ns, local_defs, file_imports, keywords)?;
            Ok(format!(
              "{}new {}({})",
              return_code,
              to_js_code(ctor, ns, local_defs, file_imports, keywords, None)?,
              args_code
            ))
          }
          None => Err(format!("`new` expected constructor, got nothing, {xs}")),
        },
        "js-await" => match body.get(0) {
          Some(body) => Ok(format!(
            "(await {})",
            to_js_code(body, ns, local_defs, file_imports, keywords, None)?,
          )),
          None => Err(format!("`new` expected constructor, got nothing, {xs}")),
        },
        "instance?" => match (body.get(0), body.get(1)) {
          (Some(ctor), Some(v)) => Ok(format!(
            "{}({} instanceof {})",
            return_code,
            to_js_code(v, ns, local_defs, file_imports, keywords, None)?,
            to_js_code(ctor, ns, local_defs, file_imports, keywords, None)?
          )),
          (_, _) => Err(format!("instance? expected 2 arguments, got {body:?}")),
        },
        "set!" => match (body.get(0), body.get(1)) {
          (Some(target), Some(v)) => Ok(format!(
            "{} = {}",
            to_js_code(target, ns, local_defs, file_imports, keywords, None)?,
            to_js_code(v, ns, local_defs, file_imports, keywords, None)?
          )),
          (_, _) => Err(format!("set! expected 2 nodes, got {body:?}")),
        },
        _ => {
          // TODO
          let args_code = gen_args_code(&body, ns, local_defs, file_imports, keywords)?;
          Ok(format!(
            "{}{}({})",
            return_code,
            to_js_code(&head, ns, local_defs, file_imports, keywords, None)?,
            args_code
          ))
        }
      }
    }
    Calcit::Method(name, kind) => match kind {
      MethodKind::Access => {
        if body.len() == 1 {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, keywords, None)?;
          if matches_js_var(name) {
            Ok(format!("{return_code}{obj}.{name}"))
          } else {
            Ok(format!("{return_code}{obj}[{}]", escape_cirru_str(name)))
          }
        } else {
          Err(format!("accessor takes only 1 argument, {xs:?}"))
        }
      }
      MethodKind::InvokeNative => {
        if !body.is_empty() {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, keywords, None)?;
          let args_code = gen_args_code(&body.skip(1).expect("get args"), ns, local_defs, file_imports, keywords)?;

          let caller = if matches_js_var(name) {
            format!("{obj}.{name}")
          } else {
            format!("{obj}[{}]", escape_cirru_str(name))
          };
          Ok(format!("{return_code}{caller}({args_code})"))
        } else {
          Err(format!("expected at least 1 object, got {xs}"))
        }
      }
      MethodKind::Invoke => {
        let proc_prefix = get_proc_prefix(ns);
        if !body.is_empty() {
          let obj = to_js_code(&body[0], ns, local_defs, file_imports, keywords, None)?;
          let args_code = gen_args_code(&body.skip(1).expect("get args"), ns, local_defs, file_imports, keywords)?;

          Ok(format!(
            "{}{}invoke_method({},{},{})",
            return_code,
            proc_prefix,
            escape_cirru_str(name),
            obj,
            args_code
          ))
        } else {
          Err(format!("expected at least 1 object, got {xs}"))
        }
      }
    },
    _ => {
      let args_code = gen_args_code(&body, ns, local_defs, file_imports, keywords)?;
      Ok(format!(
        "{}{}({})",
        return_code,
        to_js_code(&head, ns, local_defs, file_imports, keywords, None)?,
        args_code
      ))
    }
  }
}

/// a group of arguments related to scopes
struct PassedDefs<'a> {
  ns: &'a str,
  local_defs: &'a HashSet<Arc<str>>,
  file_imports: &'a RefCell<ImportsDict>,
}

fn gen_symbol_code(
  s: &str,
  def_ns: &str,
  at_def: &str,
  resolved: Option<primes::SymbolResolved>,
  xs: &Calcit,
  passed_defs: &PassedDefs,
) -> Result<String, String> {
  // println!("gen symbol: {} {} {} {:?}", s, def_ns, ns, resolved);
  let var_prefix = if passed_defs.ns == primes::CORE_NS { "" } else { "$calcit." };
  if has_ns_part(s) {
    let pieces = s.split('/').collect::<Vec<&str>>();
    let ns_part = pieces[0];
    let def_part = pieces[1];
    // TODO dirty code
    // TODO namespace part supposed be parsed during preprocessing, this mimics old behaviors
    match resolved {
      Some(ResolvedDef {
        ns: r_ns,
        def: _r_def,
        rule: _import_rule, /* None */
      }) => {
        if is_cirru_string(&r_ns) {
          track_ns_import(ns_part, ImportedTarget::AsNs(r_ns), passed_defs.file_imports)?;
          // Ok(escape_ns_var(s, ns_part))
          Ok(format!("{}.{}", escape_ns(ns_part), escape_var(def_part)))
        } else {
          track_ns_import(&r_ns, ImportedTarget::AsNs(r_ns.to_owned()), passed_defs.file_imports)?;
          // Ok(escape_ns_var(s, &r_ns))
          Ok(format!("{}.{}", escape_ns(&r_ns), escape_var(def_part)))
        }
      }
      Some(ResolvedRaw) => Err(format!("not going to generate from raw symbol, {s}")),
      Some(ResolvedLocal) => Err(format!("symbol with ns should not be local, {s}")),
      None => Err(format!("expected symbol with ns being resolved: {xs:?}")),
    }
  } else if is_js_syntax_procs(s) || is_proc_name(s) || CalcitSyntax::is_valid(s) {
    // return Ok(format!("{}{}", var_prefix, escape_var(s)));
    let proc_prefix = get_proc_prefix(passed_defs.ns);
    Ok(format!("{proc_prefix}{}", escape_var(s)))
  } else if matches!(resolved, Some(ResolvedLocal)) || passed_defs.local_defs.contains(s) {
    Ok(escape_var(s))
  } else if let Some(ResolvedDef {
    ns: r_ns,
    def: _r_def,
    rule: import_rule,
  }) = resolved.to_owned()
  {
    if &*r_ns == primes::CORE_NS {
      // functions under core uses built $calcit module entry
      return Ok(format!("{var_prefix}{}", escape_var(s)));
    }
    if let Some(ImportRule::NsDefault(_s)) = import_rule.map(|x| (*x).to_owned()) {
      // imports that using :default are special
      track_ns_import(s, ImportedTarget::DefaultNs(r_ns), passed_defs.file_imports)?;
    } else {
      track_ns_import(s, ImportedTarget::ReferNs(r_ns), passed_defs.file_imports)?;
    }
    Ok(escape_var(s))
  } else if def_ns == primes::CORE_NS {
    // local variales inside calcit.core also uses this ns
    println!("[Warn] detected variable inside core not resolved");
    Ok(format!("{var_prefix}{}", escape_var(s)))
  } else if def_ns.is_empty() {
    Err(format!("Unexpected ns at symbol, {xs:?}"))
  } else if def_ns != passed_defs.ns {
    track_ns_import(s, ImportedTarget::ReferNs(def_ns.into()), passed_defs.file_imports)?;

    // probably via macro
    // TODO dirty code collecting imports

    Ok(escape_var(s))
  } else if def_ns == passed_defs.ns {
    println!("[Warn] detected unresolved variable `{s}` in {}/{at_def}", passed_defs.ns);
    Ok(escape_var(s))
  } else {
    println!("[Warn] Unexpected case, code gen for `{s}` in {}/{at_def}", passed_defs.ns);
    Ok(format!("{var_prefix}{}", escape_var(s)))
  }
}

// track but compare first, return Err if a different one existed
fn track_ns_import(sym: &str, import_rule: ImportedTarget, file_imports: &RefCell<ImportsDict>) -> Result<(), String> {
  let mut dict = file_imports.borrow_mut();
  match dict.get(&Arc::from(sym.to_owned())) {
    Some(v) => {
      if *v == import_rule {
        Ok(())
      } else {
        Err(format!("conflicted import rule, previous {v:?}, now {import_rule:?}"))
      }
    }
    None => {
      dict.insert(sym.to_owned().into(), import_rule);
      Ok(())
    }
  }
}

fn gen_let_code(
  body: &CalcitItems,
  local_defs: &HashSet<Arc<str>>,
  xs: &Calcit,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
  keywords: &RefCell<HashSet<EdnKwd>>,
  base_return_label: Option<&str>,
) -> Result<String, String> {
  let mut let_def_body = body.to_owned();
  let return_label = base_return_label.unwrap_or("return ");

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

    match &pair {
      Calcit::Nil => {
        // non content defs_code

        for (idx, x) in content.into_iter().enumerate() {
          if idx == content.len() - 1 {
            body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, Some(return_label))?);
            body_part.push('\n');
          } else {
            body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, None)?);
            body_part.push_str(";\n");
          }
        }
        break;
      }
      Calcit::List(xs) if xs.len() == 2 => {
        let def_name = xs[0].to_owned();
        let def_code = xs[1].to_owned();

        match def_name {
          Calcit::Symbol { sym, .. } => {
            // TODO `let` inside expressions makes syntax error
            let left = escape_var(&sym);
            let right = to_js_code(&def_code, ns, &scoped_defs, file_imports, keywords, None)?;
            writeln!(defs_code, "let {left} = {right};").expect("write");

            if scoped_defs.contains(&sym) {
              for (idx, x) in content.into_iter().enumerate() {
                if idx == content.len() - 1 {
                  // normally, last item of function body returns as return value(even in recursion)
                  if local_defs.contains(&sym) {
                    // however, to shallow a conflicted variable, we need to return explicitly
                    body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, Some("return "))?);
                  } else {
                    body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, Some(return_label))?);
                  }
                  body_part.push('\n');
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, None)?);
                  body_part.push_str(";\n");
                }
              }

              // first variable is using conflicted name
              let ret = if local_defs.contains(&sym) {
                make_let_with_bind(&left, &right, &body_part)
              } else {
                make_let_with_wrapper(&left, &right, &body_part)
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
                    (Calcit::Syntax(sym, _ns), Calcit::List(zs)) if sym == &CalcitSyntax::CoreLet && zs.len() == 2 => match &zs[0] {
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

              for (idx, x) in content.into_iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, Some(return_label))?);
                  body_part.push('\n');
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports, keywords, None)?);
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
    Ok(make_fn_wrapper(&format!("{defs_code}{body_part}")))
  }
}

fn gen_if_code(
  body: &CalcitItems,
  local_defs: &HashSet<Arc<str>>,
  _xs: &Calcit,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
  keywords: &RefCell<HashSet<EdnKwd>>,
  base_return_label: Option<&str>,
) -> Result<String, String> {
  if body.len() < 2 || body.len() > 3 {
    Err(format!("if expected 2~3 nodes, got: {body:?}"))
  } else {
    let mut chunk: String = String::from("");
    let mut cond_node = body[0].to_owned();
    let mut true_node = body[1].to_owned();
    let mut some_false_node = body.get(2);
    let mut need_else = false;

    let return_label = base_return_label.unwrap_or("return ");

    loop {
      let cond_code = to_js_code(&cond_node, ns, local_defs, file_imports, keywords, None)?;
      let true_code = to_js_code(&true_node, ns, local_defs, file_imports, keywords, Some(return_label))?;
      let else_mark = if need_else { " else " } else { "" };

      write!(chunk, "\n{else_mark}if ({cond_code}) {{ {true_code} }}").expect("write");

      if let Some(false_node) = some_false_node {
        if let Calcit::List(ys) = false_node {
          if let Some(Calcit::Syntax(syn, _ns)) = ys.get(0) {
            if syn == &CalcitSyntax::If {
              if ys.len() < 3 || ys.len() > 4 {
                return Err(format!("if expected 2~3 nodes, got: {ys:?}"));
              }
              cond_node = ys[1].to_owned();
              true_node = ys[2].to_owned();
              some_false_node = ys.get(3);
              need_else = true;
              continue;
            }
          }
        }

        let false_code = to_js_code(false_node, ns, local_defs, file_imports, keywords, Some(return_label))?;
        write!(chunk, "else {{ {false_code} }}").expect("write");
      } else {
        write!(chunk, "else {{ {return_label} null; }}").expect("write");
      }
      break;
    }

    if base_return_label.is_some() {
      Ok(chunk)
    } else {
      Ok(make_fn_wrapper(&chunk))
    }
  }
}

fn gen_args_code(
  body: &CalcitItems,
  ns: &str,
  local_defs: &HashSet<Arc<str>>,
  file_imports: &RefCell<ImportsDict>,
  keywords: &RefCell<HashSet<EdnKwd>>,
) -> Result<String, String> {
  let mut result = String::from("");
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let mut spreading = false;
  for x in body {
    match x {
      Calcit::Symbol { sym, .. } if &**sym == "&" => {
        spreading = true;
      }
      _ => {
        if !result.is_empty() {
          result.push_str(", ");
        }
        if spreading {
          write!(
            result,
            "...{}listToArray({})",
            var_prefix,
            to_js_code(x, ns, local_defs, file_imports, keywords, None)?
          )
          .expect("write");
          spreading = false
        } else {
          result.push_str(&to_js_code(x, ns, local_defs, file_imports, keywords, None)?);
        }
      }
    }
  }
  Ok(result)
}

fn list_to_js_code(
  xs: &CalcitItems,
  ns: &str,
  local_defs: HashSet<Arc<str>>,
  return_label: &str,
  file_imports: &RefCell<ImportsDict>,
  keywords: &RefCell<HashSet<EdnKwd>>,
) -> Result<String, String> {
  // TODO default returnLabel="return "
  let mut result = String::from("");
  for (idx, x) in xs.into_iter().enumerate() {
    // result = result & "// " & $x & "\n"
    if idx == xs.len() - 1 {
      let line = to_js_code(x, ns, &local_defs, file_imports, keywords, Some(return_label))?;
      result.push_str(&line);
      result.push('\n');
    } else {
      let line = to_js_code(x, ns, &local_defs, file_imports, keywords, None)?;
      // if is_let_call(&x) {
      //   result.push_str(&make_curly_wrapper(&line));
      // } else {
      result.push_str(&line);
      // }
      result.push_str(";\n");
    }
  }
  Ok(result)
}

fn uses_recur(xs: &Calcit) -> bool {
  match xs {
    Calcit::Symbol { sym: s, .. } => &**s == "recur",
    Calcit::Proc(s) => *s == CalcitProc::Recur,
    Calcit::List(ys) => match &ys.get(0) {
      Some(Calcit::Syntax(syn, _)) if syn == &CalcitSyntax::Defn => false,
      Some(Calcit::Symbol { sym, .. }) if &**sym == "defn" => false,
      _ => {
        for y in ys {
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
  args: &[Arc<str>],
  raw_body: &CalcitItems,
  passed_defs: &PassedDefs,
  exported: bool,
  keywords: &RefCell<HashSet<EdnKwd>>,
) -> Result<String, String> {
  let var_prefix = if passed_defs.ns == "calcit.core" { "" } else { "$calcit." };
  let mut local_defs = passed_defs.local_defs.to_owned();
  let mut spreading_code = String::from(""); // js list and calcit-js list are different, need to convert
  let mut args_code = String::from("");
  let mut spreading = false;
  let mut has_optional = false;
  let mut args_count = 0;
  let mut optional_count = 0;
  for sym in args {
    if spreading {
      if !args_code.is_empty() {
        args_code.push_str(", ");
      }
      local_defs.insert(sym.to_owned());
      let arg_name = escape_var(sym);
      args_code.push_str("...");
      args_code.push_str(&arg_name);
      // js list and calcit-js are different in spreading
      write!(spreading_code, "\n{arg_name} = {var_prefix}arrayToList({arg_name});").expect("write");
      break; // no more args after spreading argument
    } else if has_optional {
      if !args_code.is_empty() {
        args_code.push_str(", ");
      }
      local_defs.insert(sym.to_owned());
      args_code.push_str(&escape_var(sym));
      optional_count += 1;
    } else {
      if &**sym == "&" {
        spreading = true;
        continue;
      }
      if &**sym == "?" {
        has_optional = true;
        continue;
      }
      if !args_code.is_empty() {
        args_code.push_str(", ");
      }
      local_defs.insert(sym.to_owned());
      args_code.push_str(&escape_var(sym));
      args_count += 1;
    }
  }

  let check_args = if spreading {
    snippets::tmpl_args_fewer_than(args_count)
  } else if has_optional {
    snippets::tmpl_args_between(args_count, args_count + optional_count)
  } else {
    snippets::tmpl_args_exact(name, args_count)
  };

  let mut body: CalcitItems = TernaryTreeList::Empty;
  let mut async_prefix: String = String::from("");

  for line in raw_body {
    if let Calcit::List(xs) = line {
      if let Some(Calcit::Syntax(sym, _ns)) = xs.get(0) {
        if sym == &CalcitSyntax::HintFn {
          if hinted_async(xs) {
            async_prefix = String::from("async ")
          }
          continue;
        }
      }
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
      keywords,
    )?;
    let fn_def = snippets::tmpl_tail_recursion(
      /* name = */ escape_var(name),
      /* args_code = */ args_code,
      /* check_args = */ check_args,
      /* spreading_code = */ spreading_code,
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
      list_to_js_code(&body, passed_defs.ns, local_defs, "return ", passed_defs.file_imports, keywords)?
    );
    let export_mark = if exported { "export " } else { "" };
    Ok(format!("{export_mark}{fn_definition}\n"))
  }
}

/// this is a very rough implementation for now
fn hinted_async(xs: &TernaryTreeList<Calcit>) -> bool {
  for x in xs {
    match x {
      Calcit::Symbol { sym, .. } if &**sym == "async" => return true,
      _ => {}
    }
  }
  false
}

fn contains_symbol(xs: &Calcit, y: &str) -> bool {
  match xs {
    Calcit::Symbol { sym, .. } => &**sym == y,
    Calcit::Thunk(code, _) => contains_symbol(code, y),
    Calcit::Fn { body, .. } => {
      for x in &**body {
        if contains_symbol(x, y) {
          return true;
        }
      }
      false
    }
    Calcit::List(zs) => {
      for z in zs {
        if contains_symbol(z, y) {
          return true;
        }
      }
      false
    }
    _ => false,
  }
}

fn sort_by_deps(deps: &HashMap<Arc<str>, Calcit>) -> Vec<Arc<str>> {
  let mut deps_graph: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();
  let mut def_names: Vec<Arc<str>> = Vec::with_capacity(deps.len());
  for (k, v) in deps {
    def_names.push(k.to_owned());
    let mut deps_info: HashSet<Arc<str>> = HashSet::new();
    for k2 in deps.keys() {
      if k2 == k {
        continue;
      }
      // println "checking ", k, " -> ", k2, " .. ", v.containsSymbol(k2)
      if contains_symbol(v, k2) {
        deps_info.insert(k2.to_owned());
      }
    }
    deps_graph.insert(k.to_owned(), deps_info);
  }
  // println!("\ndefs graph {:?}", deps_graph);
  def_names.sort(); // alphabet order first

  let mut result: Vec<Arc<str>> = Vec::with_capacity(def_names.len());
  'outer: for x in def_names {
    for (idx, y) in result.iter().enumerate() {
      if depends_on(y, &x, &deps_graph, 3) {
        result.insert(idx, x.to_owned());
        continue 'outer;
      }
    }
    result.push(x.to_owned());
  }
  // println!("\ndef names {:?}", def_names);

  result
}

// could be slow, need real topology sorting
fn depends_on(x: &str, y: &str, deps: &HashMap<Arc<str>, HashSet<Arc<str>>>, decay: usize) -> bool {
  if decay == 0 {
    false
  } else {
    for item in &deps[x] {
      if &**item == y || depends_on(item, y, deps, decay - 1) {
        return true;
      } else {
        // nothing
      }
    }
    false
  }
}

fn write_file_if_changed(filename: &Path, content: &str) -> Result<bool, String> {
  if filename.exists() && fs::read_to_string(filename).map_err(|e| e.to_string())? == content {
    return Ok(false);
  }
  let _ = fs::write(filename, content);
  Ok(true)
}

pub fn emit_js(entry_ns: &str, emit_path: &str) -> Result<(), String> {
  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let mut unchanged_ns: HashSet<Arc<str>> = HashSet::new();

  let program = program::clone_evaled_program();
  for (ns, file) in program {
    // println!("start handling: {}", ns);
    // side-effects, reset tracking state

    let file_imports: RefCell<ImportsDict> = RefCell::new(BTreeMap::new());
    let keywords: RefCell<HashSet<EdnKwd>> = RefCell::new(HashSet::new());

    let mut defs_in_current: HashSet<Arc<str>> = HashSet::new();
    for k in file.keys() {
      defs_in_current.insert(k.to_owned());
    }

    if !internal_states::is_first_compilation() {
      let app_pkg_name = entry_ns.split('.').collect::<Vec<&str>>()[0];
      let pkg_name = ns.split('.').collect::<Vec<&str>>()[0]; // TODO simpler
      if app_pkg_name != pkg_name {
        match internal_states::lookup_prev_ns_cache(&ns) {
          Some(v) if v == defs_in_current => {
            // same as last time, skip
            continue;
          }
          _ => (),
        }
      }
    }
    // remember defs of each ns for comparing
    internal_states::write_as_ns_cache(&ns, defs_in_current);

    // reset index each file
    reset_js_gensym_index();

    let core_lib = to_js_import_name("calcit.core", true);

    let mut defs_code = String::from(""); // code generated by functions
    let mut vals_code = String::from(""); // code generated by thunks
    let mut direct_code = String::from(""); // dirty code to run directly
    let mut keywords_code = String::from("\nvar _kwd={};"); // initialization for keywords

    let mut import_code = if &*ns == "calcit.core" {
      snippets::tmpl_import_procs(wrap_js_str("@calcit/procs"))
    } else {
      format!("\nimport * as $calcit from {core_lib};")
    };

    let mut def_names: HashSet<Arc<str>> = HashSet::new(); // multiple parts of scoped defs need to be tracked

    // tracking top level scope definitions
    for def in file.keys() {
      def_names.insert(def.to_owned());
    }

    let deps_in_order = sort_by_deps(&file);
    // println!("deps order: {:?}", deps_in_order);

    for def in deps_in_order {
      if &*ns == primes::CORE_NS {
        // some defs from core can be replaced by calcit.procs
        if is_js_unavailable_procs(&def) {
          continue;
        }
        if is_preferred_js_proc(&def) {
          writeln!(defs_code, "\nvar {} = $calcit_procs.{};", escape_var(&def), escape_var(&def)).expect("write");
          continue;
        }
      }

      let f = file[&def].to_owned();

      match &f {
        // probably not work here
        Calcit::Proc(..) => {
          writeln!(defs_code, "\nvar {} = $calcit_procs.{};", escape_var(&def), escape_var(&def)).expect("write");
        }
        Calcit::Fn {
          name,
          def_ns,
          args,
          body: code,
          ..
        } => {
          gen_stack::push_call_stack(def_ns, name, StackKind::Codegen, f.to_owned(), &TernaryTreeList::Empty);
          let passed_defs = PassedDefs {
            ns: &ns,
            local_defs: &def_names,
            file_imports: &file_imports,
          };
          defs_code.push_str(&gen_js_func(&def, args, code, &passed_defs, true, &keywords)?);
          gen_stack::pop_call_stack();
        }
        Calcit::Thunk(code, _) => {
          // TODO need topological sorting for accuracy
          // values are called directly, put them after fns
          gen_stack::push_call_stack(&ns, &def, StackKind::Codegen, (**code).to_owned(), &TernaryTreeList::Empty);
          writeln!(
            vals_code,
            "\nexport var {} = {};",
            escape_var(&def),
            to_js_code(code, &ns, &def_names, &file_imports, &keywords, None)?
          )
          .expect("write");
          gen_stack::pop_call_stack()
        }
        // macro are not traced in codegen since already expanded
        Calcit::Macro { .. } => {}
        Calcit::Syntax(_, _) => {
          // should he handled inside compiler
        }
        Calcit::Bool(_) | Calcit::Number(_) => {
          println!("[Warn] expected thunk, got macro. skipped `{ns}/{def} {f}`")
        }
        _ => {
          println!("[Warn] expected thunk for js, skipped `{ns}/{def} {f}`")
        }
      }
    }
    if &*ns == primes::CORE_NS {
      // add at end of file to register builtin classes
      direct_code.push_str(&snippets::tmpl_classes_registering())
    }

    let collected_imports = file_imports.borrow();
    if !collected_imports.is_empty() {
      // println!("imports: {:?}", collected_imports);
      for (def, item) in collected_imports.iter() {
        // println!("implicit import {} in {} ", def, ns);
        match item {
          ImportedTarget::AsNs(target_ns) => {
            if is_cirru_string(target_ns) {
              let import_target = wrap_js_str(&target_ns[1..]);
              write!(import_code, "\nimport * as {} from {import_target};", escape_ns(def)).expect("write");
            } else {
              let import_target = to_js_import_name(target_ns, true);
              write!(import_code, "\nimport * as {} from {import_target};", escape_ns(target_ns)).expect("write");
            }
          }
          ImportedTarget::DefaultNs(target_ns) => {
            if is_cirru_string(target_ns) {
              let import_target = wrap_js_str(&target_ns[1..]);
              write!(import_code, "\nimport {} from {import_target};", escape_var(def)).expect("write");
            } else {
              unreachable!("only js import leads to default ns, but got: {}", target_ns)
            }
          }
          ImportedTarget::ReferNs(target_ns) => {
            let import_target = if is_cirru_string(target_ns) {
              wrap_js_str(&target_ns[1..])
            } else {
              to_js_import_name(target_ns, true)
            };
            write!(import_code, "\nimport {{ {} }} from {import_target};", escape_var(def)).expect("write");
          }
        }
      }
    }

    let kwd_prefix = if &*ns == "calcit.core" { "" } else { "$calcit." };
    let mut kwd_arr = String::from("[");
    let mut kwds: Vec<EdnKwd> = vec![];
    for k in keywords.borrow().iter() {
      kwds.push(k.to_owned());
    }
    // need to maintain a stable order to reduce redundant reloads
    kwds.sort();

    for s in kwds {
      let name = escape_cirru_str(&s.to_string());
      write!(kwd_arr, "{name},").expect("write");
    }
    kwd_arr.push(']');
    keywords_code.push_str(&snippets::tmpl_keywords_init(&kwd_arr, kwd_prefix));
    keywords_code.push('\n');

    let js_file_path = code_emit_path.join(to_mjs_filename(&ns));
    let wrote_new = write_file_if_changed(
      &js_file_path,
      &format!(
        "{}{}{}\n{}\n\n{}\n{}",
        import_code,
        keywords_code,
        snippets::tmpl_errors_init(),
        defs_code,
        vals_code,
        direct_code
      ),
    )?;
    if wrote_new {
      println!("Emitted js file: {}", js_file_path.to_str().expect("exptract path"));
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

fn is_js_unavailable_procs(name: &str) -> bool {
  matches!(
    name,
    "&reset-gensym-index!" | "gensym" | "macroexpand" | "macroexpand-all" | "to-cirru-edn" | "extract-cirru-edn"
  )
}

#[inline(always)]
fn is_cirru_string(s: &str) -> bool {
  s.starts_with('|') || s.starts_with('"')
}

#[inline(always)]
fn get_proc_prefix(ns: &str) -> &str {
  if ns == primes::CORE_NS {
    "$calcit_procs."
  } else {
    "$calcit."
  }
}

fn cirru_to_js(code: &Cirru) -> Result<String, String> {
  match code {
    Cirru::List(xs) => {
      let mut chunk = "[".to_owned();
      for x in xs {
        chunk.push_str(&cirru_to_js(x)?);
        chunk.push(',');
      }
      if chunk.ends_with(',') {
        chunk.pop();
      }
      chunk.push(']');
      Ok(chunk)
    }
    Cirru::Leaf(s) => Ok(format!("\"{}\"", s.escape_default())),
  }
}

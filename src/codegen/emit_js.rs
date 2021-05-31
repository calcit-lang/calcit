mod internal_states;
mod snippets;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::builtins::meta::{js_gensym, reset_js_gensym_index};
use crate::builtins::{is_js_syntax_procs, is_proc_name, is_syntax_name};
use crate::call_stack;
use crate::call_stack::StackKind;
use crate::primes;
use crate::primes::{Calcit, CalcitItems, ImportRule, SymbolResolved::*};
use crate::program;
use crate::util::string::has_ns_part;
use crate::util::string::{matches_js_var, wrap_js_str};

type ImportsDict = BTreeMap<String, ImportedTarget>;

#[derive(Debug, PartialEq, Clone)]
pub enum ImportedTarget {
  AsNs(String),
  DefaultNs(String),
  ReferNs(String),
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

fn to_js_file_name(ns: &str, mjs_mode: bool) -> String {
  let mut xs: String = String::from(ns);
  if mjs_mode {
    xs.push_str(".mjs");
  } else {
    xs.push_str(".js");
  }
  xs
}

fn escape_var(name: &str) -> String {
  if has_ns_part(name) {
    unreachable!(format!("Invalid variable name `{}`, use `escape_ns_var` instead", name));
  }
  match name {
    "if" => String::from("_IF_"),
    "do" => String::from("_DO_"),
    "else" => String::from("_ELSE_"),
    "let" => String::from("_LET_"),
    "case" => String::from("_CASE_"),
    "-" => String::from("_SUB_"),
    _ => name
      .replace("-", "_")
      // dot might be part of variable `\.`. not confused with syntax
      .replace(".", "_DOT_")
      .replace("?", "_QUES_")
      .replace("+", "_ADD_")
      .replace("^", "_CRT_")
      .replace("*", "_STAR_")
      .replace("&", "_AND_")
      .replace("{}", "_MAP_")
      .replace("[]", "_LIST_")
      .replace("{", "_CURL_")
      .replace("}", "_CURR_")
      .replace("'", "_SQUO_")
      .replace("[", "_SQRL_")
      .replace("]", "_SQRR_")
      .replace("!", "_BANG_")
      .replace("%", "_PCT_")
      .replace("/", "_SLSH_")
      .replace("=", "_EQ_")
      .replace(">", "_GT_")
      .replace("<", "_LT_")
      .replace(":", "_COL_")
      .replace(";", "_SCOL_")
      .replace("#", "_SHA_")
      .replace("\\", "_BSL_"),
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

fn escape_ns_var(name: &str, ns: &str) -> String {
  if !has_ns_part(name) {
    unreachable!(format!("Invalid variable name `{}`, lack of namespace part", name))
  }

  let pieces: Vec<&str> = name.split('/').collect();
  if pieces.len() != 2 {
    unreachable!(format!("Expected format of ns/def {}", name))
  }
  let ns_part = pieces[0];
  let def_part = pieces[1];
  if ns_part == "js" {
    def_part.to_owned()
  } else {
    format!("{}.{}", escape_ns(ns), escape_var(&def_part))
  }
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
      | "starts-with?"
      | "ends-with?"
  )
}

fn escape_cirru_str(s: &str) -> String {
  let mut result = String::from("\"");
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

fn quote_to_js(xs: &Calcit, var_prefix: &str) -> Result<String, String> {
  match xs {
    Calcit::Symbol(s, ..) => Ok(format!("new {}CrDataSymbol({})", var_prefix, escape_cirru_str(&s))),
    Calcit::Str(s) => Ok(escape_cirru_str(&s)),
    Calcit::Bool(b) => Ok(b.to_string()),
    Calcit::Number(n) => Ok(n.to_string()),
    Calcit::Nil => Ok(String::from("null")),
    // mainly for methods, which are recognized during reading
    Calcit::Proc(p) => Ok(format!("new {}CrDataSymbol({})", var_prefix, escape_cirru_str(&p))),
    Calcit::List(ys) => {
      let mut chunk = String::from("");
      for y in ys {
        if !chunk.is_empty() {
          chunk.push_str(", ");
        }
        chunk.push_str(&quote_to_js(y, var_prefix)?);
      }
      Ok(format!("new {}CrDataList([{}])", var_prefix, chunk))
    }
    Calcit::Keyword(s) => Ok(format!("{}kwd({})", var_prefix, escape_cirru_str(&s))),
    _ => unreachable!(format!("Unexpected data in quote for js: {}", xs)),
  }
}

fn make_let_with_bind(left: &str, right: &str, body: &str) -> String {
  format!("(function __bind__({}){{\n{} }})({})", left, body, right)
}

fn make_let_with_wrapper(left: &str, right: &str, body: &str) -> String {
  format!("(function __let__(){{ \nlet {} = {};\n {} }})()", left, right, body)
}

fn make_fn_wrapper(body: &str) -> String {
  format!("(function __fn__(){{\n{}\n}})()", body)
}

fn to_js_code(
  xs: &Calcit,
  ns: &str,
  local_defs: &HashSet<String>,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let ret = match xs {
    Calcit::Symbol(s, def_ns, resolved) => gen_symbol_code(s, &def_ns, resolved, ns, xs, local_defs, file_imports),
    Calcit::Proc(s) => {
      let proc_prefix = if ns == primes::CORE_NS {
        "$calcit_procs."
      } else {
        "$calcit."
      };
      // println!("gen proc {} under {}", s, ns,);
      // let resolved = Some(ResolvedDef(String::from(primes::CORE_NS), s.to_owned()));
      // gen_symbol_code(s, primes::CORE_NS, &resolved, ns, xs, local_defs)

      if s.starts_with('.') {
        if s.starts_with(".-") || s.starts_with(".!") {
          Err(format!("invalid js method {} at this position", s))
        } else {
          // `.method` being used as a parameter
          let name = s.strip_prefix('.').unwrap();
          Ok(format!(
            "{}invoke_method({})",
            var_prefix,
            escape_cirru_str(&name), // TODO need confirm
          ))
        }
      } else {
        Ok(format!("{}{}", proc_prefix, escape_var(s)))
      }
    }
    Calcit::Syntax(s, ..) => {
      let resolved = Some(ResolvedDef(String::from(primes::CORE_NS), s.to_owned(), None));
      gen_symbol_code(s, primes::CORE_NS, &resolved, ns, xs, local_defs, file_imports)
    }
    Calcit::Str(s) => Ok(escape_cirru_str(&s)),
    Calcit::Bool(b) => Ok(b.to_string()),
    Calcit::Number(n) => Ok(n.to_string()),
    Calcit::Nil => Ok(String::from("null")),
    Calcit::Keyword(s) => Ok(format!("{}kwd({})", var_prefix, wrap_js_str(s))),
    Calcit::List(ys) => gen_call_code(&ys, ns, local_defs, xs, file_imports),
    a => unreachable!(format!("[Warn] unknown kind to gen js code: {}", a)),
  };

  ret
}

fn gen_call_code(
  ys: &CalcitItems,
  ns: &str,
  local_defs: &HashSet<String>,
  xs: &Calcit,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let proc_prefix = if ns == primes::CORE_NS {
    "$calcit_procs."
  } else {
    "$calcit."
  };
  if ys.is_empty() {
    println!("[Warn] Unexpected empty list inside {}", xs);
    return Ok(String::from("()"));
  }

  let head = ys[0].clone();
  let body = ys.skip(1);
  match &head {
    Calcit::Symbol(s, ..) | Calcit::Proc(s) | Calcit::Syntax(s, ..) => {
      match s.as_str() {
        "if" => match (body.get(0), body.get(1)) {
          (Some(condition), Some(true_branch)) => {
            call_stack::push_call_stack(ns, "if", StackKind::Codegen, xs.to_owned(), &im::vector![]);
            let false_code = match body.get(2) {
              Some(fal) => to_js_code(fal, ns, local_defs, file_imports)?,
              None => String::from("null"),
            };
            let cond_code = to_js_code(condition, ns, local_defs, file_imports)?;
            let true_code = to_js_code(true_branch, ns, local_defs, file_imports)?;
            call_stack::pop_call_stack();
            Ok(format!("( {} ? {} : {} )", cond_code, true_code, false_code))
          }
          (_, _) => Err(format!("if expected 2~3 nodes, got: {:?}", body)),
        },
        "&let" => gen_let_code(&body, local_defs, &xs, ns, file_imports),
        ";" => Ok(format!("(/* {} */ null)", Calcit::List(body))),

        "quote" => match body.get(0) {
          Some(item) => quote_to_js(&item, var_prefix),
          None => Err(format!("quote expected a node, got nothing from {:?}", body)),
        },
        "defatom" => {
          match (body.get(0), body.get(1)) {
            _ if body.len() > 2 => Err(format!("defatom expected name and value, got too many: {:?}", body)),
            (Some(Calcit::Symbol(sym, ..)), Some(v)) => {
              // let _name = escape_var(sym); // TODO
              let ref_path = wrap_js_str(&format!("{}/{}", ns, sym.clone()));
              call_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &im::vector![]);
              let value_code = &to_js_code(v, ns, local_defs, file_imports)?;
              call_stack::pop_call_stack();
              Ok(format!(
                "\n({}peekDefatom({}) ?? {}defatom({}, {}))\n",
                &var_prefix, &ref_path, &var_prefix, &ref_path, value_code
              ))
            }
            (_, _) => Err(format!("defatom expected name and value, got: {:?}", body)),
          }
        }

        "defn" => match (body.get(0), body.get(1)) {
          (Some(Calcit::Symbol(sym, ..)), Some(Calcit::List(ys))) => {
            let func_body = body.skip(2);
            call_stack::push_call_stack(ns, sym, StackKind::Codegen, xs.to_owned(), &im::vector![]);
            let ret = gen_js_func(sym, &ys, &func_body, ns, false, local_defs, file_imports);
            call_stack::pop_call_stack();
            ret
          }
          (_, _) => Err(format!("defn expected name arguments, got: {:?}", body)),
        },

        "defmacro" => Ok(format!("/* Unexpected macro {} */", xs)),
        "quote-replace" | "quasiquote" => Ok(format!("(/* Unexpected quasiquote {} */ null)", xs.lisp_str())),

        "raise" => {
          // not core syntax, but treat as macro for better debugging experience
          match body.get(0) {
            Some(m) => {
              let message: String = to_js_code(m, ns, local_defs, file_imports)?;
              let data_code = match body.get(1) {
                Some(d) => to_js_code(d, ns, local_defs, file_imports)?,
                None => String::from("null"),
              };
              let err_var = js_gensym("err");
              Ok(make_fn_wrapper(&format!(
                "let {} = new Error({});\n {}.data = {};\n throw {};",
                err_var, message, err_var, data_code, err_var
              )))
            }
            None => Err(format!("raise expected 1~2 arguments, got {:?}", body)),
          }
        }
        "try" => match (body.get(0), body.get(1)) {
          (Some(expr), Some(handler)) => {
            call_stack::push_call_stack(ns, "try", StackKind::Codegen, xs.to_owned(), &im::vector![]);
            let code = to_js_code(expr, ns, local_defs, file_imports)?;
            let err_var = js_gensym("errMsg");
            let handler = to_js_code(handler, ns, local_defs, file_imports)?;

            call_stack::pop_call_stack();

            Ok(snippets::tmpl_fn_wrapper(snippets::tmpl_try(err_var, code, handler)))
          }
          (_, _) => Err(format!("try expected 2 nodes, got {:?}", body)),
        },
        "echo" | "println" => {
          // not core syntax, but treat as macro for better debugging experience
          let args = ys.skip(1);
          let args_code = gen_args_code(&args, ns, local_defs, file_imports)?;
          Ok(format!("console.log({}printable({}))", proc_prefix, args_code))
        }
        "exists?" => {
          // not core syntax, but treat as macro for availability
          match body.get(0) {
            Some(Calcit::Symbol(_sym, ..)) => {
              let target = to_js_code(&body[0], ns, local_defs, file_imports)?; // TODO could be simpler
              return Ok(format!("(typeof {} !== 'undefined')", target));
            }
            Some(a) => Err(format!("exists? expected a symbol, got {}", a)),
            None => Err(format!("exists? expected 1 node, got {:?}", body)),
          }
        }
        "new" => match body.get(0) {
          Some(ctor) => {
            let args = body.skip(1);
            let args_code = gen_args_code(&args, ns, local_defs, file_imports)?;
            Ok(format!(
              "new {}({})",
              to_js_code(&ctor, ns, local_defs, file_imports)?,
              args_code
            ))
          }
          None => Err(format!("`new` expected constructor, got nothing, {}", xs)),
        },
        "instance?" => match (body.get(0), body.get(1)) {
          (Some(ctor), Some(v)) => Ok(format!(
            "({} instanceof {})",
            to_js_code(v, ns, local_defs, file_imports)?,
            to_js_code(ctor, ns, local_defs, file_imports)?
          )),
          (_, _) => Err(format!("instance? expected 2 arguments, got {:?}", body)),
        },
        "set!" => match (body.get(0), body.get(1)) {
          (Some(target), Some(v)) => Ok(format!(
            "{} = {}",
            to_js_code(target, ns, local_defs, file_imports)?,
            to_js_code(v, ns, local_defs, file_imports)?
          )),
          (_, _) => Err(format!("set! expected 2 nodes, got {:?}", body)),
        },
        _ if s.starts_with(".-") => {
          let name = s.strip_prefix(".-").unwrap();
          if name.is_empty() {
            Err(format!("invalid property accessor {}", s))
          } else {
            match body.get(0) {
              Some(obj) => Ok(format!("{}.{}", to_js_code(&obj, ns, local_defs, file_imports)?, name)),
              None => Err(format!("property accessor takes only 1 argument, {:?}", xs)),
            }
          }
        }
        _ if s.starts_with(".!") => {
          // special syntax for calling a static method, previously using `.` but now occupied
          let name = s.strip_prefix(".!").unwrap();
          if matches_js_var(name) {
            match body.get(0) {
              Some(obj) => {
                let args = body.skip(1);
                let args_code = gen_args_code(&args, ns, local_defs, file_imports)?;
                Ok(format!(
                  "{}.{}({})",
                  to_js_code(&obj, ns, local_defs, file_imports)?,
                  name,
                  args_code
                ))
              }
              None => Err(format!("expected 1 object, got {}", xs)),
            }
          } else {
            Err(format!("invalid static member accessor {}", s))
          }
        }
        _ if s.starts_with('.') => {
          let name = s.strip_prefix('.').unwrap();
          match body.get(0) {
            Some(obj) => {
              let args = body.skip(1);
              let args_code = gen_args_code(&args, ns, local_defs, file_imports)?;
              Ok(format!(
                "{}invoke_method({})({},{})",
                var_prefix,
                escape_cirru_str(&name), // TODO need confirm
                to_js_code(&obj, ns, local_defs, file_imports)?,
                args_code
              ))
            }
            None => Err(format!("expected 1 object, got {}", xs)),
          }
        }
        _ => {
          // TODO
          let args_code = gen_args_code(&body, ns, &local_defs, file_imports)?;
          Ok(format!(
            "{}({})",
            to_js_code(&head, ns, local_defs, file_imports)?,
            args_code
          ))
        }
      }
    }
    _ => {
      let args_code = gen_args_code(&body, ns, &local_defs, file_imports)?;
      Ok(format!(
        "{}({})",
        to_js_code(&head, ns, local_defs, file_imports)?,
        args_code
      ))
    }
  }
}

fn gen_symbol_code(
  s: &str,
  def_ns: &str,
  resolved: &Option<primes::SymbolResolved>,
  ns: &str,
  xs: &Calcit,
  local_defs: &HashSet<String>,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  // println!("gen symbol: {} {} {} {:?}", s, def_ns, ns, resolved);
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  if has_ns_part(s) {
    let ns_part = s.split('/').collect::<Vec<&str>>()[0]; // TODO
    if ns_part == "js" {
      Ok(escape_ns_var(s, "js"))
    } else {
      // TODO ditry code
      // TODO namespace part supposed be parsed during preprocessing, this mimics old behaviors
      match resolved {
        Some(ResolvedDef(r_ns, _r_def, _import_rule /* None */)) => {
          if is_cirru_string(r_ns) {
            track_ns_import(ns_part.to_owned(), ImportedTarget::AsNs(r_ns.to_owned()), file_imports)?;
            Ok(escape_ns_var(s, ns_part))
          } else {
            track_ns_import(r_ns.clone(), ImportedTarget::AsNs(r_ns.to_owned()), file_imports)?;
            Ok(escape_ns_var(s, r_ns))
          }
        }
        Some(ResolvedRaw) => Err(format!("not going to generate from raw symbol, {}", s)),
        Some(ResolvedLocal) => Err(format!("symbol with ns should not be local, {}", s)),
        None => Err(format!("expected symbol with ns being resolved: {:?}", xs)),
      }
    }
  } else if is_js_syntax_procs(s) || is_proc_name(s) || is_syntax_name(s) {
    // return Ok(format!("{}{}", var_prefix, escape_var(s)));
    let proc_prefix = if ns == primes::CORE_NS {
      "$calcit_procs."
    } else {
      "$calcit."
    };
    return Ok(format!("{}{}", proc_prefix, escape_var(s)));
  } else if matches!(resolved, Some(ResolvedLocal)) || local_defs.contains(s) {
    Ok(escape_var(s))
  } else if let Some(ResolvedDef(r_ns, _r_def, import_rule)) = resolved.clone() {
    if r_ns == primes::CORE_NS {
      // functions under core uses built $calcit module entry
      return Ok(format!("{}{}", var_prefix, escape_var(s)));
    }
    if let Some(ImportRule::NsDefault(_s)) = import_rule {
      // imports that using :default are special
      track_ns_import(s.to_owned(), ImportedTarget::DefaultNs(r_ns), file_imports)?;
    } else {
      track_ns_import(s.to_owned(), ImportedTarget::ReferNs(r_ns), file_imports)?;
    }
    Ok(escape_var(s))
  } else if def_ns == primes::CORE_NS {
    // local variales inside calcit.core also uses this ns
    println!("[Warn] detected variable inside core not resolved");
    Ok(format!("{}{}", var_prefix, escape_var(s)))
  } else if def_ns.is_empty() {
    Err(format!("Unexpected ns at symbol, {:?}", xs))
  } else if def_ns != ns {
    track_ns_import(s.to_owned(), ImportedTarget::ReferNs(def_ns.to_owned()), file_imports)?;

    // probably via macro
    // TODO dirty code collecting imports

    Ok(escape_var(s))
  } else if def_ns == ns {
    println!("[Warn] detected unresolved variable {:?} in {}", xs, ns);
    Ok(escape_var(s))
  } else {
    println!("[Warn] Unexpected casecode gen for {:?} in {}", xs, ns);
    Ok(format!("{}{}", var_prefix, escape_var(s)))
  }
}

// track but compare first, return Err if a different one existed
fn track_ns_import(
  sym: String,
  import_rule: ImportedTarget,
  file_imports: &RefCell<ImportsDict>,
) -> Result<(), String> {
  let mut dict = file_imports.borrow_mut();
  match dict.get(&sym) {
    Some(v) => {
      if *v == import_rule {
        Ok(())
      } else {
        Err(format!(
          "conflicted import rule, previous {:?}, now {:?}",
          v, import_rule
        ))
      }
    }
    None => {
      dict.insert(sym, import_rule);
      Ok(())
    }
  }
}

fn gen_let_code(
  body: &CalcitItems,
  local_defs: &HashSet<String>,
  xs: &Calcit,
  ns: &str,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  let mut let_def_body = body.clone();

  // defined new local variable
  let mut scoped_defs = local_defs.clone();
  let mut defs_code = String::from("");
  let mut body_part = String::from("");

  // break unless nested &let is found
  loop {
    if let_def_body.len() <= 1 {
      return Err(format!("&let expected body, but got empty, {}", xs.lisp_str()));
    }
    let pair = let_def_body[0].clone();
    let content = let_def_body.skip(1);

    match &pair {
      Calcit::Nil => {
        // non content defs_code

        for (idx, x) in content.iter().enumerate() {
          if idx == content.len() - 1 {
            body_part.push_str("return ");
            body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports)?);
            body_part.push_str(";\n");
          } else {
            body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports)?);
            body_part.push_str(";\n");
          }
        }
        break;
      }
      Calcit::List(xs) if xs.len() == 2 => {
        let def_name = xs[0].clone();
        let def_code = xs[1].clone();

        match def_name {
          Calcit::Symbol(sym, ..) => {
            // TODO `let` inside expressions makes syntax error
            let left = escape_var(&sym);
            let right = to_js_code(&def_code, &ns, &scoped_defs, file_imports)?;
            defs_code.push_str(&format!("let {} = {};\n", left, right));

            if scoped_defs.contains(&sym) {
              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports)?);
                  body_part.push_str(";\n");
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports)?);
                  body_part.push_str(";\n");
                }
              }

              // first variable is using conflicted name
              if local_defs.contains(&sym) {
                return Ok(make_let_with_bind(&left, &right, &body_part));
              } else {
                return Ok(make_let_with_wrapper(&left, &right, &body_part));
              }
            } else {
              // track variable
              scoped_defs.insert(sym.clone());

              if content.len() == 1 {
                match &content[0] {
                  Calcit::List(ys) if ys.len() > 2 => match (&ys[0], &ys[1]) {
                    (Calcit::Syntax(sym, _ns), Calcit::List(zs)) if sym == "&let" && zs.len() == 2 => match &zs[0] {
                      Calcit::Symbol(s2, ..) if !scoped_defs.contains(s2) => {
                        let_def_body = ys.skip(1);
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
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports)?);
                  body_part.push_str(";\n");
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs, file_imports)?);
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
      _ => return Err(format!("expected pair of a list of length 2, got: {}", pair)),
    }
  }
  return Ok(make_fn_wrapper(&format!("{}{}", defs_code, body_part)));
}

fn gen_args_code(
  body: &CalcitItems,
  ns: &str,
  local_defs: &HashSet<String>,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  let mut result = String::from("");
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let mut spreading = false;
  for x in body {
    match x {
      Calcit::Symbol(s, ..) if s == "&" => {
        spreading = true;
      }
      _ => {
        if !result.is_empty() {
          result.push_str(", ");
        }
        if spreading {
          result.push_str(&format!(
            "...{}listToArray({})",
            var_prefix,
            to_js_code(x, ns, local_defs, file_imports)?
          ));
          spreading = false
        } else {
          result.push_str(&to_js_code(&x, ns, &local_defs, file_imports)?);
        }
      }
    }
  }
  Ok(result)
}

fn list_to_js_code(
  xs: &CalcitItems,
  ns: &str,
  local_defs: HashSet<String>,
  return_label: &str,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  // TODO default returnLabel="return "
  let mut result = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    // result = result & "// " & $x & "\n"
    if idx == xs.len() - 1 {
      result.push_str(return_label);
      result.push_str(&to_js_code(&x, ns, &local_defs, file_imports)?);
      result.push_str(";\n");
    } else {
      result.push_str(&to_js_code(x, ns, &local_defs, file_imports)?);
      result.push_str(";\n");
    }
  }
  Ok(result)
}

fn uses_recur(xs: &Calcit) -> bool {
  match xs {
    Calcit::Symbol(s, ..) => s == "recur",
    Calcit::Proc(s) => s == "recur",
    Calcit::List(ys) => {
      for y in ys {
        if uses_recur(y) {
          return true;
        }
      }
      false
    }
    _ => false,
  }
}

fn gen_js_func(
  name: &str,
  args: &CalcitItems,
  body: &CalcitItems,
  ns: &str,
  exported: bool,
  outer_defs: &HashSet<String>,
  file_imports: &RefCell<ImportsDict>,
) -> Result<String, String> {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let mut local_defs = outer_defs.clone();
  let mut spreading_code = String::from(""); // js list and calcit-js list are different, need to convert
  let mut args_code = String::from("");
  let mut spreading = false;
  let mut has_optional = false;
  let mut args_count = 0;
  let mut optional_count = 0;
  for x in args {
    match x {
      Calcit::Symbol(sym, ..) => {
        if spreading {
          if !args_code.is_empty() {
            args_code.push_str(", ");
          }
          local_defs.insert(sym.clone());
          let arg_name = escape_var(&sym);
          args_code.push_str("...");
          args_code.push_str(&arg_name);
          // js list and calcit-js are different in spreading
          spreading_code.push_str(&format!("\n{} = {}arrayToList({});", arg_name, var_prefix, arg_name));
          break; // no more args after spreading argument
        } else if has_optional {
          if !args_code.is_empty() {
            args_code.push_str(", ");
          }
          local_defs.insert(sym.clone());
          args_code.push_str(&escape_var(&sym));
          optional_count += 1;
        } else {
          if sym == "&" {
            spreading = true;
            continue;
          }
          if sym == "?" {
            has_optional = true;
            continue;
          }
          if !args_code.is_empty() {
            args_code.push_str(", ");
          }
          local_defs.insert(sym.clone());
          args_code.push_str(&escape_var(&sym));
          args_count += 1;
        }
      }
      _ => return Err(format!("Expected symbol for arg, {}", x)),
    }
  }

  let check_args = if spreading {
    snippets::tmpl_args_fewer_than(args_count)
  } else if has_optional {
    snippets::tmpl_args_between(args_count, args_count + optional_count)
  } else {
    snippets::tmpl_args_exact(args_count)
  };

  if !body.is_empty() && uses_recur(&body[body.len() - 1]) {
    let fn_def = snippets::tmpl_tail_recursion(
      /* name = */ escape_var(name),
      /* args_code = */ args_code,
      /* check_args = */ check_args,
      /* spreading_code = */ spreading_code,
      /* body = */
      list_to_js_code(&body, ns, local_defs, "%%return_mark%% =", file_imports)?, // dirty trick
      /* var_prefix = */ var_prefix.to_owned(),
    );

    let export_mark = if exported {
      format!("export let {} = ", escape_var(name))
    } else {
      String::from("")
    };
    Ok(format!("{}{}\n", export_mark, fn_def))
  } else {
    let fn_definition = format!(
      "function {}({}) {{ {}{}\n{} }}",
      escape_var(name),
      args_code,
      check_args,
      spreading_code,
      list_to_js_code(&body, ns, local_defs, "return ", file_imports)?
    );
    let export_mark = if exported { "export " } else { "" };
    Ok(format!("{}{}\n", export_mark, fn_definition))
  }
}

fn contains_symbol(xs: &Calcit, y: &str) -> bool {
  match xs {
    Calcit::Symbol(s, ..) => s == y,
    Calcit::Thunk(code) => contains_symbol(code, y),
    Calcit::Fn(_, _, _, _, _, body) => {
      for x in body {
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

fn sort_by_deps(deps: &HashMap<String, Calcit>) -> Vec<String> {
  let mut deps_graph: HashMap<String, HashSet<String>> = HashMap::new();
  let mut def_names: Vec<String> = vec![];
  for (k, v) in deps {
    def_names.push(k.clone());
    let mut deps_info: HashSet<String> = HashSet::new();
    for k2 in deps.keys() {
      if k2 == k {
        continue;
      }
      // echo "checking ", k, " -> ", k2, " .. ", v.containsSymbol(k2)
      if contains_symbol(&v, &k2) {
        deps_info.insert(k2.clone());
      }
    }
    deps_graph.insert(k.to_owned(), deps_info);
  }
  // println!("\ndefs graph {:?}", deps_graph);
  def_names.sort(); // alphabet order first

  let mut result: Vec<String> = vec![];
  'outer: for x in def_names {
    for (idx, y) in result.iter().enumerate() {
      if depends_on(y, &x, &deps_graph, 3) {
        result.insert(idx, x.clone());
        continue 'outer;
      }
    }
    result.push(x.clone());
  }
  // println!("\ndef names {:?}", def_names);

  result
}

// could be slow, need real topology sorting
fn depends_on(x: &str, y: &str, deps: &HashMap<String, HashSet<String>>, decay: usize) -> bool {
  if decay == 0 {
    false
  } else {
    for item in &deps[x] {
      if item == y || depends_on(&item, y, &deps, decay - 1) {
        return true;
      } else {
        // nothing
      }
    }
    false
  }
}

fn write_file_if_changed(filename: &Path, content: &str) -> bool {
  if filename.exists() && fs::read_to_string(filename).unwrap() == content {
    return false;
  }
  let _ = fs::write(filename, content);
  true
}

pub fn emit_js(entry_ns: &str, emit_path: &str) -> Result<(), String> {
  let code_emit_path = Path::new(emit_path);
  if !code_emit_path.exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let mut unchanged_ns: HashSet<String> = HashSet::new();

  let program = program::clone_evaled_program();
  for (ns, file) in program {
    // println!("start handling: {}", ns);
    // side-effects, reset tracking state

    let file_imports: RefCell<ImportsDict> = RefCell::new(BTreeMap::new());

    let mut defs_in_current: HashSet<String> = HashSet::new();
    for k in file.keys() {
      defs_in_current.insert(k.clone());
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

    // let coreLib = "http://js.calcit-lang.org/calcit.core.js".escape()
    let core_lib = to_js_import_name("calcit.core", false); // TODO js_mode

    let mut defs_code = String::from(""); // code generated by functions
    let mut vals_code = String::from(""); // code generated by thunks
    let mut direct_code = String::from(""); // dirty code to run directly

    let mut import_code = if ns == "calcit.core" {
      snippets::tmpl_import_procs(wrap_js_str("@calcit/procs"))
    } else {
      format!("\nimport * as $calcit from {};\n", core_lib)
    };

    let mut def_names: HashSet<String> = HashSet::new(); // multiple parts of scoped defs need to be tracked

    // tracking top level scope definitions
    for def in file.keys() {
      def_names.insert(def.clone());
    }

    let deps_in_order = sort_by_deps(&file);
    // println!("deps order: {:?}", deps_in_order);

    for def in deps_in_order {
      if ns == primes::CORE_NS {
        // some defs from core can be replaced by calcit.procs
        if is_js_unavailable_procs(&def) {
          continue;
        }
        if is_preferred_js_proc(&def) {
          defs_code.push_str(&format!(
            "\nvar {} = $calcit_procs.{};\n",
            escape_var(&def),
            escape_var(&def)
          ));
          continue;
        }
      }

      let f = file[&def].clone();

      match &f {
        // probably not work here
        Calcit::Proc(..) => {
          defs_code.push_str(&format!(
            "\nvar {} = $calcit_procs.{};\n",
            escape_var(&def),
            escape_var(&def)
          ));
        }
        Calcit::Fn(name, def_ns, _, _, args, code) => {
          call_stack::push_call_stack(def_ns, name, StackKind::Codegen, f.to_owned(), &im::vector![]);
          defs_code.push_str(&gen_js_func(&def, args, code, &ns, true, &def_names, &file_imports)?);
          call_stack::pop_call_stack();
        }
        Calcit::Thunk(code) => {
          // TODO need topological sorting for accuracy
          // values are called directly, put them after fns
          call_stack::push_call_stack(&ns, &def, StackKind::Codegen, (**code).to_owned(), &im::vector![]);
          vals_code.push_str(&format!(
            "\nexport var {} = {};\n",
            escape_var(&def),
            to_js_code(code, &ns, &def_names, &file_imports)?
          ));
          call_stack::pop_call_stack()
        }
        Calcit::Macro(..) => {
          // macro should be handled during compilation, psuedo code
          defs_code.push_str(&snippets::tmpl_export_macro(escape_var(&def)));
        }
        Calcit::Syntax(_, _) => {
          // should he handled inside compiler
        }
        _ => {
          println!("[Warn] strange case for generating a definition: {}", f)
        }
      }
    }
    if ns == primes::CORE_NS {
      // add at end of file to register builtin classes
      direct_code.push_str(&snippets::tmpl_classes_registering())
    }

    let collected_imports = file_imports.into_inner();
    //  internal_states::clone_imports().unwrap(); // ignore unlocking details
    if !collected_imports.is_empty() {
      // println!("imports: {:?}", collected_imports);
      for (def, item) in collected_imports {
        // println!("implicit import {} in {} ", def, ns);
        match item {
          ImportedTarget::AsNs(target_ns) => {
            if is_cirru_string(&target_ns) {
              let import_target = wrap_js_str(&target_ns[1..]);
              import_code.push_str(&format!("\nimport * as {} from {};\n", escape_ns(&def), import_target));
            } else {
              let import_target = to_js_import_name(&target_ns, false); // TODO js_mode
              import_code.push_str(&format!(
                "\nimport * as {} from {};\n",
                escape_ns(&target_ns),
                import_target
              ));
            }
          }
          ImportedTarget::DefaultNs(target_ns) => {
            if is_cirru_string(&target_ns) {
              let import_target = wrap_js_str(&target_ns[1..]);
              import_code.push_str(&format!("\nimport {} from {};\n", escape_var(&def), import_target));
            } else {
              unreachable!(format!("only js import leads to default ns, but got: {}", target_ns))
            }
          }
          ImportedTarget::ReferNs(target_ns) => {
            let import_target = if is_cirru_string(&target_ns) {
              wrap_js_str(&target_ns[1..])
            } else {
              to_js_import_name(&target_ns, false) // TODO js_mode
            };
            import_code.push_str(&format!(
              "\nimport {{ {} }} from {};\n",
              escape_var(&def),
              import_target
            ));
          }
        }
      }
    }

    let js_file_path = code_emit_path.join(to_js_file_name(&ns, false)); // TODO mjs_mode
    let wrote_new = write_file_if_changed(
      &js_file_path,
      &format!("{}\n{}\n{}\n{}", import_code, defs_code, vals_code, direct_code),
    );
    if wrote_new {
      println!("Emitted js file: {}", js_file_path.to_str().unwrap());
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
    "&reset-gensym-index!"
      | "dbt->point"
      | "dbt-digits" // TODO none
      | "dbt-balanced-ternary"
      | "gensym"
      | "macroexpand"
      | "macroexpand-all"
      | "to-cirru-edn"
      | "extract-cirru-edn"
  )
}

fn is_cirru_string(s: &str) -> bool {
  s.starts_with('|') || s.starts_with('"')
}

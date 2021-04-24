mod internal_states;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::builtins::meta::{js_gensym, reset_js_gensym_index};
use crate::builtins::{is_proc_name, is_syntax_name};
use crate::primes;
use crate::primes::{format_to_lisp, Calcit, CalcitItems, SymbolResolved::*};
use crate::program;
use crate::util::string::has_ns_part;
use crate::util::string::matches_js_var;

use internal_states::CollectedImportItem;

fn to_js_import_name(ns: &str, mjs_mode: bool) -> String {
  let mut xs: String = String::from("./");
  xs.push_str(ns);
  if mjs_mode {
    xs.push_str(".mjs");
  }
  // currently use `import "./ns.name"`
  format!("\"{}\"", xs.escape_debug().to_string())
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
    name[1..].to_string() // TODO
  } else {
    name.to_string()
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
    def_part.to_string()
  } else if def_part == "@" {
    // TODO special syntax for js, using module directly, need a better solution
    escape_ns(ns)
  } else {
    format!("{}.{}", escape_ns(ns), escape_var(&def_part))
  }
}

// tell compiler to handle namespace code generation
fn is_builtin_js_proc(name: &str) -> bool {
  matches!(
    name,
    "aget"
      | "aset"
      | "extract-cirru-edn"
      | "to-cirru-edn"
      | "to-js-data"
      | "to-calcit-data"
      | "printable"
      | "instance?"
      | "timeout-call"
      | "load-console-formatter!"
      | "foldl"
  )
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
      | "atom?"
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

fn to_js_code(xs: &Calcit, ns: &str, local_defs: &HashSet<String>) -> Result<String, String> {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let ret = match xs {
    Calcit::Symbol(s, def_ns, resolved) => gen_symbol_code(s, &def_ns, resolved, ns, xs, local_defs),
    Calcit::Proc(s) => {
      let proc_prefix = if ns == primes::CORE_NS {
        "$calcit_procs."
      } else {
        "$calcit."
      };
      // println!("gen proc {} under {}", s, ns,);
      // let resolved = Some(ResolvedDef(String::from(primes::CORE_NS), s.to_string()));
      // gen_symbol_code(s, primes::CORE_NS, &resolved, ns, xs, local_defs)
      Ok(format!("{}{}", proc_prefix, escape_var(s)))
    }
    Calcit::Syntax(s, ..) => {
      let resolved = Some(ResolvedDef(String::from(primes::CORE_NS), s.to_string()));
      gen_symbol_code(s, primes::CORE_NS, &resolved, ns, xs, local_defs)
    }
    Calcit::Str(s) => Ok(escape_cirru_str(&s)),
    Calcit::Bool(b) => Ok(b.to_string()),
    Calcit::Number(n) => Ok(n.to_string()),
    Calcit::Nil => Ok(String::from("null")),
    Calcit::Keyword(s) => Ok(format!("{}kwd(\"{}\")", var_prefix, s.escape_debug())),
    Calcit::List(ys) => gen_call_code(&ys, ns, local_defs, xs),
    a => unreachable!(format!("[Warn] unknown kind to gen js code: {}", a)),
  };

  ret
}

fn gen_call_code(ys: &CalcitItems, ns: &str, local_defs: &HashSet<String>, xs: &Calcit) -> Result<String, String> {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  if ys.is_empty() {
    println!("[Warn] Unexpected empty list inside {}", xs);
    return Ok(String::from("()"));
  }

  let head = ys[0].clone();
  let body = ys.clone().slice(1..);
  match &head {
    Calcit::Symbol(s, ..) | Calcit::Proc(s) | Calcit::Syntax(s, ..) => {
      match s.as_str() {
        "if" => match (body.get(0), body.get(1)) {
          (Some(condition), Some(true_branch)) => {
            let false_code = match body.get(2) {
              Some(fal) => to_js_code(fal, ns, local_defs)?,
              None => String::from("null"),
            };
            let cond_code = to_js_code(condition, ns, local_defs)?;
            let true_code = to_js_code(true_branch, ns, local_defs)?;
            Ok(format!("( {} ? {} : {} )", cond_code, true_code, false_code))
          }
          (_, _) => Err(format!("if expected 2~3 nodes, got: {:?}", body)),
        },
        "&let" => gen_let_code(&body, local_defs, &xs, ns),
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
              let atom_path = format!("\"{}\"", format!("{}/{}", ns, sym.clone()).escape_debug());
              let value_code = &to_js_code(v, ns, local_defs)?;
              Ok(format!(
                "\n({}peekDefatom({}) ?? {}defatom({}, {}))\n",
                &var_prefix, &atom_path, &var_prefix, &atom_path, value_code
              ))
            }
            (_, _) => Err(format!("defatom expected name and value, got: {:?}", body)),
          }
        }

        "defn" => match (body.get(0), body.get(1)) {
          (Some(Calcit::Symbol(sym, ..)), Some(Calcit::List(ys))) => {
            let func_body = body.clone().slice(2..);
            gen_js_func(sym, &ys, &func_body, ns, false, local_defs)
          }
          (_, _) => Err(format!("defn expected name arguments, got: {:?}", body)),
        },

        "defmacro" => Ok(format!("/* Unexpected macro {} */", xs)),
        "quote-replace" | "quasiquote" => Ok(format!("(/* Unexpected quasiquote {} */ null)", format_to_lisp(xs))),

        "raise" => {
          // not core syntax, but treat as macro for better debugging experience
          match body.get(0) {
            Some(m) => {
              let message: String = to_js_code(m, ns, local_defs)?;
              let data_code = match body.get(1) {
                Some(d) => to_js_code(d, ns, local_defs)?,
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
            let code = to_js_code(expr, ns, local_defs)?;
            let err_var = js_gensym("errMsg");
            let handler = to_js_code(handler, ns, local_defs)?;
            Ok(make_fn_wrapper(&format!(
              "try {{\nreturn {}\n}} catch ({}) {{\nreturn ({})({}.toString())\n}}",
              code, err_var, handler, err_var
            )))
          }
          (_, _) => Err(format!("try expected 2 nodes, got {:?}", body)),
        },
        "echo" | "println" => {
          // not core syntax, but treat as macro for better debugging experience
          let args = ys.clone().slice(1..);
          let args_code = gen_args_code(&args, ns, local_defs)?;
          Ok(format!("console.log({}printable({}))", var_prefix, args_code))
        }
        "exists?" => {
          // not core syntax, but treat as macro for availability
          match body.get(0) {
            Some(Calcit::Symbol(_sym, ..)) => {
              let target = to_js_code(&body[0], ns, local_defs)?; // TODO could be simpler
              return Ok(format!("(typeof {} !== 'undefined')", target));
            }
            Some(a) => Err(format!("exists? expected a symbol, got {}", a)),
            None => Err(format!("exists? expected 1 node, got {:?}", body)),
          }
        }
        "new" => match body.get(0) {
          Some(ctor) => {
            let args = body.clone().slice(1..);
            let args_code = gen_args_code(&args, ns, local_defs)?;
            Ok(format!("new {}({})", to_js_code(&ctor, ns, local_defs)?, args_code))
          }
          None => Err(format!("`new` expected constructor, got nothing, {}", xs)),
        },
        "instance?" => match (body.get(0), body.get(1)) {
          (Some(ctor), Some(v)) => Ok(format!(
            "({} instanceof {})",
            to_js_code(v, ns, local_defs)?,
            to_js_code(ctor, ns, local_defs)?
          )),
          (_, _) => Err(format!("instance? expected 2 arguments, got {:?}", body)),
        },
        "set!" => match (body.get(0), body.get(1)) {
          (Some(target), Some(v)) => Ok(format!(
            "{} = {}",
            to_js_code(target, ns, local_defs)?,
            to_js_code(v, ns, local_defs)?
          )),
          (_, _) => Err(format!("set! expected 2 nodes, got {:?}", body)),
        },
        _ if s.starts_with(".-") => {
          let name = s.strip_prefix(".-").unwrap();
          if name.is_empty() {
            Err(format!("invalid property accessor {}", s))
          } else {
            match body.get(0) {
              Some(obj) => Ok(format!("{}.{}", to_js_code(&obj, ns, local_defs)?, name)),
              None => Err(format!("property accessor takes only 1 argument, {:?}", xs)),
            }
          }
        }
        _ if s.starts_with('.') => {
          let name = s.strip_prefix('.').unwrap();
          if matches_js_var(name) {
            match body.get(0) {
              Some(obj) => {
                let args = body.clone().slice(1..);
                let args_code = gen_args_code(&args, ns, local_defs)?;
                Ok(format!("{}.{}({})", to_js_code(&obj, ns, local_defs)?, name, args_code))
              }
              None => Err(format!("expected 1 object, got {}", xs)),
            }
          } else {
            Err(format!("invalid member accessor {}", s))
          }
        }
        _ => {
          // TODO
          let args_code = gen_args_code(&body, ns, &local_defs)?;
          Ok(format!("{}({})", to_js_code(&head, ns, local_defs)?, args_code))
        }
      }
    }
    _ => {
      let args_code = gen_args_code(&body, ns, &local_defs)?;
      Ok(format!("{}({})", to_js_code(&head, ns, local_defs)?, args_code))
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
) -> Result<String, String> {
  // println!("gen symbol: {} {} {} {:?}", s, def_ns, ns, resolved);
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  if has_ns_part(s) {
    let ns_part = s.split('/').collect::<Vec<&str>>()[0]; // TODO
    if ns_part == "js" {
      Ok(escape_ns_var(s, "js"))
    } else {
      // TODO ditry code
      // TODO namespace part would be parsed during preprocessing
      match resolved {
        Some(ResolvedDef(r_ns, _r_def)) => {
          match internal_states::lookup_import(r_ns) {
            Some(prev) => {
              if (!prev.just_ns) || &prev.ns != r_ns {
                println!("conflicted imports: {:?} {:?}", prev, resolved);
                return Err(format!("Conflicted implicit ns import, {:?}", xs));
              }
            }
            None => {
              internal_states::track_import(
                r_ns.to_string(),
                CollectedImportItem {
                  ns: r_ns.clone(),
                  just_ns: true,
                  ns_in_str: false, /* TODO */
                },
              )?;
            }
          }
          Ok(escape_ns_var(s, r_ns))
        }
        Some(ResolvedRaw) => Err(format!("not going to generate from raw symbol, {}", s)),
        Some(ResolvedLocal) => Err(format!("symbol with ns should not be local, {}", s)),
        None => Err(format!("expected symbol with ns being resolved: {:?}", xs)),
      }
    }
  } else if is_builtin_js_proc(s) || is_proc_name(s) || is_syntax_name(s) {
    // return Ok(format!("{}{}", var_prefix, escape_var(s)));
    let proc_prefix = if ns == primes::CORE_NS {
      "$calcit_procs."
    } else {
      "$calcit."
    };
    return Ok(format!("{}{}", proc_prefix, escape_var(s)));
  } else if matches!(resolved, Some(ResolvedLocal)) || local_defs.contains(s) {
    Ok(escape_var(s))
  } else if let Some(ResolvedDef(r_ns, _r_def)) = resolved.clone() {
    if r_ns == primes::CORE_NS {
      // functions under core uses built $calcit module entry
      return Ok(format!("{}{}", var_prefix, escape_var(s)));
    }
    // TODO ditry code

    match internal_states::lookup_import(s) {
      Some(prev) => {
        if prev.ns != r_ns {
          // println!("{:?} {:?}", collected_imports, xs);
          println!("prev item: {:?}", prev);
          println!("map: {:?}", internal_states::clone_imports());
          return Err(format!("Conflicted implicit imports, {} {:?}", r_ns, xs,));
        }
      }
      None => {
        internal_states::track_import(
          s.to_string(),
          CollectedImportItem {
            ns: r_ns,
            just_ns: false,
            ns_in_str: false, /* TODO */
          },
        )?;
      }
    }

    Ok(escape_var(s))
  } else if def_ns == primes::CORE_NS {
    // local variales inside calcit.core also uses this ns
    println!("[Warn] detected variable inside core not resolved");
    Ok(format!("{}{}", var_prefix, escape_var(s)))
  } else if def_ns.is_empty() {
    Err(format!("Unexpected ns at symbol, {:?}", xs))
  } else if def_ns != ns {
    match internal_states::lookup_import(s) {
      Some(prev) => {
        if prev.ns != def_ns {
          // println!("{:?} {:?}", collected_imports, xs);
          return Err(format!("Conflicted implicit imports, probably via macro, {:?}", xs));
        }
      }
      None => {
        internal_states::track_import(
          s.to_string(),
          CollectedImportItem {
            ns: def_ns.to_string(),
            just_ns: false,
            ns_in_str: false,
          },
        )?;
      }
    }
    // TODO
    // probably via macro
    // TODO ditry code collecting imports

    Ok(escape_var(s))
  } else if def_ns == ns {
    println!("[Warn] detected unresolved variable {:?} in {}", xs, ns);
    Ok(escape_var(s))
  } else {
    println!("[Warn] Unexpected casecode gen for {:?} in {}", xs, ns);
    Ok(format!("{}{}", var_prefix, escape_var(s)))
  }
}

fn gen_let_code(body: &CalcitItems, local_defs: &HashSet<String>, xs: &Calcit, ns: &str) -> Result<String, String> {
  let mut let_def_body = body.clone();

  // defined new local variable
  let mut scoped_defs = local_defs.clone();
  let mut defs_code = String::from("");
  let mut variable_existed = false;
  let mut body_part = String::from("");

  // break unless nested &let is found
  loop {
    if let_def_body.len() <= 1 {
      return Err(format!("Unexpected empty content in let, {:?}", xs));
    }
    let pair = let_def_body[0].clone();
    let content = let_def_body.clone().slice(1..);

    match &pair {
      Calcit::Nil => {
        // non content defs_code

        for (idx, x) in content.iter().enumerate() {
          if idx == content.len() - 1 {
            body_part.push_str("return ");
            body_part.push_str(&to_js_code(x, ns, &scoped_defs)?);
            body_part.push_str(";\n");
          } else {
            body_part.push_str(&to_js_code(x, ns, &scoped_defs)?);
            body_part.push_str(";\n");
          }
        }
        break;
      }
      Calcit::List(xs) if xs.len() == 2 => {
        let def_name = xs[0].clone();
        let expr_code = xs[1].clone();

        match def_name {
          Calcit::Symbol(sym, ..) => {
            // TODO `let` inside expressions makes syntax error
            let left = escape_var(&sym);
            let right = to_js_code(&expr_code, &ns, &scoped_defs)?;

            defs_code.push_str(&format!("let {} = {};\n", left, right));

            if scoped_defs.contains(&sym) {
              variable_existed = true;
            } else {
              scoped_defs.insert(sym.clone());
            }

            if variable_existed {
              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs)?);
                  body_part.push_str(";\n");
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs)?);
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
              if content.len() == 1 {
                let child = content[0].clone();
                match child {
                  Calcit::List(ys) if ys.len() == 2 => match (&ys[0], &ys[1]) {
                    (Calcit::Symbol(sym, ..), Calcit::List(zs)) if sym == "&let" && zs.len() == 2 => {
                      let_def_body = ys.clone().slice(1..);
                      continue;
                    }
                    _ => (),
                  },
                  _ => (),
                }
              }

              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs)?);
                  body_part.push_str(";\n");
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs)?);
                  body_part.push_str(";\n");
                }
              }

              break;
            }
          }
          _ => return Err(format!("Expected symbol behind let, got: {}", &pair)),
        }
      }
      Calcit::List(_xs) => return Err(format!("expected pair of length 2, got: {}", &pair)),
      _ => return Err(format!("expected pair of a list of length 2, got: {}", pair)),
    }
  }
  return Ok(make_fn_wrapper(&format!("{}{}", defs_code, body_part)));
}

fn gen_args_code(body: &CalcitItems, ns: &str, local_defs: &HashSet<String>) -> Result<String, String> {
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
            to_js_code(x, ns, local_defs)?
          ));
          spreading = false
        } else {
          result.push_str(&to_js_code(&x, ns, &local_defs)?);
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
) -> Result<String, String> {
  // TODO default returnLabel="return "
  let mut result = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    // result = result & "// " & $x & "\n"
    if idx == xs.len() - 1 {
      result.push_str(return_label);
      result.push_str(&to_js_code(&x, ns, &local_defs)?);
      result.push_str(";\n");
    } else {
      result.push_str(&to_js_code(x, ns, &local_defs)?);
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
    format!(
      "\nif (arguments.length < {}) {{ throw new Error('Too few arguments') }}",
      args_count
    )
  } else if has_optional {
    format!("\nif (arguments.length < {}) {{ throw new Error('Too few arguments') }}\nif (arguments.length > {}) {{ throw new Error('Too many arguments') }}", args_count, args_count + optional_count )
  } else {
    format!(
      "\nif (arguments.length !== {}) {{ throw new Error('Args length mismatch') }}",
      args_count
    )
  };

  if !body.is_empty() && uses_recur(&body[body.len() - 1]) {
    // ugliy code for inlining tail recursion template
    let ret_var = js_gensym("ret");
    let times_var = js_gensym("times");
    let mut fn_def = format!("function {}({})", escape_var(name), args_code);
    fn_def.push_str(&format!("{{ {} {}", check_args, spreading_code));
    fn_def.push_str(&format!("\nlet {} = null;\n", ret_var));
    fn_def.push_str(&format!("let {} = 0;\n", times_var));
    fn_def.push_str("while(true) { /* Tail Recursion */\n");
    fn_def.push_str(&format!(
      "if ({} > 10000) {{ throw new Error('Expected tail recursion to exist quickly') }}\n",
      times_var
    ));
    fn_def.push_str(&list_to_js_code(&body, ns, local_defs, &format!("{} =", ret_var))?);
    fn_def.push_str(&format!("if ({} instanceof {}CrDataRecur) {{\n", ret_var, var_prefix));
    fn_def.push_str(&check_args.replace("arguments.length", &format!("{}.args.length", ret_var)));
    fn_def.push_str(&format!("\n[ {} ] = {}.args;\n", args_code, ret_var));
    fn_def.push_str(&spreading_code);
    fn_def.push_str(&format!("{} += 1;\ncontinue;\n", times_var));
    fn_def.push_str(&format!("}} else {{ return {} }} ", ret_var));
    fn_def.push_str("}\n}");

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
      list_to_js_code(&body, ns, local_defs, "return ")?
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
  let mut result: Vec<String> = vec![];

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
    deps_graph.insert(k.to_string(), deps_info);
  }
  // echo depsGraph
  def_names.sort();
  for x in def_names {
    let mut inserted = false;
    for (idx, y) in result.iter().enumerate() {
      if deps_graph.contains_key(y) && deps_graph[y].contains(&x) {
        result.insert(idx, x.clone());
        inserted = true;
        break;
      }
    }
    if inserted {
      continue;
    }
    result.push(x.clone());
  }

  result
}

fn write_file_if_changed(filename: &str, content: &str) -> bool {
  if Path::new(filename).exists() && fs::read_to_string(filename).unwrap() == content {
    return false;
  }
  let _ = fs::write(filename, content);
  true
}

pub fn emit_js(entry_ns: &str) -> Result<(), String> {
  let code_emit_path = "js-out/"; // TODO
  if !Path::new(code_emit_path).exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let mut unchanged_ns: HashSet<String> = HashSet::new();

  let program = program::clone_evaled_program();
  for (ns, file) in program {
    // println!("start handling: {}", ns);
    // side-effects, reset tracking state
    let _ = internal_states::clear_imports(); // reset

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
    let procs_lib = format!("\"{}\"", "@calcit/procs".escape_debug());
    let mut import_code = String::from("");

    let mut defs_code = String::from(""); // code generated by functions
    let mut vals_code = String::from(""); // code generated by thunks

    if ns == "calcit.core" {
      import_code.push_str(&format!(
        "\nimport {{kwd, arrayToList, listToArray, CrDataRecur}} from {};\n",
        procs_lib
      ));
      import_code.push_str(&format!("\nimport * as $calcit_procs from {};\n", procs_lib));
      import_code.push_str(&format!("\nexport * from {};\n", procs_lib));
    } else {
      import_code.push_str(&format!("\nimport * as $calcit from {};\n", core_lib));
    }

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
        Calcit::Fn(_name, _def_ns, _, _, args, code) => {
          defs_code.push_str(&gen_js_func(&def, args, code, &ns, true, &def_names)?);
        }
        Calcit::Thunk(code) => {
          // TODO need topological sorting for accuracy
          // values are called directly, put them after fns
          vals_code.push_str(&format!(
            "\nexport var {} = {};\n",
            escape_var(&def),
            to_js_code(code, &ns, &def_names)?
          ));
        }
        Calcit::Macro(..) => {
          // macro should be handled during compilation, psuedo code
          defs_code.push_str(&format!("\nexport var {} = () => {{/* Macro */}}\n", escape_var(&def)));
          defs_code.push_str(&format!("\n{}.isMacro = true;\n", escape_var(&def)));
        }
        Calcit::Syntax(_, _) => {
          // should he handled inside compiler
        }
        _ => {
          println!("[Warn] strange case for generating a definition: {}", f)
        }
      }
    }

    let collected_imports = internal_states::clone_imports().unwrap(); // ignore unlocking details
    if !collected_imports.is_empty() {
      // echo "imports: ", collected_imports
      for def in collected_imports.keys() {
        let item = collected_imports[def].clone();
        // echo "implicit import ", defNs, "/", def, " in ", ns
        if item.just_ns {
          let import_target = if is_cirru_string(&item.ns) {
            format!("\"{}\"", item.ns[1..].escape_debug())
          } else {
            to_js_import_name(&item.ns, false) // TODO js_mode
          };
          import_code.push_str(&format!(
            "\nimport * as {} from {};\n",
            escape_ns(&item.ns),
            import_target
          ));
        } else {
          let import_target = to_js_import_name(&item.ns, false); // TODO js_mode
          import_code.push_str(&format!("\nimport {{ {} }} from {};\n", escape_var(def), import_target));
        }
      }
    }

    let js_file_path = format!("{}{}", code_emit_path, to_js_file_name(&ns, false)); // TODO mjs_mode
    let wrote_new = write_file_if_changed(&js_file_path, &format!("{}\n{}\n{}", import_code, defs_code, vals_code));
    if wrote_new {
      println!("Emitted js file: {}", js_file_path);
    } else {
      unchanged_ns.insert(ns.to_string());
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

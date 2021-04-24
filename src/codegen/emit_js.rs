use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use crate::builtins::meta::{js_gensym, reset_js_gensym_index};
use crate::primes;
use crate::primes::{Calcit, CalcitItems, SymbolResolved, SymbolResolved::*};
use crate::program;
use crate::util::string::matches_js_var;

// track if it's the first compilation
static FIRST_COMPILATION: AtomicBool = AtomicBool::new(true);

#[derive(Debug, PartialEq, Clone)]
struct CollectedImportItem {
  ns: String,
  just_ns: bool,
  ns_in_str: bool,
}

lazy_static! {
  // caches program data for detecting incremental changes of libs
  static ref GLOBAL_PREVIOUS_PROGRAM_CACHES: Mutex<HashMap<String, HashSet<String>>> = Mutex::new(HashMap::new());

  // TODO mutable way of collect things of a single tile
  static ref GLOBAL_COLLECTED_IMPORTS: Mutex <HashMap<String, CollectedImportItem>> = Mutex::new(HashMap::new());
}

fn to_js_import_name(ns: &str, mjs_mode: bool) -> String {
  let mut xs: String = String::from("./");
  xs.push_str(ns);
  if mjs_mode {
    xs.push_str(".mjs");
  }
  // currently use `import "./ns.name"`
  xs.escape_debug().to_string()
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

fn has_ns_part(x: &str) -> bool {
  match x.find('/') {
    Some(try_slash_pos) => try_slash_pos >= 1 && try_slash_pos < x.len() - 1,
    None => false,
  }
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
  format!("${}", escape_var(name))
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

fn quote_to_js(xs: &Calcit, var_prefix: &str) -> String {
  match xs {
    Calcit::Symbol(s, ..) => format!("new {}CrDataSymbol({})", var_prefix, escape_cirru_str(&s)),
    Calcit::Str(s) => escape_cirru_str(&s),
    Calcit::Bool(b) => b.to_string(),
    Calcit::Number(n) => n.to_string(),
    Calcit::Nil => String::from("null"),
    Calcit::List(ys) => {
      let mut chunk = String::from("");
      for y in ys {
        if !chunk.is_empty() {
          chunk.push_str(", ");
        }
        chunk.push_str(&quote_to_js(y, var_prefix));
      }
      format!("new {}CrDataList([{}])", var_prefix, chunk)
    }
    Calcit::Keyword(s) => format!("{}kwd({})", var_prefix, escape_cirru_str(&s)),
    _ => unreachable!(format!("Unpexpected data in quote for js: {}", xs)),
  }
}

fn make_let_with_bind(left: &str, right: &str, body: &str) -> String {
  format!("(function __let__({}){{\n{} }})({})", left, body, right)
}

fn make_let_with_wrapper(left: &str, right: &str, body: &str) -> String {
  format!("(function __let__(){{ \nlet {} = {};\n {} }})()", left, right, body)
}

fn make_fn_wrapper(body: &str) -> String {
  format!("(function __fn__(){{\n{}\n}})()", body)
}

fn to_js_code(xs: &Calcit, ns: &str, local_defs: &HashSet<String>) -> String {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  match xs {
    Calcit::Symbol(s, def_ns, resolved) => gen_symbol_code(s, &def_ns, resolved, ns, xs, local_defs),
    Calcit::Str(s) => escape_cirru_str(&s),
    Calcit::Bool(b) => b.to_string(),
    Calcit::Number(n) => n.to_string(),
    Calcit::Nil => String::from("null"),
    Calcit::Keyword(s) => format!("{}kwd(\"{}\")", var_prefix, s.escape_debug()),
    Calcit::List(ys) => {
      if ys.is_empty() {
        println!("[Warn] Unpexpected empty list");
        return String::from("()");
      }

      let head = ys[0].clone();
      let body = ys.clone().slice(1..);
      match &head {
        Calcit::Symbol(s, ..) => {
          match s.as_str() {
            "if" => {
              if body.len() < 2 {
                println!("{}", xs);
                panic!("need branches for if")
              }
              let false_branch = if body.len() >= 3 {
                to_js_code(&body[2], ns, local_defs)
              } else {
                String::from("null")
              };
              format!(
                "( {} ? {} : {} )",
                to_js_code(&body[0], ns, local_defs),
                to_js_code(&body[1], ns, local_defs),
                false_branch
              )
            }
            "&let" => gen_let_code(&body, local_defs, &xs, ns),
            ";" => format!("(/* {} */ null)", Calcit::List(body)),
            "do" => {
              // TODO use nil
              let mut body_part: String = String::from("");
              for (idx, x) in body.iter().enumerate() {
                if idx > 0 {
                  body_part.push_str(";\n");
                }
                if idx == body.len() - 1 {
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(&x, ns, local_defs));
                } else {
                  body_part.push_str(&to_js_code(&x, ns, local_defs));
                }
              }
              make_fn_wrapper(&body_part)
            }

            "quote" => {
              if body.is_empty() {
                println!("Unpexpected empty body, {}", xs);
                panic!("Unpexpected empty body");
              }
              quote_to_js(&body[0], var_prefix)
            }
            "defatom" => {
              if body.len() != 2 {
                println!("defatom expects 2 nodes, {}", xs);
                panic!("defatom expects 2 nodes")
              }
              let atom_name = body[0].clone();
              let atom_expr = body[1].clone();
              match &atom_name {
                Calcit::Symbol(sym, ..) => {
                  // let _name = escape_var(sym); // TODO
                  let atom_path = format!("\"{}\"", format!("{}/{}", ns, sym.clone()).escape_debug());
                  format!(
                    "\n({}peekDefatom({}) ?? {}defatom({}, {}))\n",
                    &var_prefix,
                    &atom_path,
                    &var_prefix,
                    &atom_path,
                    &to_js_code(&atom_expr, ns, local_defs)
                  )
                }
                _ => {
                  println!("expects atomName in symbol, {}", xs);
                  panic!("expects atomName in symbol")
                }
              }
            }

            "defn" => {
              if body.len() < 2 {
                println!("Expected name, args, code for gennerating func, too short: {}", xs);
                panic!("Expected name, args, code for gennerating func, too short");
              }
              let func_name = body[0].clone();
              let func_args = body[1].clone();
              let func_body = body.slice(2..);
              match (func_name, func_args) {
                (Calcit::Symbol(sym, ..), Calcit::List(ys)) => {
                  gen_js_func(&sym, &ys, &func_body, ns, false, local_defs)
                }
                (a, b) => panic!("expected symbol and list, got: {} {}", a, b),
              }
            }

            "defmacro" => format!("/* Unpexpected macro {} */", xs),
            "quote-replace" => format!("/* Unpexpected quote-replace {} */", xs),
            "raise" => {
              // not core syntax, but treat as macro for better debugging experience
              if body.is_empty() || body.len() > 2 {
                println!("expected 1~2 arguments: {:?}", body);
                panic!("expected 1~2 arguments:")
              }
              let message: String = to_js_code(&body[0], ns, local_defs);
              let mut data = String::from("null");
              if body.len() >= 2 {
                data = to_js_code(&body[1], ns, local_defs);
              }
              let err_var = js_gensym("err");
              make_fn_wrapper(&format!(
                "let {} = new Error({});\n {}.data = {};\n throw {};",
                err_var, message, err_var, data, err_var
              ))
            }
            "try" => {
              if body.len() != 2 {
                panic!("expected 2 argument, {:?}", body)
              }
              let code = to_js_code(&body[0], ns, local_defs);
              let err_var = js_gensym("errMsg");
              let handler = to_js_code(&body[1], ns, local_defs);
              make_fn_wrapper(&format!(
                "try {{\nreturn {}\n}} catch ({}) {{\nreturn ({})({}.toString())\n}}",
                code, err_var, handler, err_var
              ))
            }
            "echo" | "println" => {
              // not core syntax, but treat as macro for better debugging experience
              let args = ys.clone().slice(1..);
              let args_code = gen_args_code(&args, ns, local_defs);
              format!("console.log({}printable({}))", var_prefix, args_code)
            }
            "exists?" => {
              // not core syntax, but treat as macro for availability
              if body.len() != 1 {
                panic!("expected 1 argument, {}", xs)
              }
              let item = body[0].clone();
              match &item {
                Calcit::Symbol(_sym, ..) => {
                  let target = to_js_code(&item, ns, local_defs);
                  return format!("(typeof {} !== 'undefined')", target);
                }
                _ => panic!("expected a symbol, got: {}", xs),
              }
            }
            "new" => {
              if ys.len() < 2 {
                panic!("`new` takes at least an object constructor {:?}", xs)
              }
              let ctor = ys[1].clone();
              let args = ys.clone().slice(1..);
              let args_code = gen_args_code(&args, ns, local_defs);
              format!("new {}({})", to_js_code(&ctor, ns, local_defs), args_code)
            }
            "instance?" => {
              if ys.len() != 3 {
                panic!("`instance?` takes a constructor and a value, {}", xs);
              }
              let ctor = ys[1].clone();
              let v = ys[2].clone();

              format!(
                "({} instanceof {})",
                to_js_code(&v, ns, local_defs),
                to_js_code(&ctor, ns, local_defs)
              )
            }
            "set!" => {
              if ys.len() != 3 {
                panic!("set! takes a operand and a value, {}", xs);
              }
              format!(
                "{} = {}",
                to_js_code(&ys[1], ns, local_defs),
                to_js_code(&ys[2], ns, local_defs)
              )
            }
            _ => {
              // TODO
              let token = s;
              if token.len() > 2 && &token[0..1] == ".-" && matches_js_var(&token[2..]) {
                let name = token[2..].to_string();
                if ys.len() != 2 {
                  panic!("property accessor takes only 1 argument, {:?}", xs);
                }
                let obj = ys[1].clone();
                format!("{}.{}", to_js_code(&obj, ns, local_defs), name)
              } else if token.len() > 1 && token.starts_with('.') && matches_js_var(&token[1..]) {
                let name: String = token[1..].to_string();
                if ys.len() < 2 {
                  panic!("property accessor takes at least 1 argument, {:?}", xs);
                }
                let obj = ys[1].clone();
                let args = ys.clone().slice(2..);
                let args_code = gen_args_code(&args, ns, local_defs);
                format!("{}.{}({})", to_js_code(&obj, ns, local_defs), name, args_code)
              } else {
                let args_code = gen_args_code(&body, ns, &local_defs);
                format!("{}({})", to_js_code(&head, ns, local_defs), args_code)
              }
            }
          }
        }
        _ => {
          let args_code = gen_args_code(&body, ns, &local_defs);
          format!("{}({})", to_js_code(&head, ns, local_defs), args_code)
        }
      }
    }
    a => unreachable!(format!("[Warn] unknown kind to gen js code: {}", a)),
  }
}

fn gen_symbol_code(
  s: &str,
  def_ns: &str,
  resolved: &Option<primes::SymbolResolved>,
  ns: &str,
  xs: &Calcit,
  local_defs: &HashSet<String>,
) -> String {
  let var_prefix = if ns == "calcit.core" { "" } else { "$calcit." };
  if has_ns_part(s) {
    let ns_part = s.split('/').collect::<Vec<&str>>()[0]; // TODO
    if ns_part == "js" {
      escape_ns_var(s, "js")
    } else {
      // TODO ditry code
      match &resolved {
        Some(ResolvedDef(r_ns, _r_def)) => {
          let collected_imports = &mut GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
          if collected_imports.contains_key(r_ns) {
            let prev = collected_imports[r_ns].clone();
            if (!prev.just_ns) || &prev.ns != r_ns {
              println!("conflicted imports: {:?} {:?}", prev, resolved);
              panic!("Conflicted implicit ns import, {:?}", xs);
            }
          } else {
            collected_imports.insert(
              r_ns.to_string(),
              CollectedImportItem {
                ns: r_ns.clone(),
                just_ns: true,
                ns_in_str: false, /* TODO */
              },
            );
          }
          escape_ns_var(s, r_ns)
        }
        Some(ResolvedLocal) => panic!("TODO"),
        None => panic!("Expected symbol with ns being resolved: {:?}", xs),
      }
    }
  } else if is_builtin_js_proc(s) {
    return format!("{}{}", var_prefix, escape_var(s));
  } else if matches!(resolved, Some(ResolvedLocal)) && local_defs.contains(s) {
    escape_var(s)
  } else if let Some(ResolvedDef(r_ns, _r_def)) = resolved.clone() {
    if r_ns == primes::CORE_NS {
      // functions under core uses built $calcit module entry
      return format!("{}{}", var_prefix, escape_var(s));
    }
    // TODO ditry code
    let collected_imports = &mut GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
    if collected_imports.contains_key(s) {
      let prev = collected_imports[s].clone();
      if prev.ns != r_ns {
        println!("{:?} {:?}", collected_imports, xs);
        panic!("Conflicted implicit imports, {:?}", xs);
      }
    } else {
      collected_imports.insert(
        s.to_string(),
        CollectedImportItem {
          ns: r_ns,
          just_ns: false,
          ns_in_str: false, /* TODO */
        },
      );
    }
    escape_var(s)
  } else if def_ns == primes::CORE_NS {
    // local variales inside calcit.core also uses this ns
    println!("[Warn] detected variable inside core not resolved");
    format!("{}{}", var_prefix, escape_var(s))
  } else if def_ns.is_empty() {
    panic!("Unpexpected ns at symbol, {:?}", xs);
  } else if def_ns != ns {
    let collected_imports = &mut GLOBAL_COLLECTED_IMPORTS.lock().unwrap(); // TODO
                                                                           // probably via macro
                                                                           // TODO ditry code collecting imports
    if collected_imports.contains_key(s) {
      let prev = collected_imports[s].clone();
      if prev.ns != def_ns {
        println!("{:?} {:?}", collected_imports, xs);
        panic!("Conflicted implicit imports, probably via macro, {:?}", xs);
      }
      return escape_var(s);
    } else {
      collected_imports.insert(
        s.to_string(),
        CollectedImportItem {
          ns: def_ns.to_string(),
          just_ns: false,
          ns_in_str: false,
        },
      );
    }
    escape_var(s)
  } else if def_ns == ns {
    println!("[Warn] detected unresolved variable {:?} in {}", xs, ns);
    escape_var(s)
  } else {
    println!("[Warn] Unpexpected casecode gen for {:?} in {}", xs, ns);
    format!("{}{}", var_prefix, escape_var(s))
  }
}

fn gen_let_code(body: &CalcitItems, local_defs: &HashSet<String>, xs: &Calcit, ns: &str) -> String {
  let mut let_def_body = body.clone();

  // defined new local variable
  let mut scoped_defs = local_defs.clone();
  let mut defs_code = String::from("");
  let mut variable_existed = false;
  let mut body_part = String::from("");

  // break unless nested &let is found
  loop {
    if let_def_body.len() <= 1 {
      panic!("Unpexpected empty content in let, {:?}", xs);
    }
    let pair = let_def_body[0].clone();
    let content = let_def_body.slice(1..);

    match &pair {
      Calcit::List(xs) if xs.len() == 2 => {
        let def_name = xs[0].clone();
        let expr_code = xs[1].clone();

        match def_name {
          Calcit::Symbol(sym, ..) => {
            // TODO `let` inside expressions makes syntax error
            let left = escape_var(&sym);
            let right = to_js_code(&expr_code, &ns, &scoped_defs);

            defs_code.push_str(&format!("let {} = {};\n", left, right));

            if scoped_defs.contains(&sym) {
              variable_existed = true;
            } else {
              scoped_defs.insert(sym.clone());
            }

            if variable_existed {
              for (idx, x) in content.clone().slice(1..).iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs));
                  body_part.push_str(";\n");
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs));
                  body_part.push_str(";\n");
                }
              }

              // first variable is using conflicted name
              if local_defs.contains(&sym) {
                return make_let_with_bind(&left, &right, &body_part);
              } else {
                return make_let_with_wrapper(&left, &right, &body_part);
              }
            } else {
              if content.len() == 1 {
                let child = content[0].clone();
                match child {
                  Calcit::List(ys) if ys.len() == 2 => match (&ys[0], &ys[1]) {
                    (Calcit::Symbol(sym, ..), Calcit::List(zs)) if sym == "&let" && zs.len() == 2 => {
                      let_def_body = ys.clone().slice(1..);
                    }
                    _ => (),
                  },
                  _ => (),
                }
              }

              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  body_part.push_str("return ");
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs));
                  body_part.push_str(";\n");
                } else {
                  body_part.push_str(&to_js_code(x, ns, &scoped_defs));
                  body_part.push_str(";\n");
                }
              }

              break;
            }
          }
          _ => panic!("Expected symbol behind let, got: {}", &pair),
        }
      }
      Calcit::List(_xs) => panic!("expected pair of length 2, got: {}", &pair),
      _ => panic!("expected pair of a list of length 2, got: {}", pair),
    }
  }
  return make_fn_wrapper(&format!("{}{}", defs_code, body_part));
}

fn gen_args_code(body: &CalcitItems, ns: &str, local_defs: &HashSet<String>) -> String {
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
            to_js_code(x, ns, local_defs)
          ));
          spreading = false
        } else {
          result.push_str(&to_js_code(&x, ns, &local_defs));
        }
      }
    }
  }
  result
}

fn list_to_js_code(xs: &CalcitItems, ns: &str, local_defs: HashSet<String>, return_label: &str) -> String {
  // TODO default returnLabel="return "
  let mut result = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    // result = result & "// " & $x & "\n"
    if idx == xs.len() - 1 {
      result.push_str(return_label);
      result.push_str(&to_js_code(&x, ns, &local_defs));
      result.push_str(";\n");
    } else {
      result.push_str(&to_js_code(x, ns, &local_defs));
      result.push_str(";\n");
    }
  }
  result
}

fn uses_recur(xs: &Calcit) -> bool {
  match xs {
    Calcit::Symbol(s, ..) => s == "recur",
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
) -> String {
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
      _ => panic!("Expected symbol for arg, {}", x),
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
    let mut fn_fefinition = format!("function {}({})", escape_var(name), args_code);
    fn_fefinition.push_str(&format!("{{ {} {}", check_args, spreading_code));
    fn_fefinition.push_str(&format!("\nlet {} = null;\n", ret_var));
    fn_fefinition.push_str(&format!("let {} = 0;\n", times_var));
    fn_fefinition.push_str("while(true) {{ /* Tail Recursion */\n");
    fn_fefinition.push_str(&format!(
      "if ({} > 10000) {{ throw new Error('Expected tail recursion to exist quickly') }}\n",
      times_var
    ));
    fn_fefinition.push_str(&list_to_js_code(&body, ns, local_defs, &format!("{} =", ret_var)));
    fn_fefinition.push_str(&format!("if ({} instanceof {}CrDataRecur) {{\n", ret_var, var_prefix));
    fn_fefinition.push_str(&check_args.replace("arguments.length", &format!("{}.args.length", ret_var)));
    fn_fefinition.push_str(&format!("\n[ {} ] = {}.args;\n", args_code, ret_var));
    fn_fefinition.push_str(&spreading_code);
    fn_fefinition.push_str(&format!("{} += 1;\ncontinue;\n", times_var));
    fn_fefinition.push_str(&format!("}} else {{ return {} }}  ", ret_var));
    fn_fefinition.push_str("}\n}");

    let export_mark = if exported {
      format!("export let = {}", escape_var(name))
    } else {
      String::from("")
    };
    return format!("{}{}\n", export_mark, fn_fefinition);
  } else {
    let fn_definition = format!(
      "function {}({}) {{ {}{}\n{} }}",
      escape_var(name),
      args_code,
      check_args,
      spreading_code,
      list_to_js_code(&body, ns, local_defs, "return ")
    );
    let export_mark = if exported { "export " } else { "" };
    return format!("{}{}\n", export_mark, fn_definition);
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

pub fn emit_js(program_data: &HashMap<String, program::ProgramFileData>, entry_ns: &str) {
  let code_emit_path = "js-out/"; // TODO
  if !Path::new(code_emit_path).exists() {
    let _ = fs::create_dir(code_emit_path);
  }

  let mut unchanged_ns: HashSet<String> = HashSet::new();

  let collected_imports = &mut GLOBAL_COLLECTED_IMPORTS.lock().unwrap();
  let previous_program_caches = &mut GLOBAL_PREVIOUS_PROGRAM_CACHES.lock().unwrap();

  for (ns, file) in program_data {
    // side-effects, reset tracking state
    collected_imports.clear(); // reset

    let mut defs_in_current: HashSet<String> = HashSet::new();
    for k in file.defs.keys() {
      defs_in_current.insert(k.clone());
    }

    if !FIRST_COMPILATION.load(Ordering::Relaxed) {
      let app_pkg_name = entry_ns.split('.').collect::<Vec<&str>>()[0];
      let pkg_name = ns.split('.').collect::<Vec<&str>>()[0]; // TODO simpler
      if app_pkg_name != pkg_name
        && previous_program_caches.contains_key(ns)
        && (previous_program_caches[ns] == defs_in_current)
      {
        continue; // since libraries do not have to be re-compiled
      }
    }
    // remember defs of each ns for comparing
    previous_program_caches.insert(ns.to_string(), defs_in_current);

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
      import_code.push_str(&format!("\"import * as $calcit_procs from {};\"", procs_lib));
      import_code.push_str(&format!("\"export * from {};\"", procs_lib));
    } else {
      import_code.push_str(&format!("\nimport * as $calcit from {};\n", core_lib));
    }

    let mut def_names: HashSet<String> = HashSet::new(); // multiple parts of scoped defs need to be tracked

    // tracking top level scope definitions
    for def in file.defs.keys() {
      def_names.insert(def.clone());
    }

    let deps_in_order = sort_by_deps(&file.defs);
    // echo "deps order: ", deps_in_order

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

      let f = file.defs[&def].clone();

      match &f {
        Calcit::Proc(..) => {
          defs_code.push_str(&format!(
            "\"var {} = $calcit_procs.{};\"",
            escape_var(&def),
            escape_var(&def)
          ));
        }
        Calcit::Fn(_name, def_ns, _, _, args, code) => {
          defs_code.push_str(&gen_js_func(&def, args, code, def_ns, true, &def_names));
        }
        Calcit::Thunk(code) => {
          // TODO need topological sorting for accuracy
          // values are called directly, put them after fns
          vals_code.push_str(&format!(
            "\"export var {} = {};\"",
            escape_var(&def),
            to_js_code(code, &ns, &def_names)
          ));
        }
        Calcit::Macro(..) => {
          // macro should be handled during compilation, psuedo code
          defs_code.push_str(&format!("\"export var {} = () => {{/* Macro */}}\"", escape_var(&def)));
          defs_code.push_str(&format!("\"{}.isMacro = true;\"", escape_var(&def)));
        }
        Calcit::Syntax(_, _) => {
          // should he handled inside compiler
        }
        _ => {
          println!("[Warn] strange case for generating a definition: {}", f)
        }
      }

      if !collected_imports.is_empty() {
        // echo "imports: ", collected_imports
        for def in collected_imports.keys() {
          let item = collected_imports[def].clone();
          // echo "implicit import ", defNs, "/", def, " in ", ns
          if item.just_ns {
            let import_target = if item.ns_in_str {
              format!("\"{}\"", item.ns.escape_debug())
            } else {
              to_js_import_name(&item.ns, false) // TODO js_mode
            };
            import_code.push_str(&format!(
              "\"import * as {} from {};\"",
              escape_ns(&item.ns),
              import_target
            ));
          } else {
            let import_target = to_js_import_name(&item.ns, false); // TODO js_mode
            import_code.push_str(&format!("\"import {{ {} }} from {};\"", escape_var(def), import_target));
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
  }

  if !unchanged_ns.is_empty() {
    println!("\n... and {} files not changed.", unchanged_ns.len());
  }

  FIRST_COMPILATION.store(false, Ordering::SeqCst); // TODO
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

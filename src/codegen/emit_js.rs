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
use crate::util::string::matchesJsVar;

const C_LINE: &str = "\n";
const C_CURLYL: &str = "{";
const C_CURLYR: &str = "}";

// track if it's the first compilation
static first_compilation: AtomicBool = AtomicBool::new(true);

#[derive(Debug, PartialEq, Clone)]
struct CollectedImportItem {
  ns: String,
  just_ns: bool,
  ns_in_str: bool,
}

lazy_static! {
  // caches program data for detecting incremental changes of libs
  static ref global_previous_program_caches: Mutex<HashMap<String, HashSet<String>>> = Mutex::new(HashMap::new());

  // TODO mutable way of collect things of a single tile
  static ref global_collected_imports: Mutex <HashMap<String, CollectedImportItem>> = Mutex::new(HashMap::new());
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
  let mut xs: String = String::from("");
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

  if name == "if" {
    return String::from("_IF_");
  }
  if name == "do" {
    return String::from("_DO_");
  }
  if name == "else" {
    return String::from("_ELSE_");
  }
  if name == "let" {
    return String::from("_LET_");
  }
  if name == "case" {
    return String::from("_CASE_");
  }
  if name == "-" {
    return String::from("_SUB_");
  }

  return name
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
    .replace("\\", "_BSL_");
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
  let nsPart = pieces[0];
  let defPart = pieces[1];
  if nsPart == "js" {
    defPart.to_string()
  } else if defPart == "@" {
    // TODO special syntax for js, using module directly, need a better solution
    escape_ns(ns)
  } else {
    format!("{}.{}", escape_ns(ns), escape_var(&defPart))
  }
}

// tell compiler to handle namespace code generation
fn is_builtIn_js_proc(name: &str) -> bool {
  match name {
    "aget"
    | "aset"
    | "extract-cirru-edn"
    | "to-cirru-edn"
    | "to-js-data"
    | "to-calcit-data"
    | "printable"
    | "instance?"
    | "timeout-call"
    | "load-console-formatter!" => true,
    _ => false,
  }
}

// code generated from calcit.core.cirru may not be faster enough,
// possible way to use code from calcit.procs.ts
fn is_preferredJsProc(name: &str) -> bool {
  match name {
    "number?" | "keyword?" | "map?" | "nil?" | "list?" | "set?" | "string?" | "fn?" | "bool?" | "atom?" | "record?"
    | "starts-with?" => true,
    _ => false,
  }
}

fn escapeCirruStr(s: &str) -> String {
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

fn quote_to_js(xs: &Calcit, varPrefix: &str) -> String {
  match xs {
    Calcit::Symbol(s, ..) => format!("new {}CrDataSymbol({})", varPrefix, escapeCirruStr(&s)),
    Calcit::Str(s) => escapeCirruStr(&s),
    Calcit::Bool(b) => b.to_string(),
    Calcit::Number(n) => n.to_string(),
    Calcit::Nil => String::from("null"),
    Calcit::List(ys) => {
      let mut chunk = String::from("");
      for y in ys {
        if chunk.len() > 0 {
          chunk.push_str(", ");
        }
        chunk.push_str(&quote_to_js(y, varPrefix));
      }
      format!("new {}CrDataList([{}])", varPrefix, chunk)
    }
    Calcit::Keyword(s) => format!("{}kwd({})", varPrefix, escapeCirruStr(&s)),
    _ => unreachable!(format!("Unpexpected data in quote for js: {}", xs)),
  }
}

fn make_let_with_bind(left: &str, right: &str, body: &str) -> String {
  format!("(function __let__({}){{\n{} }})({})", left, body, right)
}

fn make_let_with_wrapper(left: &str, right: &str, body: &str) -> String {
  format!("(function __let__(){{ \nlet {} = {};\n {} }})()", left, right, body)
}

fn makeFnWrapper(body: &str) -> String {
  format!("(function __fn__(){{\n{}\n}})()", body)
}

fn toJsCode(xs: &Calcit, ns: &str, localDefs: &HashSet<String>) -> String {
  let varPrefix = if ns == "calcit.core" { "" } else { "$calcit." };
  match xs {
    Calcit::Symbol(s, def_ns, resolved) => gen_symbol_code(s, &def_ns, resolved, ns, xs, localDefs),
    Calcit::Str(s) => escapeCirruStr(&s),
    Calcit::Bool(b) => b.to_string(),
    Calcit::Number(n) => n.to_string(),
    Calcit::Nil => String::from("null"),
    Calcit::Keyword(s) => format!("{}kwd(\"{}\")", varPrefix, s.escape_debug()),
    Calcit::List(ys) => {
      if ys.len() == 0 {
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
              let falseBranch = if body.len() >= 3 {
                toJsCode(&body[2], ns, localDefs)
              } else {
                String::from("null")
              };
              format!(
                "( {} ? {} : {} )",
                toJsCode(&body[0], ns, localDefs),
                toJsCode(&body[1], ns, localDefs),
                falseBranch
              )
            }
            "&let" => gen_let_code(&body, localDefs, &xs, ns),
            ";" => format!("(/* {} */ null)", Calcit::List(body)),
            "do" => {
              // TODO use nil
              let mut bodyPart: String = String::from("");
              for (idx, x) in body.iter().enumerate() {
                if idx > 0 {
                  bodyPart.push_str(";\n");
                }
                if idx == body.len() - 1 {
                  bodyPart.push_str("return ");
                  bodyPart.push_str(&toJsCode(&x, ns, localDefs));
                } else {
                  bodyPart.push_str(&toJsCode(&x, ns, localDefs));
                }
              }
              return makeFnWrapper(&bodyPart);
            }

            "quote" => {
              if body.len() < 1 {
                panic!(format!("Unpexpected empty body, {}", xs));
              }
              return quote_to_js(&body[0], varPrefix);
            }
            "defatom" => {
              if body.len() != 2 {
                panic!(format!("defatom expects 2 nodes, {}", xs))
              }
              let atomName = body[0].clone();
              let atomExpr = body[1].clone();
              match &atomName {
                Calcit::Symbol(sym, ..) => {
                  // let _name = escape_var(sym); // TODO
                  let atomPath = format!("\"{}\"", format!("{}/{}", ns, sym.clone()).escape_debug());
                  format!(
                    "\n({}peekDefatom({}) ?? {}defatom({}, {}))\n",
                    &varPrefix,
                    &atomPath,
                    &varPrefix,
                    &atomPath,
                    &toJsCode(&atomExpr, ns, localDefs)
                  )
                }
                _ => panic!(format!("expects atomName in symbol, {}", xs)),
              }
            }

            "defn" => {
              if body.len() < 2 {
                panic!(format!(
                  "Expected name, args, code for gennerating func, too short: {}",
                  xs
                ))
              }
              let funcName = body[0].clone();
              let funcArgs = body[1].clone();
              let funcBody = body.clone().slice(2..);
              match (funcName, funcArgs) {
                (Calcit::Symbol(sym, ..), Calcit::List(ys)) => {
                  return genJsFunc(&sym, &ys, &funcBody, ns, false, localDefs)
                }
                (a, b) => panic!(format!("expected symbol and list, got: {} {}", a, b)),
              }
            }

            "defmacro" => format!("/* Unpexpected macro {} */", xs),
            "quote-replace" => format!("/* Unpexpected quote-replace {} */", xs),
            "raise" => {
              // not core syntax, but treat as macro for better debugging experience
              if body.len() < 1 || body.len() > 2 {
                panic!(format!("expected 1~2 arguments: {:?}", body))
              }
              let message: String = toJsCode(&body[0], ns, localDefs);
              let mut data = String::from("null");
              if body.len() >= 2 {
                data = toJsCode(&body[1], ns, localDefs);
              }
              let errVar = js_gensym("err");
              makeFnWrapper(&format!(
                "let {} = new Error({});\n {}.data = {};\n throw {};",
                errVar, message, errVar, data, errVar
              ))
            }
            "try" => {
              if body.len() != 2 {
                panic!(format!("expected 2 argument, {:?}", body))
              }
              let code = toJsCode(&body[0], ns, localDefs);
              let errVar = js_gensym("errMsg");
              let handler = toJsCode(&body[1], ns, localDefs);
              makeFnWrapper(&format!(
                "try {{\nreturn {}\n}} catch ({}) {{\nreturn ({})({}.toString())\n}}",
                code, errVar, handler, errVar
              ))
            }
            "echo" | "println" => {
              // not core syntax, but treat as macro for better debugging experience
              let args = ys.clone().slice(1..);
              let argsCode = genArgsCode(&args, ns, localDefs);
              format!("console.log({}printable({}))", varPrefix, argsCode)
            }
            "exists?" => {
              // not core syntax, but treat as macro for availability
              if body.len() != 1 {
                panic!(format!("expected 1 argument, {}", xs))
              }
              let item = body[0].clone();
              match &item {
                Calcit::Symbol(_sym, ..) => {
                  let target = toJsCode(&item, ns, localDefs);
                  return format!("(typeof {} !== 'undefined')", target);
                }
                _ => panic!(format!("expected a symbol, got: {}", xs)),
              }
            }
            "new" => {
              if ys.len() < 2 {
                panic!("`new` takes at least an object constructor {:?}", xs)
              }
              let ctor = ys[1].clone();
              let args = ys.clone().slice(1..);
              let argsCode = genArgsCode(&args, ns, localDefs);
              format!("new {}({})", toJsCode(&ctor, ns, localDefs), argsCode)
            }
            "instance?" => {
              if ys.len() != 3 {
                panic!(format!("`instance?` takes a constructor and a value, {}", xs));
              }
              let ctor = ys[1].clone();
              let v = ys[2].clone();

              format!(
                "({} instanceof {})",
                toJsCode(&v, ns, localDefs),
                toJsCode(&ctor, ns, localDefs)
              )
            }
            "set!" => {
              if ys.len() != 3 {
                panic!(format!("set! takes a operand and a value, {}", xs));
              }
              format!(
                "{} = {}",
                toJsCode(&ys[1], ns, localDefs),
                toJsCode(&ys[2], ns, localDefs)
              )
            }
            _ => {
              // TODO
              let token = s;
              if token.len() > 2 && &token[0..1] == ".-" && matchesJsVar(&token[2..]) {
                let name = token[2..].to_string();
                if ys.len() != 2 {
                  panic!(format!("property accessor takes only 1 argument, {:?}", xs));
                }
                let obj = ys[1].clone();
                format!("{}.{}", toJsCode(&obj, ns, localDefs), name)
              } else if token.len() > 1 && token.chars().next().unwrap() == '.' && matchesJsVar(&token[1..]) {
                let name: String = token[1..].to_string();
                if ys.len() < 2 {
                  panic!(format!("property accessor takes at least 1 argument, {:?}", xs));
                }
                let obj = ys[1].clone();
                let args = ys.clone().slice(2..);
                let argsCode = genArgsCode(&args, ns, localDefs);
                format!("{}.{}({})", toJsCode(&obj, ns, localDefs), name, argsCode)
              } else {
                let argsCode = genArgsCode(&body, ns, &localDefs);
                format!("{}({})", toJsCode(&head, ns, localDefs), argsCode)
              }
            }
          }
        }
        _ => {
          let argsCode = genArgsCode(&body, ns, &localDefs);
          format!("{}({})", toJsCode(&head, ns, localDefs), argsCode)
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
  localDefs: &HashSet<String>,
) -> String {
  let varPrefix = if ns == "calcit.core" { "" } else { "$calcit." };
  if has_ns_part(s) {
    let nsPart = s.split("/").collect::<Vec<&str>>()[0]; // TODO
    if nsPart == "js" {
      return escape_ns_var(s, "js");
    } else {
      // TODO ditry code
      match &resolved {
        Some(ResolvedDef(r_ns, _r_def)) => {
          let collected_imports = &mut global_collected_imports.lock().unwrap();
          if collected_imports.contains_key(r_ns) {
            let prev = collected_imports[r_ns].clone();
            if (!prev.just_ns) || &prev.ns != r_ns {
              println!("conflicted imports: {:?} {:?}", prev, resolved);
              panic!(format!("Conflicted implicit ns import, {:?}", xs));
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
          return escape_ns_var(s, r_ns);
        }
        Some(ResolvedLocal) => panic!("TODO"),
        None => panic!(format!("Expected symbol with ns being resolved: {:?}", xs)),
      }
    }
  } else if is_builtIn_js_proc(s) {
    return format!("{}{}", varPrefix, escape_var(s));
  } else if is_local(resolved) && localDefs.contains(s) {
    return escape_var(s);
  } else if let Some(ResolvedDef(r_ns, _r_def)) = resolved.clone() {
    if r_ns == primes::CORE_NS {
      // functions under core uses built $calcit module entry
      return format!("{}{}", varPrefix, escape_var(s));
    }
    // TODO ditry code
    let collected_imports = &mut global_collected_imports.lock().unwrap();
    if collected_imports.contains_key(s) {
      let prev = collected_imports[s].clone();
      if prev.ns != r_ns {
        println!("{:?} {:?}", collected_imports, xs);
        panic!(format!("Conflicted implicit imports, {:?}", xs));
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
    return escape_var(s);
  } else if def_ns == primes::CORE_NS {
    // local variales inside calcit.core also uses this ns
    println!("[Warn] detected variable inside core not resolved");
    format!("{}{}", varPrefix, escape_var(s))
  } else if def_ns == "" {
    panic!(format!("Unpexpected ns at symbol, {:?}", xs));
  } else if def_ns != ns {
    let collected_imports = &mut global_collected_imports.lock().unwrap(); // TODO
                                                                           // probably via macro
                                                                           // TODO ditry code collecting imports
    if collected_imports.contains_key(s) {
      let prev = collected_imports[s].clone();
      if prev.ns != def_ns {
        println!("{:?} {:?}", collected_imports, xs);
        panic!(format!("Conflicted implicit imports, probably via macro, {:?}", xs));
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
    return escape_var(s);
  } else if def_ns == ns {
    println!("[Warn] detected unresolved variable {:?} in {}", xs, ns);
    return escape_var(s);
  } else {
    println!("[Warn] Unpexpected casecode gen for {:?} in {}", xs, ns);
    format!("{}{}", varPrefix, escape_var(s))
  }
}

fn is_local(x: &Option<SymbolResolved>) -> bool {
  match x {
    Some(ResolvedLocal) => true,
    _ => false,
  }
}

fn gen_let_code(body: &CalcitItems, localDefs: &HashSet<String>, xs: &Calcit, ns: &str) -> String {
  let mut letDefBody = body.clone();

  // defined new local variable
  let mut scopedDefs = localDefs.clone();
  let mut defsCode = String::from("");
  let mut variableExisted = false;
  let mut bodyPart = String::from("");

  // break unless nested &let is found
  loop {
    if letDefBody.len() <= 1 {
      panic!(format!("Unpexpected empty content in let, {:?}", xs));
    }
    let pair = letDefBody[0].clone();
    let content = letDefBody.slice(1..);

    match &pair {
      Calcit::List(xs) if xs.len() == 2 => {
        let defName = xs[0].clone();
        let exprCode = xs[1].clone();

        match defName {
          Calcit::Symbol(sym, ..) => {
            // TODO `let` inside expressions makes syntax error
            let left = escape_var(&sym);
            let right = toJsCode(&exprCode, &ns, &scopedDefs);

            defsCode.push_str(&format!("let {} = {};\n", left, right));

            if scopedDefs.contains(&sym) {
              variableExisted = true;
            } else {
              scopedDefs.insert(sym.clone());
            }

            if variableExisted {
              for (idx, x) in content.clone().slice(1..).iter().enumerate() {
                if idx == content.len() - 1 {
                  bodyPart.push_str("return ");
                  bodyPart.push_str(&toJsCode(x, ns, &scopedDefs));
                  bodyPart.push_str(";\n");
                } else {
                  bodyPart.push_str(&toJsCode(x, ns, &scopedDefs));
                  bodyPart.push_str(";\n");
                }
              }

              // first variable is using conflicted name
              if localDefs.contains(&sym) {
                return make_let_with_bind(&left, &right, &bodyPart);
              } else {
                return make_let_with_wrapper(&left, &right, &bodyPart);
              }
            } else {
              if content.len() == 1 {
                let child = content[0].clone();
                match child {
                  Calcit::List(ys) if ys.len() == 2 => match (&ys[0], &ys[1]) {
                    (Calcit::Symbol(sym, ..), Calcit::List(zs)) if sym == "&let" && zs.len() == 2 => {
                      letDefBody = ys.clone().slice(1..);
                    }
                    _ => (),
                  },
                  _ => (),
                }
              }

              for (idx, x) in content.iter().enumerate() {
                if idx == content.len() - 1 {
                  bodyPart.push_str("return ");
                  bodyPart.push_str(&toJsCode(x, ns, &scopedDefs));
                  bodyPart.push_str(";\n");
                } else {
                  bodyPart.push_str(&toJsCode(x, ns, &scopedDefs));
                  bodyPart.push_str(";\n");
                }
              }

              break;
            }
          }
          _ => panic!(format!("Expected symbol behind let, got: {}", &pair)),
        }
      }
      Calcit::List(xs) => panic!(format!("expected pair of length 2, got: {}", &pair)),
      _ => panic!(format!("expected pair of a list of length 2, got: {}", pair)),
    }
  }
  return makeFnWrapper(&format!("{}{}", defsCode, bodyPart));
}

fn genArgsCode(body: &CalcitItems, ns: &str, localDefs: &HashSet<String>) -> String {
  let mut result = String::from("");
  let varPrefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let mut spreading = false;
  for x in body {
    match x {
      Calcit::Symbol(s, ..) if s == "&" => {
        spreading = true;
      }
      _ => {
        if result != "" {
          result.push_str(", ");
        }
        if spreading {
          result.push_str(&format!("...{}listToArray({})", varPrefix, toJsCode(x, ns, localDefs)));
          spreading = false
        } else {
          result.push_str(&toJsCode(&x, ns, &localDefs));
        }
      }
    }
  }
  result
}

fn listToJsCode(xs: &CalcitItems, ns: &str, localDefs: HashSet<String>, returnLabel: &str) -> String {
  // TODO default returnLabel="return "
  let mut result = String::from("");
  for (idx, x) in xs.iter().enumerate() {
    // result = result & "// " & $x & "\n"
    if idx == xs.len() - 1 {
      result.push_str(returnLabel);
      result.push_str(&toJsCode(&x, ns, &localDefs));
      result.push_str(";\n");
    } else {
      result.push_str(&toJsCode(x, ns, &localDefs));
      result.push_str(";\n");
    }
  }
  result
}

fn usesRecur(xs: &Calcit) -> bool {
  match xs {
    Calcit::Symbol(s, ..) => s == "recur",
    Calcit::List(ys) => {
      for y in ys {
        if usesRecur(y) {
          return true;
        }
      }
      false
    }
    _ => false,
  }
}

fn genJsFunc(
  name: &str,
  args: &CalcitItems,
  body: &CalcitItems,
  ns: &str,
  exported: bool,
  outerDefs: &HashSet<String>,
) -> String {
  let varPrefix = if ns == "calcit.core" { "" } else { "$calcit." };
  let mut localDefs = outerDefs.clone();
  let mut spreadingCode = String::from(""); // js list and calcit-js list are different, need to convert
  let mut argsCode = String::from("");
  let mut spreading = false;
  let mut hasOptional = false;
  let mut argsCount = 0;
  let mut optionalCount = 0;
  for x in args {
    match x {
      Calcit::Symbol(sym, ..) => {
        if spreading {
          if argsCode != "" {
            argsCode.push_str(", ");
          }
          localDefs.insert(sym.clone());
          let argName = escape_var(&sym);
          argsCode.push_str("...");
          argsCode.push_str(&argName);
          // js list and calcit-js are different in spreading
          spreadingCode.push_str(&format!("\n{} = {}arrayToList({});", argName, varPrefix, argName));
          break; // no more args after spreading argument
        } else if hasOptional {
          if argsCode != "" {
            argsCode.push_str(", ");
          }
          localDefs.insert(sym.clone());
          argsCode.push_str(&escape_var(&sym));
          optionalCount += 1;
        } else {
          if sym == "&" {
            spreading = true;
            continue;
          }
          if sym == "?" {
            hasOptional = true;
            continue;
          }
          if argsCode != "" {
            argsCode.push_str(", ");
          }
          localDefs.insert(sym.clone());
          argsCode.push_str(&escape_var(&sym));
          argsCount += 1;
        }
      }
      _ => panic!(format!("Expected symbol for arg, {}", x)),
    }
  }

  let checkArgs = if spreading {
    format!(
      "\nif (arguments.length < {}) {{ throw new Error('Too few arguments') }}",
      argsCount
    )
  } else if hasOptional {
    format!("\nif (arguments.length < {}) {{ throw new Error('Too few arguments') }}\nif (arguments.length > {}) {{ throw new Error('Too many arguments') }}", argsCount, argsCount + optionalCount )
  } else {
    format!(
      "\nif (arguments.length !== {}) {{ throw new Error('Args length mismatch') }}",
      argsCount
    )
  };

  if body.len() > 0 && usesRecur(&body[body.len() - 1]) {
    // ugliy code for inlining tail recursion template
    let retVar = js_gensym("ret");
    let timesVar = js_gensym("times");
    let mut fnDefinition = format!("function {}({})", escape_var(name), argsCode);
    fnDefinition.push_str(&format!("{{ {} {}", checkArgs, spreadingCode));
    fnDefinition.push_str(&format!("\nlet {} = null;\n", retVar));
    fnDefinition.push_str(&format!("let {} = 0;\n", timesVar));
    fnDefinition.push_str(&format!("while(true) {{ /* Tail Recursion */\n"));
    fnDefinition.push_str(&format!(
      "if ({} > 10000) {{ throw new Error('Expected tail recursion to exist quickly') }}\n",
      timesVar
    ));
    fnDefinition.push_str(&listToJsCode(&body, ns, localDefs, &format!("{} =", retVar)));
    fnDefinition.push_str(&format!("if ({} instanceof {}CrDataRecur) {{\n", retVar, varPrefix));
    fnDefinition.push_str(&checkArgs.replace("arguments.length", &format!("{}.args.length", retVar)));
    fnDefinition.push_str(&format!("\n[ {} ] = {}.args;\n", argsCode, retVar));
    fnDefinition.push_str(&spreadingCode);
    fnDefinition.push_str(&format!("{} += 1;\ncontinue;\n", timesVar));
    fnDefinition.push_str(&format!("}} else {{ return {} }}  ", retVar));
    fnDefinition.push_str("}\n}");

    let exportMark = if exported {
      format!("export let = {}", escape_var(name))
    } else {
      String::from("")
    };
    return format!("{}{}{}", exportMark, fnDefinition, C_LINE);
  } else {
    let fnDefinition = format!(
      "function {}({}) {{ {}{}\n{} }}",
      escape_var(name),
      argsCode,
      checkArgs,
      spreadingCode,
      listToJsCode(&body, ns, localDefs, "return ")
    );
    let exportMark = if exported { "export " } else { "" };
    return format!("{}{}\n", exportMark, fnDefinition);
  }
}

fn containsSymbol(xs: &Calcit, y: &str) -> bool {
  match xs {
    Calcit::Symbol(s, ..) => s == y,
    Calcit::Thunk(code) => containsSymbol(code, y),
    Calcit::Fn(_, _, _, _, _, body) => {
      for x in body {
        if containsSymbol(x, y) {
          return true;
        }
      }
      false
    }
    Calcit::List(zs) => {
      for z in zs {
        if containsSymbol(z, y) {
          return true;
        }
      }
      false
    }
    _ => false,
  }
}

fn sortByDeps(deps: &HashMap<String, Calcit>) -> Vec<String> {
  let mut result: Vec<String> = vec![];

  let mut depsGraph: HashMap<String, HashSet<String>> = HashMap::new();
  let mut defNames: Vec<String> = vec![];
  for (k, v) in deps {
    defNames.push(k.clone());
    let mut depsInfo: HashSet<String> = HashSet::new();
    for (k2, _v2) in deps {
      if k2 == k {
        continue;
      }
      // echo "checking ", k, " -> ", k2, " .. ", v.containsSymbol(k2)
      if containsSymbol(&v, &k2) {
        depsInfo.insert(k2.clone());
      }
    }
    depsGraph.insert(k.to_string(), depsInfo);
  }
  // echo depsGraph
  defNames.sort();
  for x in defNames {
    let mut inserted = false;
    for (idx, y) in result.iter().enumerate() {
      if depsGraph.contains_key(y) && depsGraph[y].contains(&x) {
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

fn writeFileIfChanged(filename: &str, content: &str) -> bool {
  if Path::new(filename).exists() && fs::read_to_string(filename).unwrap() == content {
    return false;
  }
  let _ = fs::write(filename, content);
  return true;
}

fn emitJs(programData: &HashMap<String, program::ProgramFileData>, entryNs: &str) {
  let codeEmitPath = "js-out/"; // TODO
  if !Path::new(codeEmitPath).exists() {
    let _ = fs::create_dir(codeEmitPath);
  }

  let mut unchangedNs: HashSet<String> = HashSet::new();

  let collected_imports = &mut global_collected_imports.lock().unwrap();
  let previous_program_caches = &mut global_previous_program_caches.lock().unwrap();

  for (ns, file) in programData {
    // side-effects, reset tracking state
    collected_imports.clear(); // reset

    let mut defsInCurrent: HashSet<String> = HashSet::new();
    for (k, _) in &file.defs {
      defsInCurrent.insert(k.clone());
    }

    if !first_compilation.load(Ordering::Relaxed) {
      let appPkgName = entryNs.split('.').collect::<Vec<&str>>()[0];
      let pkgName = ns.split('.').collect::<Vec<&str>>()[0]; // TODO simpler
      if appPkgName != pkgName
        && previous_program_caches.contains_key(ns)
        && (previous_program_caches[ns] == defsInCurrent)
      {
        continue; // since libraries do not have to be re-compiled
      }
    }
    // remember defs of each ns for comparing
    previous_program_caches.insert(ns.to_string(), defsInCurrent);

    // reset index each file
    reset_js_gensym_index();

    // let coreLib = "http://js.calcit-lang.org/calcit.core.js".escape()
    let coreLib = to_js_import_name("calcit.core", false); // TODO js_mode
    let procsLib = format!("\"{}\"", "@calcit/procs".escape_debug());
    let mut importCode = String::from("");

    let mut defsCode = String::from(""); // code generated by functions
    let mut valsCode = String::from(""); // code generated by thunks

    if ns == "calcit.core" {
      importCode.push_str(&format!(
        "\nimport {{kwd, arrayToList, listToArray, CrDataRecur}} from {};\n",
        procsLib
      ));
      importCode.push_str(&format!("\"import * as $calcit_procs from {};\"", procsLib));
      importCode.push_str(&format!("\"export * from {};\"", procsLib));
    } else {
      importCode.push_str(&format!("\nimport * as $calcit from {};\n", coreLib));
    }

    let mut defNames: HashSet<String> = HashSet::new(); // multiple parts of scoped defs need to be tracked

    // tracking top level scope definitions
    for (def, _) in &file.defs {
      defNames.insert(def.clone());
    }

    let depsInOrder = sortByDeps(&file.defs);
    // echo "deps order: ", depsInOrder

    for def in depsInOrder {
      if ns == primes::CORE_NS {
        // some defs from core can be replaced by calcit.procs
        if is_jsUnavailableProcs(&def) {
          continue;
        }
        if is_preferredJsProc(&def) {
          defsCode.push_str(&format!(
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
          defsCode.push_str(&format!(
            "\"var {} = $calcit_procs.{};\"",
            escape_var(&def),
            escape_var(&def)
          ));
        }
        Calcit::Fn(name, def_ns, _, _, args, code) => {
          defsCode.push_str(&genJsFunc(&def, args, code, def_ns, true, &defNames));
        }
        Calcit::Thunk(code) => {
          // TODO need topological sorting for accuracy
          // values are called directly, put them after fns
          valsCode.push_str(&format!(
            "\"export var {} = {};\"",
            escape_var(&def),
            toJsCode(code, &ns, &defNames)
          ));
        }
        Calcit::Macro(..) => {
          // macro should be handled during compilation, psuedo code
          defsCode.push_str(&format!("\"export var {} = () => {{/* Macro */}}\"", escape_var(&def)));
          defsCode.push_str(&format!("\"{}.isMacro = true;\"", escape_var(&def)));
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
            let importTarget = if item.ns_in_str {
              format!("\"{}\"", item.ns.escape_debug())
            } else {
              to_js_import_name(&item.ns, false) // TODO js_mode
            };
            importCode.push_str(&format!(
              "\"import * as {} from {};\"",
              escape_ns(&item.ns),
              importTarget
            ));
          } else {
            let importTarget = to_js_import_name(&item.ns, false); // TODO js_mode
            importCode.push_str(&format!("\"import {{ {} }} from {};\"", escape_var(def), importTarget));
          }
        }
      }

      let jsFilePath = format!("{}{}", codeEmitPath, to_js_file_name(&ns, false)); // TODO mjs_mode
      let wroteNew = writeFileIfChanged(&jsFilePath, &format!("{}\n{}\n{}", importCode, defsCode, valsCode));
      if wroteNew {
        println!("Emitted js file: {}", jsFilePath);
      } else {
        unchangedNs.insert(ns.to_string());
      }
    }
  }

  if unchangedNs.len() > 0 {
    println!("\n... and {} files not changed.", unchangedNs.len());
  }

  first_compilation.store(false, Ordering::SeqCst); // TODO
}

fn is_jsUnavailableProcs(name: &str) -> bool {
  match name {
    "&reset-gensym-index!"
    | "dbt->point"
    | "dbt-digits"
    | "dbt-balanced-ternary"
    | "gensym"
    | "macroexpand"
    | "macroexpand-all"
    | "to-cirru-edn"
    | "extract-cirru-edn" => true,
    _ => false,
  }
}

use crate::builtins;
use crate::builtins::records::find_in_fields;
use crate::call_stack;
use crate::data::cirru;
use crate::data::edn;
use crate::primes;
use crate::primes::{Calcit, CalcitItems, CrListWrap};
use crate::program;
use crate::runner;
use crate::util::number::f64_to_usize;

use cirru_parser::Cirru;

use std::cmp::Ordering;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;

static SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);
static JS_SYMBOL_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn type_of(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => match a {
      Calcit::Nil => Ok(Calcit::Keyword(String::from("nil"))),
      // CalcitRef(Calcit), // TODO
      Calcit::Bool(..) => Ok(Calcit::Keyword(String::from("bool"))),
      Calcit::Number(..) => Ok(Calcit::Keyword(String::from("number"))),
      Calcit::Symbol(..) => Ok(Calcit::Keyword(String::from("symbol"))),
      Calcit::Keyword(..) => Ok(Calcit::Keyword(String::from("keyword"))),
      Calcit::Str(..) => Ok(Calcit::Keyword(String::from("string"))),
      Calcit::Thunk(..) => Ok(Calcit::Keyword(String::from("thunk"))), // internal
      Calcit::Ref(..) => Ok(Calcit::Keyword(String::from("ref"))),
      Calcit::Tuple(..) => Ok(Calcit::Keyword(String::from("tuple"))),
      Calcit::Recur(..) => Ok(Calcit::Keyword(String::from("recur"))),
      Calcit::List(..) => Ok(Calcit::Keyword(String::from("list"))),
      Calcit::Set(..) => Ok(Calcit::Keyword(String::from("set"))),
      Calcit::Map(..) => Ok(Calcit::Keyword(String::from("map"))),
      Calcit::Record(..) => Ok(Calcit::Keyword(String::from("record"))),
      Calcit::Proc(..) => Ok(Calcit::Keyword(String::from("fn"))), // special kind proc, but also fn
      Calcit::Macro(..) => Ok(Calcit::Keyword(String::from("macro"))),
      Calcit::Fn(..) => Ok(Calcit::Keyword(String::from("fn"))),
      Calcit::Syntax(..) => Ok(Calcit::Keyword(String::from("synta"))),
    },
    None => Err(String::from("type-of expected 1 argument")),
  }
}

pub fn recur(xs: &CalcitItems) -> Result<Calcit, String> {
  Ok(Calcit::Recur(xs.to_owned()))
}

pub fn format_to_lisp(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(v) => Ok(Calcit::Str(v.lisp_str())),
    None => Err(String::from("format-to-lisp expected 1 argument")),
  }
}

pub fn gensym(xs: &CalcitItems) -> Result<Calcit, String> {
  let idx = SYMBOL_INDEX.fetch_add(1, atomic::Ordering::SeqCst);
  let n = idx + 1; // use 1 as first value since previous implementation did this

  let s = match xs.get(0) {
    Some(Calcit::Str(s)) | Some(Calcit::Keyword(s)) | Some(Calcit::Symbol(s, ..)) => {
      let mut chunk = s.to_owned();
      chunk.push('_');
      chunk.push('_');
      chunk.push_str(&n.to_string());
      chunk
    }
    Some(a) => return Err(format!("gensym expected a string, but got: {}", a)),
    None => {
      let mut chunk = String::from("G__");
      chunk.push_str(&n.to_string());
      chunk
    }
  };
  Ok(Calcit::Symbol(
    s,
    String::from(primes::GENERATED_NS),
    String::from(primes::GENERATED_DEF),
    None,
  ))
}

pub fn reset_gensym_index(_xs: &CalcitItems) -> Result<Calcit, String> {
  let _ = SYMBOL_INDEX.swap(0, atomic::Ordering::SeqCst);
  Ok(Calcit::Nil)
}

pub fn force_reset_gensym_index() -> Result<(), String> {
  let _ = SYMBOL_INDEX.swap(0, atomic::Ordering::SeqCst);
  Ok(())
}

pub fn reset_js_gensym_index() {
  let _ = JS_SYMBOL_INDEX.swap(0, atomic::Ordering::SeqCst);
}

// for emitting js
pub fn js_gensym(name: &str) -> String {
  let idx = JS_SYMBOL_INDEX.fetch_add(1, atomic::Ordering::SeqCst);
  let n = idx + 1; // use 1 as first value since previous implementation did this

  let mut chunk = String::from(name);
  chunk.push_str("_AUTO_");
  chunk.push_str(&n.to_string());
  chunk
}

pub fn generate_id(xs: &CalcitItems) -> Result<Calcit, String> {
  let size = match xs.get(0) {
    Some(Calcit::Number(n)) => match f64_to_usize(*n) {
      Ok(size) => Some(size),
      Err(e) => return Err(e),
    },
    Some(a) => return Err(format!("expected usize, got: {}", a)),
    None => None, // nanoid defaults to 21
  };

  match (size, xs.get(1)) {
    (None, None) => Ok(Calcit::Str(nanoid!())),
    (Some(n), None) => Ok(Calcit::Str(nanoid!(n))),
    (Some(n), Some(Calcit::Str(s))) => {
      let mut charset: Vec<char> = vec![];
      for c in s.chars() {
        charset.push(c);
      }
      Ok(Calcit::Str(nanoid!(n, &charset)))
    }
    (a, b) => Err(format!("generate-id! expected size or charset, got: {:?} {:?}", a, b)),
  }
}

pub fn display_stack(_xs: &CalcitItems) -> Result<Calcit, String> {
  call_stack::show_stack();
  Ok(Calcit::Nil)
}

pub fn parse_cirru(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match cirru_parser::parse(s) {
      Ok(nodes) => Ok(cirru::cirru_to_calcit(&Cirru::List(nodes))),
      Err(e) => Err(format!("parse-cirru failed, {}", e)),
    },
    Some(a) => Err(format!("parse-cirru expected a string, got: {}", a)),
    None => Err(String::from("parse-cirru expected 1 argument")),
  }
}

pub fn format_cirru(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => {
      let options = cirru_parser::CirruWriterOptions { use_inline: false };
      match cirru::calcit_data_to_cirru(a) {
        Ok(v) => {
          if let Cirru::List(ys) = v {
            Ok(Calcit::Str(cirru_parser::format(&ys, options)?))
          } else {
            Err(format!("expected vector for Cirru formatting: {}", v))
          }
        }
        Err(e) => Err(format!("format-cirru failed, {}", e)),
      }
    }
    None => Err(String::from("parse-cirru expected 1 argument")),
  }
}

pub fn parse_cirru_edn(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match cirru_edn::parse(s) {
      Ok(nodes) => Ok(edn::edn_to_calcit(&nodes)),
      Err(e) => Err(format!("parse-cirru-edn failed, {}", e)),
    },
    Some(a) => Err(format!("parse-cirru-edn expected a string, got: {}", a)),
    None => Err(String::from("parse-cirru-edn expected 1 argument")),
  }
}

pub fn format_cirru_edn(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(a) => Ok(Calcit::Str(cirru_edn::format(&edn::calcit_to_edn(a)?, true)?)),
    None => Err(String::from("format-cirru-edn expected 1 argument")),
  }
}

pub fn turn_symbol(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Symbol(
      s.to_owned(),
      String::from(primes::GENERATED_NS),
      String::from(primes::GENERATED_DEF),
      None,
    )),
    Some(Calcit::Keyword(s)) => Ok(Calcit::Symbol(
      s.to_owned(),
      String::from(primes::GENERATED_NS),
      String::from(primes::GENERATED_DEF),
      None,
    )),
    Some(Calcit::Symbol(s, ns, def, resolved)) => Ok(Calcit::Symbol(
      s.to_owned(),
      ns.to_owned(),
      def.to_owned(),
      resolved.to_owned(),
    )),
    Some(a) => Err(format!("turn-symbol cannot turn this to symbol: {}", a)),
    None => Err(String::from("turn-symbol expected 1 argument, got nothing")),
  }
}

pub fn turn_keyword(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => Ok(Calcit::Keyword(s.to_owned())),
    Some(Calcit::Keyword(s)) => Ok(Calcit::Keyword(s.to_owned())),
    Some(Calcit::Symbol(s, ..)) => Ok(Calcit::Keyword(s.to_owned())),
    Some(a) => Err(format!("turn-keyword cannot turn this to keyword: {}", a)),
    None => Err(String::from("turn-keyword expected 1 argument, got nothing")),
  }
}

pub fn new_tuple(xs: &CalcitItems) -> Result<Calcit, String> {
  if xs.len() != 2 {
    Err(format!("tuple expected 2 arguments, got {}", CrListWrap(xs.to_owned())))
  } else {
    Ok(Calcit::Tuple(Box::new(xs[0].to_owned()), Box::new(xs[1].to_owned())))
  }
}

pub fn invoke_method(
  name: &str,
  invoke_args: &CalcitItems,
  program_code: &program::ProgramCodeData,
) -> Result<Calcit, String> {
  let (class, value) = match invoke_args.get(0) {
    Some(Calcit::Tuple(a, _b)) => ((**a).to_owned(), invoke_args.get(0).unwrap().to_owned()),
    Some(Calcit::Number(..)) => {
      // classed should already be preprocessed
      let code = gen_sym("&core-number-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::Str(..)) => {
      let code = gen_sym("&core-string-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::Set(..)) => {
      let code = gen_sym("&core-set-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::List(..)) => {
      let code = gen_sym("&core-list-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::Map(..)) => {
      let code = gen_sym("&core-map-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::Record(..)) => {
      let code = gen_sym("&core-record-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::Nil) => {
      let code = gen_sym("&core-nil-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    Some(Calcit::Fn(..)) | Some(Calcit::Proc(..)) => {
      let code = gen_sym("&core-fn-class");
      let class = runner::evaluate_expr(&code, &im::HashMap::new(), primes::CORE_NS, program_code)?;
      (class, invoke_args[0].to_owned())
    }
    x => return Err(format!("cannot decide a class from: {:?}", x)),
  };
  match &class {
    Calcit::Record(_, fields, values) => {
      match find_in_fields(fields, name) {
        Some(idx) => {
          let mut method_args: im::Vector<Calcit> = im::vector![];
          method_args.push_back(value);
          let mut at_first = true;
          for x in invoke_args {
            if at_first {
              at_first = false
            } else {
              method_args.push_back(x.to_owned())
            }
          }

          match &values[idx] {
            // dirty copy...
            Calcit::Fn(_, def_ns, _, def_scope, args, body) => {
              runner::run_fn(&method_args, def_scope, args, body, def_ns, program_code)
            }
            Calcit::Proc(proc) => builtins::handle_proc(proc, &method_args),
            Calcit::Syntax(syn, _ns) => Err(format!(
              "cannot get syntax here since instance is always evaluated, got: {}",
              syn
            )),
            y => Err(format!("expected a function to invoke, got: {}", y)),
          }
        }
        None => Err(format!("missing field `{}` in {}", name, fields.join(","))),
      }
    }
    x => Err(format!("method invoking expected a record as class, got: {}", x)),
  }
}

fn gen_sym(sym: &str) -> Calcit {
  Calcit::Symbol(
    String::from("&core-map-class"),
    String::from(primes::CORE_NS),
    String::from(primes::GENERATED_DEF),
    Some(primes::SymbolResolved::ResolvedDef(
      String::from(primes::CORE_NS),
      String::from(sym),
      None,
    )),
  )
}

pub fn native_compare(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(a), Some(b)) => match a.cmp(b) {
      Ordering::Less => Ok(Calcit::Number(-1.0)),
      Ordering::Greater => Ok(Calcit::Number(1.0)),
      Ordering::Equal => Ok(Calcit::Number(0.0)),
    },
    (a, b) => Err(format!("&compare expected 2 values, got {:?} {:?}", a, b)),
  }
}

pub fn tuple_nth(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1)) {
    (Some(Calcit::Tuple(a, b)), Some(Calcit::Number(n))) => match f64_to_usize(*n) {
      Ok(0) => Ok((**a).to_owned()),
      Ok(1) => Ok((**b).to_owned()),
      Ok(m) => Err(format!("Tuple only got 2 elements, trying to index with {}", m)),
      Err(e) => Err(format!("&tuple:nth expect usize, {}", e)),
    },
    (Some(_), None) => Err(format!("&tuple:nth expected a tuple and an index, got: {:?}", xs)),
    (None, Some(_)) => Err(format!("&tuple:nth expected a tuple and an index, got: {:?}", xs)),
    (_, _) => Err(format!(
      "&tuple:nth expected 2 argument, got: {}",
      CrListWrap(xs.to_owned())
    )),
  }
}

pub fn assoc(xs: &CalcitItems) -> Result<Calcit, String> {
  match (xs.get(0), xs.get(1), xs.get(2)) {
    (Some(Calcit::Tuple(a0, a1)), Some(Calcit::Number(n)), Some(a)) => match f64_to_usize(*n) {
      Ok(idx) => {
        if idx == 0 {
          Ok(Calcit::Tuple(Box::new(a.to_owned()), a1.to_owned()))
        } else if idx == 1 {
          Ok(Calcit::Tuple(a0.to_owned(), Box::new(a.to_owned())))
        } else {
          Err(format!("Tuple only has fields of 0,1 , unknown index: {}", idx))
        }
      }
      Err(e) => Err(e),
    },
    (Some(a), ..) => Err(format!("tuplu:assoc expected a tuple, got: {}", a)),
    (None, ..) => Err(format!("tuplu:assoc expected 3 arguments, got: {:?}", xs)),
  }
}

pub fn no_op() -> Result<Calcit, String> {
  Ok(Calcit::Nil)
}

pub fn get_os(_xs: &CalcitItems) -> Result<Calcit, String> {
  // https://doc.rust-lang.org/std/env/consts/constant.OS.html
  Ok(Calcit::Keyword(std::env::consts::OS.to_owned()))
}

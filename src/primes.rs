// use cirru_parser::CirruNode; // TODO for Calcit::Thunk
use core::cmp::Ord;
use regex::Regex;
use std::cmp::Eq;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::hash::{Hash, Hasher};

// String from nanoid!
pub type NanoId = String;

// scope
pub type CalcitScope = im::HashMap<String, Calcit>;
pub type CalcitItems = im::Vector<Calcit>;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolResolved {
  ResolvedLocal,
  ResolvedDef(String, String), // ns, def
}

/// special types wraps vector of calcit data for displaying
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct CrListWrap(pub im::Vector<Calcit>);

#[derive(Debug, Clone)]
pub enum Calcit {
  Nil,
  Bool(bool),
  Number(f64),
  Symbol(String, String, Option<SymbolResolved>), // content, ns... so it has meta information
  Keyword(String),
  Str(String),
  Thunk(Box<Calcit>),
  /// holding a path to its state
  Ref(String),
  Recur(CalcitItems), // not data, but for recursion
  List(CalcitItems),
  Set(im::HashSet<Calcit>),
  Map(im::HashMap<Calcit, Calcit>),
  Record(String, Vec<String>, Vec<Calcit>),
  Proc(String),
  Macro(
    String, // name
    String, // ns
    NanoId,
    CalcitItems, // args
    CalcitItems, // body
  ),
  Fn(
    String,
    String,
    NanoId,
    CalcitScope,
    CalcitItems, // args
    CalcitItems, // body
  ),
  Syntax(String, String), // name, ns... notice that `ns` is a meta info
}

impl fmt::Display for Calcit {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Calcit::Nil => f.write_str("nil"),
      Calcit::Bool(v) => f.write_str(&format!("{}", v)),
      Calcit::Number(n) => f.write_str(&format!("{}", n)),
      Calcit::Symbol(s, ..) => f.write_str(&format!("'{}", s)),
      Calcit::Keyword(s) => f.write_str(&format!(":{}", s)),
      Calcit::Str(s) => {
        lazy_static! {
          static ref RE_SIMPLE_TOKEN: Regex = Regex::new(r"^[\w\d\-\?!\|]+$").unwrap();
        }
        if RE_SIMPLE_TOKEN.is_match(s) {
          write!(f, "|{}", s)
        } else {
          write!(f, "\"|{}\"", str::escape_debug(s))
        }
      } // TODO, escaping choices
      Calcit::Thunk(v) => f.write_str(&format!("(&thunk {})", v)),
      Calcit::Ref(name) => f.write_str(&format!("(&ref {})", name)),
      Calcit::Recur(xs) => {
        f.write_str("(&recur")?;
        for x in xs {
          f.write_str(&format!(" {}", x))?;
        }
        f.write_str(")")
      }
      Calcit::List(xs) => {
        f.write_str("([]")?;
        for x in xs {
          f.write_str(&format!(" {}", x))?;
        }
        f.write_str(")")
      }
      Calcit::Set(xs) => {
        f.write_str("(#{}")?;
        for x in xs {
          f.write_str(&format!(" {}", x))?;
        }
        f.write_str(")")
      }
      Calcit::Map(xs) => {
        f.write_str("({}")?;
        for (k, v) in xs {
          f.write_str(&format!(" ({} {})", k, v))?;
        }
        f.write_str(")")?;
        Ok(())
      }
      Calcit::Record(name, fields, values) => {
        f.write_str(&format!("(%{{}} {}", name))?;
        for idx in 0..fields.len() {
          f.write_str(&format!("({} {})", fields[idx], values[idx]))?;
        }
        f.write_str(")")
      }
      Calcit::Proc(name) => f.write_str(&format!("(&proc {})", name)),
      Calcit::Macro(name, _def_ns, _, args, body) => {
        f.write_str(&format!("(&macro {} (", name))?;
        let mut need_space = false;
        for a in args {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(a))?;
          need_space = true;
        }
        f.write_str(") (")?;
        need_space = false;
        for b in body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str("))")
      }
      Calcit::Fn(name, _, _, _, args, body) => {
        f.write_str(&format!("(&fn {} (", name))?;
        let mut need_space = false;
        for a in args {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(a))?;
          need_space = true;
        }
        f.write_str(") (")?;
        need_space = false;
        for b in body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str("))")
      }
      Calcit::Syntax(name, _ns) => f.write_str(&format!("(&syntax {})", name)),
    }
  }
}
/// special types wraps vector of calcit data for displaying

impl fmt::Display for CrListWrap {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&format_to_lisp(&Calcit::List(self.0.clone()))) // TODO performance
  }
}

/// display data into Lisp style for readability
pub fn format_to_lisp(x: &Calcit) -> String {
  match x {
    Calcit::List(ys) => {
      let mut s = String::from("(");
      for (idx, y) in ys.iter().enumerate() {
        if idx > 0 {
          s.push(' ');
        }
        s.push_str(&format_to_lisp(y));
      }
      s.push(')');
      s
    }
    Calcit::Symbol(s, ..) => s.clone(),
    a => format!("{}", a),
  }
}

impl Hash for Calcit {
  fn hash<H>(&self, _state: &mut H)
  where
    H: Hasher,
  {
    match self {
      Calcit::Nil => "nil:".hash(_state),
      Calcit::Bool(v) => {
        "bool:".hash(_state);
        v.hash(_state);
      }
      Calcit::Number(n) => {
        "number:".hash(_state);
        // TODO https://stackoverflow.com/q/39638363/883571
        (*n as usize).hash(_state)
      }
      Calcit::Symbol(s, _ns, _resolved) => {
        "symbol:".hash(_state);
        s.hash(_state);
        // probaly no need, also won't be used in hashing
        // ns.hash(_state);
      }
      Calcit::Keyword(s) => {
        "keyword:".hash(_state);
        s.hash(_state);
      }
      Calcit::Str(s) => {
        "string:".hash(_state);
        s.hash(_state);
      }
      Calcit::Thunk(v) => {
        "quote:".hash(_state);
        v.hash(_state);
      }
      Calcit::Ref(name) => {
        "quote:".hash(_state);
        name.hash(_state);
      }
      Calcit::Recur(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      Calcit::List(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      Calcit::Set(v) => {
        "set:".hash(_state);
        // TODO order for set is stable
        for x in v {
          x.hash(_state)
        }
      }
      Calcit::Map(v) => {
        "map:".hash(_state);
        // TODO order for map is not stable
        for x in v {
          x.hash(_state)
        }
      }
      Calcit::Record(name, fields, values) => {
        "record:".hash(_state);
        name.hash(_state);
        fields.hash(_state);
        values.hash(_state);
      }
      Calcit::Proc(name) => {
        "proc:".hash(_state);
        name.hash(_state);
      }
      Calcit::Macro(name, gen_id, ..) => {
        "macro:".hash(_state);
        name.hash(_state);
        gen_id.hash(_state);
      }
      Calcit::Fn(name, gen_id, ..) => {
        "fn:".hash(_state);
        name.hash(_state);
        gen_id.hash(_state);
      }
      Calcit::Syntax(name, _ns) => {
        "syntax:".hash(_state);
        // syntax name can be used as identity
        name.hash(_state);
      }
    }
  }
}

impl Ord for Calcit {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (Calcit::Nil, Calcit::Nil) => Equal,
      (Calcit::Nil, _) => Less,
      (_, Calcit::Nil) => Greater,

      (Calcit::Bool(a), Calcit::Bool(b)) => a.cmp(b),
      (Calcit::Bool(_), _) => Less,
      (_, Calcit::Bool(_)) => Greater,

      (Calcit::Number(a), Calcit::Number(b)) => {
        if a < b {
          Less
        } else if a > b {
          Greater
        } else {
          Equal
        }
      }
      (Calcit::Number(_), _) => Less,
      (_, Calcit::Number(_)) => Greater,

      (Calcit::Symbol(a, ..), Calcit::Symbol(b, ..)) => a.cmp(&b),
      (Calcit::Symbol(..), _) => Less,
      (_, Calcit::Symbol(..)) => Greater,

      (Calcit::Keyword(a), Calcit::Keyword(b)) => a.cmp(&b),
      (Calcit::Keyword(_), _) => Less,
      (_, Calcit::Keyword(_)) => Greater,

      (Calcit::Str(a), Calcit::Str(b)) => a.cmp(&b),
      (Calcit::Str(_), _) => Less,
      (_, Calcit::Str(_)) => Greater,

      (Calcit::Thunk(a), Calcit::Thunk(b)) => a.cmp(b),
      (Calcit::Thunk(_), _) => Less,
      (_, Calcit::Thunk(_)) => Greater,

      (Calcit::Ref(a), Calcit::Ref(b)) => a.cmp(b),
      (Calcit::Ref(_), _) => Less,
      (_, Calcit::Ref(_)) => Greater,

      (Calcit::Recur(a), Calcit::Recur(b)) => a.cmp(b),
      (Calcit::Recur(_), _) => Less,
      (_, Calcit::Recur(_)) => Greater,

      (Calcit::List(a), Calcit::List(b)) => a.cmp(b),
      (Calcit::List(_), _) => Less,
      (_, Calcit::List(_)) => Greater,

      (Calcit::Set(a), Calcit::Set(b)) => match a.len().cmp(&b.len()) {
        Equal => {
          unreachable!("TODO sets are not cmp ed") // TODO
        }
        a => a,
      },
      (Calcit::Set(_), _) => Less,
      (_, Calcit::Set(_)) => Greater,

      (Calcit::Map(a), Calcit::Map(b)) => {
        unreachable!(format!("TODO maps are not cmp ed {:?} {:?}", a, b))
        // TODO
      }
      (Calcit::Map(_), _) => Less,
      (_, Calcit::Map(_)) => Greater,

      (Calcit::Record(_name1, _fields1, _values1), Calcit::Record(_name2, _fields2, _values2)) => {
        unreachable!("TODO records are not cmp ed") // TODO
      }
      (Calcit::Record(..), _) => Less,
      (_, Calcit::Record(..)) => Greater,

      (Calcit::Proc(a), Calcit::Proc(b)) => a.cmp(b),
      (Calcit::Proc(_), _) => Less,
      (_, Calcit::Proc(_)) => Greater,

      (Calcit::Macro(_, a, ..), Calcit::Macro(_, b, ..)) => a.cmp(b),
      (Calcit::Macro(..), _) => Less,
      (_, Calcit::Macro(..)) => Greater,

      (Calcit::Fn(_, a, ..), Calcit::Fn(_, b, ..)) => a.cmp(&b), // compared with nanoid
      (Calcit::Fn(..), _) => Less,
      (_, Calcit::Fn(..)) => Greater,

      (Calcit::Syntax(a, _), Calcit::Syntax(b, _)) => a.cmp(&b),
    }
  }
}

impl PartialOrd for Calcit {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Eq for Calcit {}

impl PartialEq for Calcit {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Calcit::Nil, Calcit::Nil) => true,
      (Calcit::Bool(a), Calcit::Bool(b)) => a == b,
      (Calcit::Number(a), Calcit::Number(b)) => a == b,
      (Calcit::Symbol(a, ..), Calcit::Symbol(b, ..)) => a == b,
      (Calcit::Keyword(a), Calcit::Keyword(b)) => a == b,
      (Calcit::Str(a), Calcit::Str(b)) => a == b,
      (Calcit::Thunk(a), Calcit::Thunk(b)) => a == b,
      (Calcit::Ref(a), Calcit::Ref(b)) => a == b,
      (Calcit::List(a), Calcit::List(b)) => a == b,
      (Calcit::Set(a), Calcit::Set(b)) => a == b,
      (Calcit::Map(a), Calcit::Map(b)) => a == b,
      (Calcit::Record(name1, fields1, values1), Calcit::Record(name2, fields2, values2)) => {
        name1 == name2 && fields1 == fields2 && values1 == values2
      }

      // functions compared with nanoid
      (Calcit::Proc(a), Calcit::Proc(b)) => a == b,
      (Calcit::Macro(_, a, ..), Calcit::Macro(_, b, ..)) => a == b,
      (Calcit::Fn(_, a, ..), Calcit::Fn(_, b, ..)) => a == b,
      (Calcit::Syntax(a, _), Calcit::Syntax(b, _)) => a == b,
      (_, _) => false,
    }
  }
}

pub const CORE_NS: &str = "calcit.core";
pub const GENERATED_NS: &str = "calcit.gen";

pub const CALCI_VERSION: &str = "0.0.1";

impl Calcit {
  pub fn turn_string(&self) -> String {
    match self {
      Calcit::Str(s) => s.clone(),
      _ => format!("{}", self),
    }
  }
}

mod syntax_name;

use core::cmp::Ord;
use std::cmp::Eq;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;

use cirru_edn::EdnKwd;
use im_ternary_tree::TernaryTreeList;

static ID_GEN: AtomicUsize = AtomicUsize::new(0);

// scope
pub type CalcitScope = rpds::HashTrieMapSync<Arc<str>, Calcit>;
pub type CalcitItems = TernaryTreeList<Calcit>;

pub use syntax_name::CalcitSyntax;

use crate::call_stack::CallStackList;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolResolved {
  ResolvedLocal,
  ResolvedRaw, // raw syntax, no target
  ResolvedDef {
    ns: Arc<str>,
    def: Arc<str>,
    rule: Option<Arc<ImportRule>>,
  }, // ns, def
}

/// defRule: ns def
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRule {
  NsAs(Arc<str>),                 // ns
  NsReferDef(Arc<str>, Arc<str>), // ns, def
  NsDefault(Arc<str>),            // ns, js only
}

/// special types wraps vector of calcit data for displaying
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct CrListWrap(pub TernaryTreeList<Calcit>);

pub struct CalcitList(pub Arc<TernaryTreeList<Calcit>>);

#[derive(Debug, Clone)]
pub enum Calcit {
  Nil,
  Bool(bool),
  Number(f64),
  Symbol {
    sym: Arc<str>,
    ns: Arc<str>,
    at_def: Arc<str>,
    resolved: Option<Arc<SymbolResolved>>,
  }, // content, ns... so it has meta information
  Keyword(EdnKwd),
  Str(Arc<str>),
  Thunk(Arc<Calcit>, Option<Arc<Calcit>>),
  /// holding a path to its state
  Ref(Arc<str>),
  /// more tagged union type, more like an internal structure
  Tuple(Arc<Calcit>, Arc<Calcit>),
  ///  to be used by FFIs
  Buffer(Vec<u8>),
  /// not for data, but for recursion
  Recur(CalcitItems),
  List(CalcitItems),
  Set(rpds::HashTrieSetSync<Calcit>),
  Map(rpds::HashTrieMapSync<Calcit, Calcit>),
  Record(EdnKwd, Arc<Vec<EdnKwd>>, Arc<Vec<Calcit>>), // usize of keyword id
  Proc(Arc<str>),
  Macro {
    name: Arc<str>,         // name
    def_ns: Arc<str>,       // ns
    id: Arc<str>,           // an id
    args: Arc<CalcitItems>, // args
    body: Arc<CalcitItems>, // body
  },
  Fn {
    name: Arc<str>,   // name
    def_ns: Arc<str>, // ns
    id: Arc<str>,     // an id
    scope: Arc<CalcitScope>,
    args: Arc<CalcitItems>, // args
    body: Arc<CalcitItems>, // body
  },
  Syntax(CalcitSyntax, Arc<str>), // name, ns... notice that `ns` is a meta info
}

impl fmt::Display for Calcit {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Calcit::Nil => f.write_str("nil"),
      Calcit::Bool(v) => f.write_str(&format!("{}", v)),
      Calcit::Number(n) => f.write_str(&format!("{}", n)),
      Calcit::Symbol { sym, .. } => f.write_str(&format!("'{}", sym)),
      Calcit::Keyword(s) => f.write_str(&format!(":{}", s.to_string())),
      Calcit::Str(s) => {
        if is_simple_str(s) {
          write!(f, "|{}", s)
        } else {
          write!(f, "\"|{}\"", s.escape_default())
        }
      } // TODO, escaping choices
      Calcit::Thunk(code, v) => match v {
        Some(data) => f.write_str(&format!("(&thunk {} {})", data, code)),
        None => f.write_str(&format!("(&thunk _ {})", code)),
      },
      Calcit::Ref(name) => f.write_str(&format!("(&ref {})", name)),
      Calcit::Tuple(a, b) => f.write_str(&format!("(:: {} {})", a, b)),
      Calcit::Buffer(buf) => {
        f.write_str("(&buffer")?;
        if buf.len() > 8 {
          f.write_str(&format!(
            " {} {} {} {} {} {} {} {} ..+{}",
            buffer_bit_hex(buf[0]),
            buffer_bit_hex(buf[1]),
            buffer_bit_hex(buf[2]),
            buffer_bit_hex(buf[3]),
            buffer_bit_hex(buf[4]),
            buffer_bit_hex(buf[5]),
            buffer_bit_hex(buf[6]),
            buffer_bit_hex(buf[7]),
            buf.len() - 8
          ))?;
        } else {
          for b in buf {
            f.write_str(" ")?;
            f.write_str(&buffer_bit_hex(b.to_owned()))?;
          }
        }
        f.write_str(")")
      }
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
        f.write_str(&format!("(%{{}} {}", Calcit::Keyword(name.to_owned())))?;
        for idx in 0..fields.len() {
          f.write_str(&format!(" ({} {})", Calcit::Keyword(fields[idx].to_owned()), values[idx]))?;
        }
        f.write_str(")")
      }
      Calcit::Proc(name) => f.write_str(&format!("(&proc {})", name)),
      Calcit::Macro { name, args, body, .. } => {
        f.write_str(&format!("(&macro {} (", name))?;
        let mut need_space = false;
        for a in &**args {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(a))?;
          need_space = true;
        }
        f.write_str(") (")?;
        need_space = false;
        for b in &**body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str("))")
      }
      Calcit::Fn { name, args, body, .. } => {
        f.write_str(&format!("(&fn {} (", name))?;
        let mut need_space = false;
        for a in &**args {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(a))?;
          need_space = true;
        }
        f.write_str(") ")?;
        need_space = false;
        for b in &**body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str(")")
      }
      Calcit::Syntax(name, _ns) => f.write_str(&format!("(&syntax {})", name)),
    }
  }
}

fn is_simple_str(tok: &str) -> bool {
  for c in tok.chars() {
    if !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '?' | '!' | '|') {
      return false;
    }
  }
  true
}

fn buffer_bit_hex(n: u8) -> String {
  hex::encode(vec![n])
}

/// special types wraps vector of calcit data for displaying
impl fmt::Display for CrListWrap {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&format_to_lisp(&Calcit::List(self.0.to_owned()))) // TODO performance
  }
}

/// display data into Lisp style for readability
pub fn format_to_lisp(x: &Calcit) -> String {
  match x {
    Calcit::List(ys) => {
      let mut s = String::from("(");
      for (idx, y) in ys.into_iter().enumerate() {
        if idx > 0 {
          s.push(' ');
        }
        s.push_str(&format_to_lisp(y));
      }
      s.push(')');
      s
    }
    Calcit::Symbol { sym, .. } => sym.to_string(),
    Calcit::Syntax(s, _ns) => s.to_string(),
    Calcit::Proc(s) => s.to_string(),
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
      Calcit::Symbol { sym, .. } => {
        "symbol:".hash(_state);
        sym.hash(_state);
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
      Calcit::Thunk(v, _) => {
        "quote:".hash(_state);
        v.hash(_state);
      }
      Calcit::Ref(name) => {
        "ref:".hash(_state);
        name.hash(_state);
      }
      Calcit::Tuple(a, b) => {
        "tuple:".hash(_state);
        a.hash(_state);
        b.hash(_state);
      }
      Calcit::Buffer(buf) => {
        "buffer:".hash(_state);
        buf.hash(_state);
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
      Calcit::Macro { id: gen_id, .. } => {
        "macro:".hash(_state);
        // name.hash(_state);
        gen_id.hash(_state);
      }
      Calcit::Fn { id: gen_id, .. } => {
        "fn:".hash(_state);
        // name.hash(_state);
        gen_id.hash(_state);
      }
      Calcit::Syntax(name, _ns) => {
        "syntax:".hash(_state);
        // syntax name can be used as identity
        name.to_string().hash(_state); // TODO
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

      (Calcit::Symbol { sym: a, .. }, Calcit::Symbol { sym: b, .. }) => a.cmp(b),
      (Calcit::Symbol { .. }, _) => Less,
      (_, Calcit::Symbol { .. }) => Greater,

      (Calcit::Keyword(a), Calcit::Keyword(b)) => a.cmp(b),
      (Calcit::Keyword(_), _) => Less,
      (_, Calcit::Keyword(_)) => Greater,

      (Calcit::Str(a), Calcit::Str(b)) => a.cmp(b),
      (Calcit::Str(_), _) => Less,
      (_, Calcit::Str(_)) => Greater,

      (Calcit::Thunk(a, _), Calcit::Thunk(b, _)) => a.cmp(b),
      (Calcit::Thunk(_, _), _) => Less,
      (_, Calcit::Thunk(_, _)) => Greater,

      (Calcit::Ref(a), Calcit::Ref(b)) => a.cmp(b),
      (Calcit::Ref(_), _) => Less,
      (_, Calcit::Ref(_)) => Greater,

      (Calcit::Tuple(a0, b0), Calcit::Tuple(a1, b1)) => match a0.cmp(a1) {
        Equal => b0.cmp(b1),
        v => v,
      },
      (Calcit::Tuple(_, _), _) => Less,
      (_, Calcit::Tuple(_, _)) => Greater,

      (Calcit::Buffer(buf1), Calcit::Buffer(buf2)) => buf1.cmp(buf2),
      (Calcit::Buffer(..), _) => Less,
      (_, Calcit::Buffer(..)) => Greater,

      (Calcit::Recur(a), Calcit::Recur(b)) => a.cmp(b),
      (Calcit::Recur(_), _) => Less,
      (_, Calcit::Recur(_)) => Greater,

      (Calcit::List(a), Calcit::List(b)) => a.cmp(b),
      (Calcit::List(_), _) => Less,
      (_, Calcit::List(_)) => Greater,

      (Calcit::Set(a), Calcit::Set(b)) => match a.size().cmp(&b.size()) {
        Equal => {
          if a == b {
            Equal
          } else {
            unreachable!("TODO sets are not cmp ed") // TODO
          }
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

      (Calcit::Macro { id: a, .. }, Calcit::Macro { id: b, .. }) => a.cmp(b),
      (Calcit::Macro { .. }, _) => Less,
      (_, Calcit::Macro { .. }) => Greater,

      (Calcit::Fn { id: a, .. }, Calcit::Fn { id: b, .. }) => a.cmp(b), // compared with nanoid
      (Calcit::Fn { .. }, _) => Less,
      (_, Calcit::Fn { .. }) => Greater,

      (Calcit::Syntax(a, _), Calcit::Syntax(b, _)) => a.cmp(b),
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
      (Calcit::Symbol { sym: a, .. }, Calcit::Symbol { sym: b, .. }) => a == b,
      (Calcit::Keyword(a), Calcit::Keyword(b)) => a == b,
      (Calcit::Str(a), Calcit::Str(b)) => a == b,
      (Calcit::Thunk(a, _), Calcit::Thunk(b, _)) => a == b,
      (Calcit::Ref(a), Calcit::Ref(b)) => a == b,
      (Calcit::Tuple(a, b), Calcit::Tuple(c, d)) => a == c && b == d,
      (Calcit::Buffer(b), Calcit::Buffer(d)) => b == d,
      (Calcit::List(a), Calcit::List(b)) => a == b,
      (Calcit::Set(a), Calcit::Set(b)) => a == b,
      (Calcit::Map(a), Calcit::Map(b)) => a == b,
      (Calcit::Record(name1, fields1, values1), Calcit::Record(name2, fields2, values2)) => {
        name1 == name2 && fields1 == fields2 && values1 == values2
      }

      // functions compared with nanoid
      (Calcit::Proc(a), Calcit::Proc(b)) => a == b,
      (Calcit::Macro { id: a, .. }, Calcit::Macro { id: b, .. }) => a == b,
      (Calcit::Fn { id: a, .. }, Calcit::Fn { id: b, .. }) => a == b,
      (Calcit::Syntax(a, _), Calcit::Syntax(b, _)) => a == b,
      (_, _) => false,
    }
  }
}

pub const CORE_NS: &str = "calcit.core";
pub const BUILTIN_CLASSES_ENTRY: &str = "&init-builtin-classes!";
pub const GENERATED_NS: &str = "calcit.gen";
pub const GENERATED_DEF: &str = "gen%";

lazy_static! {
  pub static ref GEN_NS: Arc<str> = GENERATED_NS.to_string().into();
  pub static ref GEN_DEF: Arc<str> = GENERATED_DEF.to_string().into();
  pub static ref GEN_CORE_NS: Arc<str> = CORE_NS.to_string().into();
  pub static ref GEN_CLASS_ENTRY: Arc<str> = BUILTIN_CLASSES_ENTRY.to_string().into();
}

impl Calcit {
  pub fn turn_string(&self) -> String {
    match self {
      Calcit::Nil => String::from(""),
      Calcit::Str(s) => (**s).to_owned(),
      _ => format!("{}", self),
    }
  }

  pub fn lisp_str(&self) -> String {
    format_to_lisp(self)
  }

  pub fn new_str<T: Into<String>>(s: T) -> Calcit {
    Calcit::Str(s.into().into())
  }

  /// makes sure that keyword is from global dict, not created by fresh
  pub fn kwd(s: &str) -> Self {
    Calcit::Keyword(EdnKwd::from(s))
  }
}

/// too naive id generator to be safe in WASM
pub fn gen_core_id() -> Arc<str> {
  let c = ID_GEN.fetch_add(1, SeqCst);
  format!("gen_id_{}", c).into()
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalcitErr {
  pub msg: String,
  pub warnings: Vec<String>,
  pub stack: rpds::ListSync<crate::call_stack::CalcitStack>,
}

impl fmt::Display for CalcitErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.msg)?;
    if !self.warnings.is_empty() {
      f.write_str("\n")?;
      for w in &self.warnings {
        writeln!(f, "{}", w)?;
      }
    }
    Ok(())
  }
}

impl From<String> for CalcitErr {
  /// hope this does not add extra costs
  fn from(msg: String) -> Self {
    CalcitErr {
      msg,
      warnings: vec![],
      stack: rpds::List::new_sync(),
    }
  }
}

impl CalcitErr {
  pub fn use_str<T: Into<String>>(msg: T) -> Self {
    CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: rpds::List::new_sync(),
    }
  }
  pub fn err_str<T: Into<String>>(msg: T) -> Result<Calcit, Self> {
    Err(CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: rpds::List::new_sync(),
    })
  }
  pub fn use_msg_stack<T: Into<String>>(msg: T, stack: &CallStackList) -> Self {
    CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: stack.to_owned(),
    }
  }
}

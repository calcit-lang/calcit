mod fns;
mod list;
mod local;
mod proc_name;
mod record;
mod symbol;
mod syntax_name;
mod thunk;
mod tuple;

use core::cmp::Ord;
use std::cmp::Eq;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::fmt::Display;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use cirru_edn::EdnAnyRef;
use cirru_edn::{Edn, EdnTag};
use cirru_parser::Cirru;
use im_ternary_tree::TernaryTreeList;

pub use fns::{CalcitArgLabel, CalcitFn, CalcitFnArgs, CalcitMacro, CalcitScope};
pub use list::CalcitList;
pub use local::CalcitLocal;
pub use proc_name::CalcitProc;
pub use record::CalcitRecord;
pub use symbol::{CalcitImport, CalcitSymbolInfo, ImportInfo};
pub use syntax_name::CalcitSyntax;
pub use thunk::{CalcitThunk, CalcitThunkInfo};
pub use tuple::CalcitTuple;

use crate::builtins::ValueAndListeners;
use crate::call_stack::CallStackList;

/// dead simple counter for ID generator, better use nanoid in business
static ID_GEN: AtomicUsize = AtomicUsize::new(0);

/// dynamic data defined in Calcit
#[derive(Debug, Clone)]
pub enum Calcit {
  Nil,
  Bool(bool),
  Number(f64),
  Symbol {
    sym: Arc<str>,
    info: Arc<CalcitSymbolInfo>,
    /// positions in the tree of Cirru
    location: Option<Arc<Vec<u8>>>,
  },
  /// local variable
  Local(CalcitLocal),
  /// things that can be looked up from program snapshot, also things in :require block
  Import(CalcitImport),
  /// registered in runtime
  Registered(Arc<str>),
  /// sth between string and enum, used a key or weak identifier
  Tag(EdnTag),
  Str(Arc<str>),
  /// to compile to js, global variables are stored in thunks at first, rather than evaluated
  /// and it is still different from quoted data which was intentionally turned in to data.
  Thunk(CalcitThunk), // code, value
  /// atom, holding a path to its state, data inside remains during hot code swapping
  Ref(Arc<str>, Arc<Mutex<ValueAndListeners>>),
  /// more tagged union type, more like an internal structure
  Tuple(CalcitTuple),
  /// binary data, to be used by FFIs
  Buffer(Vec<u8>),
  /// cirru quoted data, for faster meta programming
  CirruQuote(Cirru),
  /// not for data, but for recursion
  Recur(Vec<Calcit>),
  List(Arc<CalcitList>),
  Set(rpds::HashTrieSetSync<Calcit>),
  Map(rpds::HashTrieMapSync<Calcit, Calcit>),
  /// with only static and limited keys, for performance and checking
  /// size of keys are values should be kept consistent
  Record(CalcitRecord),
  /// native functions that providing feature from Rust
  Proc(CalcitProc),
  Macro {
    id: Arc<str>,
    info: Arc<CalcitMacro>,
  },
  Fn {
    id: Arc<str>,
    info: Arc<CalcitFn>,
  },
  /// name, ns... notice that `ns` is a meta info
  Syntax(CalcitSyntax, Arc<str>),
  /// Method is kind like macro, it's handled during preprocessing, into `&invoke` or `&invoke-native`
  /// method name, method kind
  Method(Arc<str>, MethodKind),
  /// currently only JavaScript calls are handled
  RawCode(RawCodeType, Arc<str>),
  /// reference to native Rust data, not intended for Calcit usages
  AnyRef(EdnAnyRef),
}

#[derive(Debug, Clone)]
pub enum RawCodeType {
  Js,
}

impl fmt::Display for Calcit {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Calcit::Nil => f.write_str("nil"),
      Calcit::Bool(v) => f.write_str(&format!("{v}")),
      Calcit::Number(n) => f.write_str(&format!("{n}")),
      Calcit::Symbol { sym, .. } => f.write_str(&format!("'{sym}")),
      Calcit::Local(CalcitLocal { sym, .. }) => f.write_str(&format!("'{sym}")),
      Calcit::Import(CalcitImport { ns, def, .. }) => f.write_str(&format!("{ns}/{def}")),
      Calcit::Registered(alias) => f.write_str(&format!("{alias}")),
      Calcit::Tag(s) => f.write_str(&format!(":{s}")),
      Calcit::Str(s) => {
        if is_simple_str(s) {
          write!(f, "|{s}")
        } else {
          // write!(f, "\"|{}\"", s.escape_default())
          write!(f, "\"|")?;
          for c in s.chars() {
            if cirru_edn::is_simple_char(c) {
              write!(f, "{}", c)?;
            } else {
              write!(f, "{}", c.escape_default())?;
            }
          }
          write!(f, "\"")
        }
      } // TODO, escaping choices
      Calcit::Thunk(thunk) => match thunk {
        CalcitThunk::Code { code, .. } => f.write_str(&format!("(&thunk _ {code})")),
        CalcitThunk::Evaled { code, value } => f.write_str(&format!("(&thunk {value} {code})")),
      },
      Calcit::CirruQuote(code) => f.write_str(&format!("(&cirru-quote {code})")),
      Calcit::Ref(name, _locked_pair) => f.write_str(&format!("(&ref {name} ...)")),
      Calcit::Tuple(CalcitTuple { tag, extra, class }) => {
        if let Some(record) = class {
          f.write_str("(%:: ")?;
          f.write_str(&tag.to_string())?;

          for item in extra {
            f.write_char(' ')?;
            f.write_str(&item.to_string())?;
          }
          f.write_str(&format!(" (:class {})", record.name))?;
          f.write_str(")")
        } else {
          f.write_str("(:: ")?;
          f.write_str(&tag.to_string())?;

          for item in extra {
            f.write_char(' ')?;
            f.write_str(&item.to_string())?;
          }

          f.write_str(")")
        }
      }
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
        for x in &**xs {
          f.write_str(&format!(" {x}"))?;
        }
        f.write_str(")")
      }
      Calcit::List(xs) => {
        f.write_str("([]")?;
        xs.traverse_result(&mut |x| match f.write_str(&format!(" {x}")) {
          Ok(_) => Ok(()),
          Err(e) => Err(e),
        })?;
        f.write_str(")")
      }
      Calcit::Set(xs) => {
        f.write_str("(#{}")?;
        for x in xs {
          f.write_str(&format!(" {x}"))?;
        }
        f.write_str(")")
      }
      Calcit::Map(xs) => {
        f.write_str("({}")?;
        for (k, v) in xs {
          f.write_str(&format!(" ({k} {v})"))?;
        }
        f.write_str(")")?;
        Ok(())
      }
      Calcit::Record(CalcitRecord { name, fields, values, .. }) => {
        f.write_str(&format!("(%{{}} {}", Calcit::Tag(name.to_owned())))?;
        for idx in 0..fields.len() {
          f.write_str(&format!(" ({} {})", Calcit::tag(fields[idx].ref_str()), values[idx]))?;
        }
        f.write_str(")")
      }
      Calcit::Proc(name) => f.write_str(&format!("(&proc {name})")),
      Calcit::Macro { info, .. } => {
        let name = &info.name;
        f.write_str(&format!("(&macro {name} ("))?;
        let mut need_space = false;
        for a in &**info.args {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&a.to_string())?;
          need_space = true;
        }
        f.write_str(") (")?;
        need_space = false;
        for b in &*info.body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str("))")
      }
      Calcit::Fn { info, .. } => {
        let name = &info.name;
        f.write_str(&format!("(&fn {name} ("))?;
        let mut need_space = false;
        match &*info.args {
          CalcitFnArgs::MarkedArgs(xs) => {
            for a in xs {
              if need_space {
                f.write_str(" ")?;
              }
              f.write_str(&a.to_string())?;
              need_space = true;
            }
          }
          CalcitFnArgs::Args(xs) => {
            for a in xs {
              if need_space {
                f.write_str(" ")?;
              }
              f.write_str(&a.to_string())?;
              need_space = true;
            }
          }
        }
        f.write_str(") ")?;
        need_space = false;
        for b in &*info.body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str(")")
      }
      Calcit::Syntax(name, _ns) => f.write_str(&format!("(&syntax {name})")),
      Calcit::Method(name, method_kind) => f.write_str(&format!("(&{method_kind} {name})")),
      Calcit::RawCode(_, code) => f.write_str(&format!("(&raw-code {code})")),
      Calcit::AnyRef(_r) => f.write_str("(&any-ref ...)"),
    }
  }
}

fn is_simple_str(tok: &str) -> bool {
  for c in tok.chars() {
    if !cirru_edn::is_simple_char(c) {
      return false;
    }
  }
  true
}

/// encode as hex string like `ff`
fn buffer_bit_hex(n: u8) -> String {
  hex::encode(vec![n])
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
    Calcit::Symbol { sym, .. } => sym.to_string(),
    Calcit::Local(CalcitLocal { sym, .. }) => sym.to_string(),
    Calcit::Import(CalcitImport { ns, def, .. }) => format!("{ns}/{def}"),
    Calcit::Registered(alias) => format!("{alias}"),
    Calcit::Tag(s) => format!(":{s}"),
    Calcit::Syntax(s, _ns) => s.to_string(),
    Calcit::Proc(s) => s.to_string(),
    a => format!("{a}"),
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
      Calcit::Local(CalcitLocal { sym, .. }) => {
        "local:".hash(_state);
        sym.hash(_state);
      }
      Calcit::Import(CalcitImport { ns, def, .. }) => {
        "import:".hash(_state);
        ns.hash(_state);
        def.hash(_state);
      }
      Calcit::Registered(alias) => {
        "registered:".hash(_state);
        alias.hash(_state);
      }
      Calcit::Tag(s) => {
        "tag:".hash(_state);
        s.hash(_state);
      }
      Calcit::Str(s) => {
        "string:".hash(_state);
        s.hash(_state);
      }
      Calcit::Thunk(..) => {
        unreachable!("thunk should not be used in hashing")
      }
      Calcit::Ref(name, _locked_pair) => {
        "ref:".hash(_state);
        name.hash(_state);
      }
      Calcit::Tuple(CalcitTuple { tag, extra, .. }) => {
        "tuple:".hash(_state);
        tag.hash(_state);
        extra.hash(_state);
        // _class is internal prototype data, not used in hashing
      }
      Calcit::Buffer(buf) => {
        "buffer:".hash(_state);
        buf.hash(_state);
      }
      Calcit::CirruQuote(code) => {
        "cirru-quote:".hash(_state);
        code.hash(_state);
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
        let mut xs: Vec<_> = v.iter().collect();
        // sort to ensure stable result
        xs.sort();
        for x in xs {
          x.hash(_state)
        }
      }
      Calcit::Map(v) => {
        "map:".hash(_state);
        // order for map is not stable
        let mut xs: Vec<_> = v.iter().collect();
        xs.sort();
        for x in xs {
          x.hash(_state)
        }
      }
      Calcit::Record(CalcitRecord { name, fields, values, .. }) => {
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
        name.to_string().hash(_state);
      }
      Calcit::Method(name, call_native) => {
        "method:".hash(_state);
        name.hash(_state);
        call_native.hash(_state);
      }
      Calcit::RawCode(_name, code) => {
        "raw-code:".hash(_state);
        code.hash(_state);
      }
      Calcit::AnyRef(_) => {
        unreachable!("AnyRef should not be used in hashing")
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

      (Calcit::Local(CalcitLocal { sym: a, .. }), Calcit::Local(CalcitLocal { sym: b, .. })) => a.cmp(b),
      (Calcit::Local { .. }, _) => Less,
      (_, Calcit::Local { .. }) => Greater,

      (Calcit::Import(CalcitImport { ns: a, def: a1, .. }), Calcit::Import(CalcitImport { ns: b, def: b1, .. })) => {
        a.cmp(b).then(a1.cmp(b1))
      }
      (Calcit::Import { .. }, _) => Less,
      (_, Calcit::Import { .. }) => Greater,

      (Calcit::Registered(a), Calcit::Registered(b)) => a.cmp(b),
      (Calcit::Registered(_), _) => Less,
      (_, Calcit::Registered(_)) => Greater,

      (Calcit::Tag(a), Calcit::Tag(b)) => a.cmp(b),
      (Calcit::Tag(_), _) => Less,
      (_, Calcit::Tag(_)) => Greater,

      (Calcit::Str(a), Calcit::Str(b)) => a.cmp(b),
      (Calcit::Str(_), _) => Less,
      (_, Calcit::Str(_)) => Greater,

      (Calcit::Thunk(a), Calcit::Thunk(b)) => a.cmp(b),
      (Calcit::Thunk(_), _) => Less,
      (_, Calcit::Thunk(_)) => Greater,

      (Calcit::CirruQuote(a), Calcit::CirruQuote(b)) => a.cmp(b),
      (Calcit::CirruQuote(_), _) => Less,
      (_, Calcit::CirruQuote(_)) => Greater,

      (Calcit::Ref(a, _), Calcit::Ref(b, _)) => a.cmp(b),
      (Calcit::Ref(_, _), _) => Less,
      (_, Calcit::Ref(_, _)) => Greater,

      (
        Calcit::Tuple(CalcitTuple {
          tag: a0, extra: extra0, ..
        }),
        Calcit::Tuple(CalcitTuple {
          tag: a1, extra: extra1, ..
        }),
      ) => match a0.cmp(a1) {
        Equal => extra0.cmp(extra1),
        v => v,
      },
      (Calcit::Tuple { .. }, _) => Less,
      (_, Calcit::Tuple { .. }) => Greater,

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
            unreachable!("TODO sets are not cmp ed")
          }
        }
        a => a,
      },
      (Calcit::Set(_), _) => Less,
      (_, Calcit::Set(_)) => Greater,

      (Calcit::Map(a), Calcit::Map(b)) => {
        unreachable!("TODO maps are not cmp ed {:?} {:?}", a, b)
      }
      (Calcit::Map(_), _) => Less,
      (_, Calcit::Map(_)) => Greater,

      (Calcit::Record(CalcitRecord { name: name1, .. }), Calcit::Record(CalcitRecord { name: name2, .. })) => match name1.cmp(name2) {
        Equal => unreachable!("TODO records are not cmp ed"),
        ord => ord,
      },
      (Calcit::Record { .. }, _) => Less,
      (_, Calcit::Record { .. }) => Greater,

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
      (Calcit::Syntax(..), _) => Less,
      (_, Calcit::Syntax(..)) => Greater,

      (Calcit::Method(a, na), Calcit::Method(b, nb)) => match a.cmp(b) {
        Equal => na.cmp(nb),
        v => v,
      },
      (Calcit::Method(..), _) => Less,
      (_, Calcit::Method(..)) => Greater,

      (Calcit::RawCode(_, a), Calcit::RawCode(_, b)) => a.cmp(b),
      (Calcit::RawCode(..), _) => Less,
      (_, Calcit::RawCode(..)) => Greater,

      (Calcit::AnyRef(_), Calcit::AnyRef(_)) => unreachable!("AnyRef should not be used in cmp"),
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
      (Calcit::Local(CalcitLocal { sym: a, .. }), Calcit::Local(CalcitLocal { sym: b, .. })) => a == b,

      // special case for symbol and local, compatible with old implementation
      (Calcit::Symbol { sym: a, .. }, Calcit::Local(CalcitLocal { sym: b, .. })) => a == b,
      (Calcit::Local(CalcitLocal { sym: a, .. }), Calcit::Symbol { sym: b, .. }) => a == b,
      (Calcit::Symbol { sym: a, .. }, Calcit::Import(CalcitImport { def: b, .. })) => a == b,
      (Calcit::Import(CalcitImport { def: b, .. }), Calcit::Symbol { sym: a, .. }) => a == b,
      (Calcit::Registered(a), Calcit::Registered(b)) => a == b,

      (Calcit::Import(CalcitImport { ns: a, def: a1, .. }), Calcit::Import(CalcitImport { ns: b, def: b1, .. })) => a == b && a1 == b1,
      (Calcit::Tag(a), Calcit::Tag(b)) => a == b,
      (Calcit::Str(a), Calcit::Str(b)) => a == b,
      (Calcit::Thunk(a), Calcit::Thunk(b)) => a == b,
      (Calcit::Ref(a, _), Calcit::Ref(b, _)) => a == b,
      (Calcit::Tuple(a), Calcit::Tuple(b)) => a == b,
      (Calcit::Buffer(b), Calcit::Buffer(d)) => b == d,
      (Calcit::CirruQuote(b), Calcit::CirruQuote(d)) => b == d,
      (Calcit::List(a), Calcit::List(b)) => a == b,
      (Calcit::Set(a), Calcit::Set(b)) => a == b,
      (Calcit::Map(a), Calcit::Map(b)) => a == b,
      (Calcit::Record(a), Calcit::Record(b)) => a == b,
      (Calcit::Proc(a), Calcit::Proc(b)) => a == b,
      (Calcit::Macro { id: a, .. }, Calcit::Macro { id: b, .. }) => a == b,
      // functions compared with nanoid
      (Calcit::Fn { id: a, .. }, Calcit::Fn { id: b, .. }) => a == b,
      (Calcit::Syntax(a, _), Calcit::Syntax(b, _)) => a == b,
      (Calcit::Method(a, b), Calcit::Method(c, d)) => a == c && b == d,
      (Calcit::AnyRef(a), Calcit::AnyRef(b)) => a == b,
      (_, _) => false,
    }
  }
}

impl From<TernaryTreeList<Calcit>> for Calcit {
  fn from(xs: TernaryTreeList<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::List(xs)))
  }
}

impl From<Vec<Calcit>> for Calcit {
  fn from(xs: Vec<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::Vector(xs)))
  }
}

impl From<&TernaryTreeList<Calcit>> for Calcit {
  fn from(xs: &TernaryTreeList<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::from(xs)))
  }
}

pub const CORE_NS: &str = "calcit.core";
pub const BUILTIN_CLASSES_ENTRY: &str = "&init-builtin-classes!";
pub const GEN_NS: &str = "calcit.gen";
pub const GENERATED_DEF: &str = "gen%";

impl Calcit {
  /// data converting, not displaying
  pub fn turn_string(&self) -> String {
    match self {
      Calcit::Nil => String::from(""),
      Calcit::Str(s) => (**s).to_owned(),
      _ => format!("{self}"),
    }
  }

  pub fn lisp_str(&self) -> String {
    format_to_lisp(self)
  }

  pub fn new_str<T: Into<String>>(s: T) -> Calcit {
    Calcit::Str(s.into().into())
  }

  /// makes sure that tag is from global dict, not created by fresh
  pub fn tag(s: &str) -> Self {
    Calcit::Tag(EdnTag::from(s))
  }

  /// currently only symbol has node location
  pub fn get_location(&self) -> Option<NodeLocation> {
    match self {
      Calcit::Symbol { info, location, .. } => Some(NodeLocation::new(
        info.at_ns.to_owned(),
        info.at_def.to_owned(),
        location.to_owned().unwrap_or_default(),
      )),
      Calcit::Local(CalcitLocal { info, location, .. }) => Some(NodeLocation::new(
        info.at_ns.to_owned(),
        info.at_def.to_owned(),
        location.to_owned().unwrap_or_default(),
      )),
      _ => None,
    }
  }

  /// during evaluation, maybe skip evaluation since evaluated data is already in the value
  pub fn is_expr_evaluated(&self) -> bool {
    !matches!(
      self,
      // variants that need to be further evaluated
      Calcit::Symbol { .. } | Calcit::Local { .. } | Calcit::Import(..) | Calcit::Thunk(..) | Calcit::List(..)
    )
  }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn gen_core_id() -> Arc<str> {
  use std::time::{SystemTime, UNIX_EPOCH};

  let c = ID_GEN.fetch_add(1, SeqCst);
  let start = SystemTime::now();
  let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
  let in_ms = since_the_epoch.as_millis();

  format!("gen_id_{c}_{in_ms}").into()
}

/// time lib not available for WASM. TODO id may not be unique
#[cfg(target_arch = "wasm32")]
pub fn gen_core_id() -> Arc<str> {
  let c = ID_GEN.fetch_add(1, SeqCst);
  format!("gen_id_{c}").into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitErr {
  pub msg: String,
  pub warnings: Vec<LocatedWarning>,
  pub location: Option<Arc<NodeLocation>>,
  pub stack: CallStackList,
}

impl fmt::Display for CalcitErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.msg)?;
    if !self.warnings.is_empty() {
      f.write_str("\n")?;
      LocatedWarning::print_list(&self.warnings);
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
      stack: CallStackList::default(),
      location: None,
    }
  }
}

impl CalcitErr {
  pub fn use_str<T: Into<String>>(msg: T) -> Self {
    CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    }
  }
  pub fn err_str<T: Into<String>>(msg: T) -> Result<Calcit, Self> {
    Err(CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    })
  }
  /// display nodes in error message
  pub fn err_nodes<T: Into<String>>(msg: T, nodes: &[Calcit]) -> Result<Calcit, Self> {
    Err(CalcitErr {
      msg: format!("{} {}", msg.into(), CalcitList::from(nodes)),
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    })
  }
  pub fn err_str_location<T: Into<String>>(msg: T, location: Option<Arc<NodeLocation>>) -> Result<Calcit, Self> {
    Err(CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: CallStackList::default(),
      location,
    })
  }
  pub fn use_msg_stack<T: Into<String>>(msg: T, stack: &CallStackList) -> Self {
    CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: stack.to_owned(),
      location: None,
    }
  }
  pub fn use_msg_stack_location<T: Into<String>>(msg: T, stack: &CallStackList, location: Option<NodeLocation>) -> Self {
    CalcitErr {
      msg: msg.into(),
      warnings: vec![],
      stack: stack.to_owned(),
      location: location.map(Arc::new),
    }
  }
}

/// location of node in Snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLocation {
  pub ns: Arc<str>,
  pub def: Arc<str>,
  pub coord: Arc<Vec<u8>>,
}

impl From<NodeLocation> for Edn {
  fn from(v: NodeLocation) -> Self {
    Edn::map_from_iter([
      (Edn::tag("ns"), v.ns.into()),
      (Edn::tag("def"), v.def.into()),
      (Edn::tag("coord"), (*v.coord).to_owned().into()),
    ])
  }
}

impl From<&NodeLocation> for Edn {
  fn from(v: &NodeLocation) -> Self {
    v.to_owned().into()
  }
}

impl fmt::Display for NodeLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}/{} {}",
      self.ns,
      self.def,
      self.coord.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("-")
    )
  }
}

impl NodeLocation {
  pub fn new(ns: Arc<str>, def: Arc<str>, coord: Arc<Vec<u8>>) -> Self {
    NodeLocation {
      ns,
      def,
      coord: coord.to_owned(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedWarning(String, NodeLocation);

impl Display for LocatedWarning {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} @{}", self.0, self.1)
  }
}

/// warning from static checking of macro expanding
impl LocatedWarning {
  pub fn new(msg: String, location: NodeLocation) -> Self {
    LocatedWarning(msg, location)
  }

  /// create an empty list
  pub fn default_list() -> Vec<Self> {
    vec![]
  }

  pub fn print_list(list: &Vec<Self>) {
    for warn in list {
      println!("{warn}");
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MethodKind {
  /// (.call a)
  Invoke,
  /// (.!f a)
  InvokeNative,
  /// (.?!f a)
  InvokeNativeOptional,
  /// (.-p a)
  Access,
  /// (.?-p a)
  AccessOptional,
}

impl fmt::Display for MethodKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      MethodKind::Invoke => write!(f, "invoke"),
      MethodKind::InvokeNative => write!(f, "invoke-native"),
      MethodKind::InvokeNativeOptional => write!(f, "invoke-native-optional"),
      MethodKind::Access => write!(f, "access"),
      MethodKind::AccessOptional => write!(f, "access-optional"),
    }
  }
}

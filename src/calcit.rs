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
    location: Option<Arc<Vec<u16>>>,
  },
  /// local variable
  Local(CalcitLocal),
  /// things that can be looked up from program snapshot, also things in :require block
  Import(CalcitImport),
  /// registered in runtime
  Registered(Arc<str>),
  /// something between string and enum, used a key or weak identifier
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
    use Calcit::*;
    match self {
      Nil => f.write_str("nil"),
      Bool(v) => f.write_str(&format!("{v}")),
      Number(n) => f.write_str(&format!("{n}")),
      Symbol { sym, .. } => f.write_str(&format!("'{sym}")),
      Local(CalcitLocal { sym, .. }) => f.write_str(&format!("'{sym}")),
      Import(CalcitImport { ns, def, .. }) => f.write_str(&format!("{ns}/{def}")),
      Registered(alias) => f.write_str(&format!("{alias}")),
      Tag(s) => f.write_str(&format!(":{s}")),
      Str(s) => {
        if is_simple_str(s) {
          write!(f, "|{s}")
        } else {
          // write!(f, "\"|{}\"", s.escape_default())
          write!(f, "\"|")?;
          for c in s.chars() {
            if cirru_edn::is_simple_char(c) {
              write!(f, "{c}")?;
            } else {
              write!(f, "{}", c.escape_default())?;
            }
          }
          write!(f, "\"")
        }
      } // TODO, escaping choices
      Thunk(thunk) => match thunk {
        CalcitThunk::Code { code, .. } => f.write_str(&format!("(&thunk _ {code})")),
        CalcitThunk::Evaled { code, value } => f.write_str(&format!("(&thunk {value} {code})")),
      },
      CirruQuote(code) => f.write_str(&format!("(&cirru-quote {code})")),
      Ref(name, _locked_pair) => f.write_str(&format!("(&ref {name} ...)")),
      Tuple(CalcitTuple { tag, extra, class }) => {
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
      Buffer(buf) => {
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
      Recur(xs) => {
        f.write_str("(&recur")?;
        for x in &**xs {
          f.write_str(&format!(" {x}"))?;
        }
        f.write_str(")")
      }
      List(xs) => {
        f.write_str("([]")?;
        xs.traverse_result(&mut |x| match f.write_str(&format!(" {x}")) {
          Ok(_) => Ok(()),
          Err(e) => Err(e),
        })?;
        f.write_str(")")
      }
      Set(xs) => {
        f.write_str("(#{}")?;
        for x in xs {
          f.write_str(&format!(" {x}"))?;
        }
        f.write_str(")")
      }
      Map(xs) => {
        f.write_str("({}")?;
        for (k, v) in xs {
          f.write_str(&format!(" ({k} {v})"))?;
        }
        f.write_str(")")?;
        Ok(())
      }
      Record(CalcitRecord { name, fields, values, .. }) => {
        f.write_str(&format!("(%{{}} {}", Tag(name.to_owned())))?;
        for idx in 0..fields.len() {
          f.write_str(&format!(" ({} {})", Calcit::tag(fields[idx].ref_str()), values[idx]))?;
        }
        f.write_str(")")
      }
      Proc(name) => f.write_str(&format!("(&proc {name})")),
      Macro { info, .. } => {
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
      Fn { info, .. } => {
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
              f.write_str(&CalcitLocal::read_name(*a))?;
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
      Syntax(name, _ns) => f.write_str(&format!("(&syntax {name})")),
      Method(name, method_kind) => f.write_str(&format!("(&{method_kind} {name})")),
      RawCode(_, code) => f.write_str(&format!("(&raw-code {code})")),
      AnyRef(_r) => f.write_str("(&any-ref ...)"),
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
  use Calcit::*;
  match x {
    List(ys) => {
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
    Symbol { sym, .. } => sym.to_string(),
    Local(CalcitLocal { sym, .. }) => sym.to_string(),
    Import(CalcitImport { ns, def, .. }) => format!("{ns}/{def}"),
    Registered(alias) => format!("{alias}"),
    Tag(s) => format!(":{s}"),
    Syntax(s, _ns) => s.to_string(),
    Proc(s) => s.to_string(),
    a => format!("{a}"),
  }
}

impl Hash for Calcit {
  fn hash<H>(&self, _state: &mut H)
  where
    H: Hasher,
  {
    use Calcit::*;
    match self {
      Nil => "nil:".hash(_state),
      Bool(v) => {
        "bool:".hash(_state);
        v.hash(_state);
      }
      Number(n) => {
        "number:".hash(_state);
        // TODO https://stackoverflow.com/q/39638363/883571
        (*n as usize).hash(_state)
      }
      Symbol { sym, .. } => {
        "symbol:".hash(_state);
        sym.hash(_state);
        // probaly no need, also won't be used in hashing
        // ns.hash(_state);
      }
      Local(CalcitLocal { sym, .. }) => {
        "local:".hash(_state);
        sym.hash(_state);
      }
      Import(CalcitImport { ns, def, .. }) => {
        "import:".hash(_state);
        ns.hash(_state);
        def.hash(_state);
      }
      Registered(alias) => {
        "registered:".hash(_state);
        alias.hash(_state);
      }
      Tag(s) => {
        "tag:".hash(_state);
        s.hash(_state);
      }
      Str(s) => {
        "string:".hash(_state);
        s.hash(_state);
      }
      Thunk(..) => {
        unreachable!("thunk should not be used in hashing")
      }
      Ref(name, _locked_pair) => {
        "ref:".hash(_state);
        name.hash(_state);
      }
      Tuple(CalcitTuple { tag, extra, .. }) => {
        "tuple:".hash(_state);
        tag.hash(_state);
        extra.hash(_state);
        // _class is internal prototype data, not used in hashing
      }
      Buffer(buf) => {
        "buffer:".hash(_state);
        buf.hash(_state);
      }
      CirruQuote(code) => {
        "cirru-quote:".hash(_state);
        code.hash(_state);
      }
      Recur(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      List(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      Set(v) => {
        "set:".hash(_state);
        let mut xs: Vec<_> = v.iter().collect();
        // sort to ensure stable result
        xs.sort();
        for x in xs {
          x.hash(_state)
        }
      }
      Map(v) => {
        "map:".hash(_state);
        // order for map is not stable
        let mut xs: Vec<_> = v.iter().collect();
        xs.sort();
        for x in xs {
          x.hash(_state)
        }
      }
      Record(CalcitRecord { name, fields, values, .. }) => {
        "record:".hash(_state);
        name.hash(_state);
        fields.hash(_state);
        values.hash(_state);
      }
      Proc(name) => {
        "proc:".hash(_state);
        name.hash(_state);
      }
      Macro { id: gen_id, .. } => {
        "macro:".hash(_state);
        // name.hash(_state);
        gen_id.hash(_state);
      }
      Fn { id: gen_id, .. } => {
        "fn:".hash(_state);
        // name.hash(_state);
        gen_id.hash(_state);
      }
      Syntax(name, _ns) => {
        "syntax:".hash(_state);
        // syntax name can be used as identity
        name.to_string().hash(_state);
      }
      Method(name, call_native) => {
        "method:".hash(_state);
        name.hash(_state);
        call_native.hash(_state);
      }
      RawCode(_name, code) => {
        "raw-code:".hash(_state);
        code.hash(_state);
      }
      AnyRef(_) => {
        unreachable!("AnyRef should not be used in hashing")
      }
    }
  }
}

impl Ord for Calcit {
  fn cmp(&self, other: &Self) -> Ordering {
    use Calcit::*;

    match (self, other) {
      (Nil, Nil) => Equal,
      (Nil, _) => Less,
      (_, Nil) => Greater,

      (Bool(a), Bool(b)) => a.cmp(b),
      (Bool(_), _) => Less,
      (_, Bool(_)) => Greater,

      (Number(a), Number(b)) => {
        if a < b {
          Less
        } else if a > b {
          Greater
        } else {
          Equal
        }
      }
      (Number(_), _) => Less,
      (_, Number(_)) => Greater,

      (Symbol { sym: a, .. }, Symbol { sym: b, .. }) => a.cmp(b),
      (Symbol { .. }, _) => Less,
      (_, Symbol { .. }) => Greater,

      (Local(CalcitLocal { sym: a, .. }), Local(CalcitLocal { sym: b, .. })) => a.cmp(b),
      (Local { .. }, _) => Less,
      (_, Local { .. }) => Greater,

      (Import(CalcitImport { ns: a, def: a1, .. }), Import(CalcitImport { ns: b, def: b1, .. })) => a.cmp(b).then(a1.cmp(b1)),
      (Import { .. }, _) => Less,
      (_, Import { .. }) => Greater,

      (Registered(a), Registered(b)) => a.cmp(b),
      (Registered(_), _) => Less,
      (_, Registered(_)) => Greater,

      (Tag(a), Tag(b)) => a.cmp(b),
      (Tag(_), _) => Less,
      (_, Tag(_)) => Greater,

      (Str(a), Str(b)) => a.cmp(b),
      (Str(_), _) => Less,
      (_, Str(_)) => Greater,

      (Thunk(a), Thunk(b)) => a.cmp(b),
      (Thunk(_), _) => Less,
      (_, Thunk(_)) => Greater,

      (CirruQuote(a), CirruQuote(b)) => a.cmp(b),
      (CirruQuote(_), _) => Less,
      (_, CirruQuote(_)) => Greater,

      (Ref(a, _), Ref(b, _)) => a.cmp(b),
      (Ref(_, _), _) => Less,
      (_, Ref(_, _)) => Greater,

      (
        Tuple(CalcitTuple {
          tag: a0, extra: extra0, ..
        }),
        Tuple(CalcitTuple {
          tag: a1, extra: extra1, ..
        }),
      ) => match a0.cmp(a1) {
        Equal => extra0.cmp(extra1),
        v => v,
      },
      (Tuple { .. }, _) => Less,
      (_, Tuple { .. }) => Greater,

      (Buffer(buf1), Buffer(buf2)) => buf1.cmp(buf2),
      (Buffer(..), _) => Less,
      (_, Buffer(..)) => Greater,

      (Recur(a), Recur(b)) => a.cmp(b),
      (Recur(_), _) => Less,
      (_, Recur(_)) => Greater,

      (List(a), List(b)) => a.cmp(b),
      (List(_), _) => Less,
      (_, List(_)) => Greater,

      (Set(a), Set(b)) => match a.size().cmp(&b.size()) {
        Equal => {
          if a == b {
            Equal
          } else {
            unreachable!("TODO sets are not cmp ed")
          }
        }
        a => a,
      },
      (Set(_), _) => Less,
      (_, Set(_)) => Greater,

      (Map(a), Map(b)) => {
        unreachable!("TODO maps are not cmp ed {:?} {:?}", a, b)
      }
      (Map(_), _) => Less,
      (_, Map(_)) => Greater,

      (Record(CalcitRecord { name: name1, .. }), Record(CalcitRecord { name: name2, .. })) => match name1.cmp(name2) {
        Equal => unreachable!("TODO records are not cmp ed"),
        ord => ord,
      },
      (Record { .. }, _) => Less,
      (_, Record { .. }) => Greater,

      (Proc(a), Proc(b)) => a.cmp(b),
      (Proc(_), _) => Less,
      (_, Proc(_)) => Greater,

      (Macro { id: a, .. }, Macro { id: b, .. }) => a.cmp(b),
      (Macro { .. }, _) => Less,
      (_, Macro { .. }) => Greater,

      (Fn { id: a, .. }, Fn { id: b, .. }) => a.cmp(b), // compared with nanoid
      (Fn { .. }, _) => Less,
      (_, Fn { .. }) => Greater,

      (Syntax(a, _), Syntax(b, _)) => a.cmp(b),
      (Syntax(..), _) => Less,
      (_, Syntax(..)) => Greater,

      (Method(a, na), Method(b, nb)) => match a.cmp(b) {
        Equal => na.cmp(nb),
        v => v,
      },
      (Method(..), _) => Less,
      (_, Method(..)) => Greater,

      (RawCode(_, a), RawCode(_, b)) => a.cmp(b),
      (RawCode(..), _) => Less,
      (_, RawCode(..)) => Greater,

      (AnyRef(_), AnyRef(_)) => unreachable!("AnyRef should not be used in cmp"),
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
    use Calcit::*;

    match (self, other) {
      (Nil, Nil) => true,
      (Bool(a), Bool(b)) => a == b,
      (Number(a), Number(b)) => a == b,
      (Symbol { sym: a, .. }, Symbol { sym: b, .. }) => a == b,
      (Local(CalcitLocal { sym: a, .. }), Local(CalcitLocal { sym: b, .. })) => a == b,

      // special case for symbol and local, compatible with old implementation
      (Symbol { sym: a, .. }, Local(CalcitLocal { sym: b, .. })) => a == b,
      (Local(CalcitLocal { sym: a, .. }), Symbol { sym: b, .. }) => a == b,
      (Symbol { sym: a, .. }, Import(CalcitImport { def: b, .. })) => a == b,
      (Import(CalcitImport { def: b, .. }), Symbol { sym: a, .. }) => a == b,
      (Registered(a), Registered(b)) => a == b,

      (Import(CalcitImport { ns: a, def: a1, .. }), Import(CalcitImport { ns: b, def: b1, .. })) => a == b && a1 == b1,
      (Tag(a), Tag(b)) => a == b,
      (Str(a), Str(b)) => a == b,
      (Thunk(a), Thunk(b)) => a == b,
      (Ref(a, _), Ref(b, _)) => a == b,
      (Tuple(a), Tuple(b)) => a == b,
      (Buffer(b), Buffer(d)) => b == d,
      (CirruQuote(b), CirruQuote(d)) => b == d,
      (List(a), List(b)) => a == b,
      (Set(a), Set(b)) => a == b,
      (Map(a), Map(b)) => a == b,
      (Record(a), Record(b)) => a == b,
      (Proc(a), Proc(b)) => a == b,
      (Macro { id: a, .. }, Macro { id: b, .. }) => a == b,
      // functions compared with nanoid
      (Fn { id: a, .. }, Fn { id: b, .. }) => a == b,
      (Syntax(a, _), Syntax(b, _)) => a == b,
      (Method(a, b), Method(c, d)) => a == c && b == d,
      (AnyRef(a), AnyRef(b)) => a == b,
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
pub enum CalcitErrKind {
  Syntax,
  Type,
  Arity,
  Var,
  Effect,
  Unexpected,
  Unimplemented,
}

impl fmt::Display for CalcitErrKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use CalcitErrKind::*;

    f.write_str(match self {
      Syntax => "Syntax",
      Type => "Type",
      Arity => "Arity",
      Var => "Var",
      Effect => "Effect",
      Unexpected => "Unexpected",
      Unimplemented => "Unimplemented",
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitErr {
  pub kind: CalcitErrKind,
  pub msg: String,
  pub warnings: Vec<LocatedWarning>,
  pub location: Option<Arc<NodeLocation>>,
  pub stack: CallStackList,
}

impl fmt::Display for CalcitErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "[{} Error] {}", self.kind, self.msg)?;
    if let Some(location) = &self.location {
      write!(f, "\n  at {location}")?;
    }
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
      kind: CalcitErrKind::Unexpected,
      msg,
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    }
  }
}

impl CalcitErr {
  pub fn use_str<T: Into<String>>(kind: CalcitErrKind, msg: T) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    }
  }
  pub fn err_str<T: Into<String>>(kind: CalcitErrKind, msg: T) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: msg.into(),
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    })
  }
  /// display nodes in error message
  pub fn err_nodes<T: Into<String>>(kind: CalcitErrKind, msg: T, nodes: &[Calcit]) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: format!("{} {}", msg.into(), CalcitList::from(nodes)),
      warnings: vec![],
      stack: CallStackList::default(),
      location: None,
    })
  }
  pub fn err_str_location<T: Into<String>>(kind: CalcitErrKind, msg: T, location: Option<Arc<NodeLocation>>) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: msg.into(),
      warnings: vec![],
      stack: CallStackList::default(),
      location,
    })
  }
  pub fn use_msg_stack<T: Into<String>>(kind: CalcitErrKind, msg: T, stack: &CallStackList) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      warnings: vec![],
      stack: stack.to_owned(),
      location: None,
    }
  }
  pub fn use_msg_stack_location<T: Into<String>>(
    kind: CalcitErrKind,
    msg: T,
    stack: &CallStackList,
    location: Option<NodeLocation>,
  ) -> Self {
    CalcitErr {
      kind,
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
  pub coord: Arc<Vec<u16>>,
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
  pub fn new(ns: Arc<str>, def: Arc<str>, coord: Arc<Vec<u16>>) -> Self {
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
  /// (.:k a)
  KeywordAccess,
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
      MethodKind::KeywordAccess => write!(f, "keyword-access"),
      MethodKind::Access => write!(f, "access"),
      MethodKind::AccessOptional => write!(f, "access-optional"),
    }
  }
}

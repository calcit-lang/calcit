// use cirru_parser::CirruNode; // TODO for CalcitThunk
use core::cmp::Ord;
use std::cmp::Eq;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::hash::{Hash, Hasher};

// String from nanoid!
pub type NanoId = String;

// scope
pub type CalcitScope = im::HashMap<String, CalcitData>;
pub type CalcitItems = im::Vector<CalcitData>;

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolResolved {
  ResolvedLocal,
  ResolvedDef(String, String), // ns, def
}

/// special types wraps vector of calcit data for displaying
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct CrListWrap(pub im::Vector<CalcitData>);

#[derive(Debug, Clone)]
pub enum CalcitData {
  CalcitNil,
  CalcitBool(bool),
  CalcitNumber(f32),
  CalcitSymbol(String, String, Option<SymbolResolved>), // content, ns... so it has meta information
  CalcitKeyword(String),
  CalcitString(String),
  // CalcitRef(CalcitData), // TODO
  // CalcitThunk(CirruNode), // TODO
  CalcitRecur(CalcitItems), // not data, but for recursion
  CalcitList(CalcitItems),
  CalcitSet(im::HashSet<CalcitData>),
  CalcitMap(im::HashMap<CalcitData, CalcitData>),
  CalcitRecord(String, Vec<String>, Vec<CalcitData>),
  CalcitProc(String),
  CalcitMacro(
    String, // name
    String, // ns
    NanoId,
    CalcitItems, // args
    CalcitItems, // body
  ),
  CalcitFn(
    String,
    String,
    NanoId,
    CalcitScope,
    CalcitItems, // args
    CalcitItems, // body
  ),
  CalcitSyntax(String, String), // name, ns... notice that `ns` is a meta info
}

use CalcitData::*;

impl fmt::Display for CalcitData {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CalcitNil => f.write_str("nil"),
      CalcitBool(v) => f.write_str(&format!("{}", v)),
      CalcitNumber(n) => f.write_str(&format!("{}", n)),
      CalcitSymbol(s, ..) => f.write_str(&format!("'{}", s)),
      CalcitKeyword(s) => f.write_str(&format!(":{}", s)),
      CalcitString(s) => f.write_str(&format!("\"|{}\"", s)), // TODO, escaping choices
      // CalcitThunk(v) => f.write_str(&format!("{}", v)),
      CalcitRecur(xs) => {
        f.write_str("(&recur")?;
        for x in xs {
          f.write_str(&format!(" {}", x))?;
        }
        f.write_str(")")
      }
      CalcitList(xs) => {
        f.write_str("([]")?;
        for x in xs {
          f.write_str(&format!(" {}", x))?;
        }
        f.write_str(")")
      }
      CalcitSet(xs) => {
        f.write_str("(#{}")?;
        for x in xs {
          f.write_str(&format!(" {}", x))?;
        }
        f.write_str(")")
      }
      CalcitMap(xs) => {
        f.write_str("({}")?;
        for (k, v) in xs {
          f.write_str(&format!(" ({} {})", k, v))?;
        }
        f.write_str(")")?;
        Ok(())
      }
      CalcitRecord(name, fields, values) => {
        f.write_str(&format!("(%{{}} {}", name))?;
        for idx in 0..fields.len() {
          f.write_str(&format!("({} {})", fields[idx], values[idx]))?;
        }
        f.write_str(")")
      }
      CalcitProc(name) => f.write_str(&format!("(&proc {})", name)),
      CalcitMacro(name, _def_ns, _, args, body) => {
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
      CalcitFn(name, _, _, _, args, body) => {
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
      CalcitSyntax(name, _ns) => f.write_str(&format!("(&syntax {})", name)),
    }
  }
}
/// special types wraps vector of calcit data for displaying

impl fmt::Display for CrListWrap {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&format_to_lisp(&CalcitList(self.0.clone()))) // TODO performance
  }
}

/// display data into Lisp style for readability
pub fn format_to_lisp(x: &CalcitData) -> String {
  match x {
    CalcitList(ys) => {
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
    CalcitSymbol(s, ..) => s.clone(),
    a => format!("{}", a),
  }
}

impl Hash for CalcitData {
  fn hash<H>(&self, _state: &mut H)
  where
    H: Hasher,
  {
    match self {
      CalcitNil => "nil:".hash(_state),
      CalcitBool(v) => {
        "bool:".hash(_state);
        v.hash(_state);
      }
      CalcitNumber(n) => {
        "number:".hash(_state);
        // TODO https://stackoverflow.com/q/39638363/883571
        (*n as usize).hash(_state)
      }
      CalcitSymbol(s, ns, _resolved) => {
        "symbol:".hash(_state);
        s.hash(_state);
        // probaly no need, also won't be used in hashing
        // ns.hash(_state);
      }
      CalcitKeyword(s) => {
        "keyword:".hash(_state);
        s.hash(_state);
      }
      CalcitString(s) => {
        "string:".hash(_state);
        s.hash(_state);
      }
      // CalcitThunk(v) => {
      //   "quote:".hash(_state);
      //   v.hash(_state);
      // }
      CalcitRecur(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      CalcitList(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      CalcitSet(v) => {
        "set:".hash(_state);
        // TODO order for set is stable
        for x in v {
          x.hash(_state)
        }
      }
      CalcitMap(v) => {
        "map:".hash(_state);
        // TODO order for map is not stable
        for x in v {
          x.hash(_state)
        }
      }
      CalcitRecord(name, fields, values) => {
        "record:".hash(_state);
        name.hash(_state);
        fields.hash(_state);
        values.hash(_state);
      }
      CalcitProc(name) => {
        "proc:".hash(_state);
        name.hash(_state);
      }
      CalcitMacro(name, gen_id, ..) => {
        "macro:".hash(_state);
        name.hash(_state);
        gen_id.hash(_state);
      }
      CalcitFn(name, gen_id, ..) => {
        "fn:".hash(_state);
        name.hash(_state);
        gen_id.hash(_state);
      }
      CalcitSyntax(name, _ns) => {
        "syntax:".hash(_state);
        // syntax name can be used as identity
        name.hash(_state);
      }
    }
  }
}

impl Ord for CalcitData {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (CalcitNil, CalcitNil) => Equal,
      (CalcitNil, _) => Less,
      (_, CalcitNil) => Greater,

      (CalcitBool(a), CalcitBool(b)) => a.cmp(b),
      (CalcitBool(_), _) => Less,
      (_, CalcitBool(_)) => Greater,

      (CalcitNumber(a), CalcitNumber(b)) => {
        if a < b {
          Less
        } else if a > b {
          Greater
        } else {
          Equal
        }
      }
      (CalcitNumber(_), _) => Less,
      (_, CalcitNumber(_)) => Greater,

      (CalcitSymbol(a, ..), CalcitSymbol(b, ..)) => a.cmp(&b),
      (CalcitSymbol(..), _) => Less,
      (_, CalcitSymbol(..)) => Greater,

      (CalcitKeyword(a), CalcitKeyword(b)) => a.cmp(&b),
      (CalcitKeyword(_), _) => Less,
      (_, CalcitKeyword(_)) => Greater,

      (CalcitString(a), CalcitString(b)) => a.cmp(&b),
      (CalcitString(_), _) => Less,
      (_, CalcitString(_)) => Greater,

      // (CalcitThunk(a), CalcitThunk(b)) => a.cmp(b),
      // (CalcitThunk(_), _) => Less,
      // (_, CalcitThunk(_)) => Greater,
      (CalcitRecur(a), CalcitRecur(b)) => a.cmp(b),
      (CalcitRecur(_), _) => Less,
      (_, CalcitRecur(_)) => Greater,

      (CalcitList(a), CalcitList(b)) => a.cmp(b),
      (CalcitList(_), _) => Less,
      (_, CalcitList(_)) => Greater,

      (CalcitSet(a), CalcitSet(b)) => match a.len().cmp(&b.len()) {
        Equal => {
          unreachable!("TODO sets are not cmp ed") // TODO
        }
        a => a,
      },
      (CalcitSet(_), _) => Less,
      (_, CalcitSet(_)) => Greater,

      (CalcitMap(a), CalcitMap(b)) => {
        unreachable!(format!("TODO maps are not cmp ed {:?} {:?}", a, b)) // TODO
      }
      (CalcitMap(_), _) => Less,
      (_, CalcitMap(_)) => Greater,

      (CalcitRecord(_name1, _fields1, _values1), CalcitRecord(_name2, _fields2, _values2)) => {
        unreachable!("TODO records are not cmp ed") // TODO
      }
      (CalcitRecord(..), _) => Less,
      (_, CalcitRecord(..)) => Greater,

      (CalcitProc(a), CalcitProc(b)) => a.cmp(b),
      (CalcitProc(_), _) => Less,
      (_, CalcitProc(_)) => Greater,

      (CalcitMacro(_, a, ..), CalcitMacro(_, b, ..)) => a.cmp(b),
      (CalcitMacro(..), _) => Less,
      (_, CalcitMacro(..)) => Greater,

      (CalcitFn(_, a, ..), CalcitFn(_, b, ..)) => a.cmp(&b), // compared with nanoid
      (CalcitFn(..), _) => Less,
      (_, CalcitFn(..)) => Greater,

      (CalcitSyntax(a, _), CalcitSyntax(b, _)) => a.cmp(&b),
    }
  }
}

impl PartialOrd for CalcitData {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Eq for CalcitData {}

impl PartialEq for CalcitData {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (CalcitNil, CalcitNil) => true,
      (CalcitBool(a), CalcitBool(b)) => a == b,
      (CalcitNumber(a), CalcitNumber(b)) => a == b,
      (CalcitSymbol(a, ..), CalcitSymbol(b, ..)) => a == b,
      (CalcitKeyword(a), CalcitKeyword(b)) => a == b,
      (CalcitString(a), CalcitString(b)) => a == b,
      // (CalcitThunk(a), CalcitThunk(b)) => a == b,
      (CalcitList(a), CalcitList(b)) => a == b,
      (CalcitSet(a), CalcitSet(b)) => a == b,
      (CalcitMap(a), CalcitMap(b)) => a == b,
      (CalcitRecord(name1, fields1, values1), CalcitRecord(name2, fields2, values2)) => {
        name1 == name2 && fields1 == fields2 && values1 == values2
      }

      // functions compared with nanoid
      (CalcitProc(a), CalcitProc(b)) => a == b,
      (CalcitMacro(_, a, ..), CalcitMacro(_, b, ..)) => a == b,
      (CalcitFn(_, a, ..), CalcitFn(_, b, ..)) => a == b,
      (CalcitSyntax(a, _), CalcitSyntax(b, _)) => a == b,
      (_, _) => false,
    }
  }
}

pub const CORE_NS: &str = "calcit.core";
pub const GENERATED_NS: &str = "calcit.gen";

pub const CALCI_VERSION: &str = "0.0.1";

impl CalcitData {
  pub fn turn_string(&self) -> String {
    match self {
      CalcitString(s) => s.clone(),
      _ => format!("{}", self),
    }
  }
}

use cirru_parser::CirruNode;
use core::cmp::Ord;
use im;
use std::cmp::Eq;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::hash::{Hash, Hasher};

// String from nanoid!
type NanoId = String;

type CalcitScope = im::HashMap<String, CalcitData>;

type FnEvalFn = fn(CalcitData, CalcitScope) -> CalcitData;

#[derive(Debug, Clone)]
pub enum CalcitData {
  CalcitNil,
  CalcitBool(bool),
  CalcitNumber(f32),
  CalcitSymbol(String),
  CalcitKeyword(String),
  CalcitString(String),
  // CalcitRef(CalcitData), // TODO
  // CalcitThunk(CirruNode), // TODO
  CalcitList(im::Vector<CalcitData>),
  CalcitSet(im::HashSet<CalcitData>),
  CalcitMap(im::HashMap<CalcitData, CalcitData>),
  CalcitRecord(String, Vec<String>, Vec<CalcitData>),
  CalcitMacro(String, NanoId, fn(Vec<CalcitData>) -> CalcitData),
  CalcitFn(String, NanoId, fn(Vec<CalcitData>) -> CalcitData),
  CalcitSyntax(
    String,
    fn(Vec<CalcitData>, CalcitScope, FnEvalFn) -> CalcitData,
  ),
}

use CalcitData::*;

impl fmt::Display for CalcitData {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CalcitNil => f.write_str("nil"),
      CalcitBool(v) => f.write_str(&format!("{}", v)),
      CalcitNumber(n) => f.write_str(&format!("{}", n)),
      CalcitSymbol(s) => f.write_str(&format!("\"|{}\"", s)), // TODO
      CalcitKeyword(s) => f.write_str(&format!(":{}", s)),
      CalcitString(s) => f.write_str(&format!("'{}", s)),
      // CalcitThunk(v) => f.write_str(&format!("{}", v)),
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
        for (k, v) in xs {
          f.write_str(&format!("{} {}", k, v))?;
        }
        Ok(())
      }
      CalcitRecord(name, fields, values) => {
        f.write_str(&format!("(%{{}} {}", name))?;

        for idx in 0..fields.len() {
          f.write_str(&format!("({} {})", fields[idx], values[idx]))?;
        }

        f.write_str(")")
      }
      CalcitMacro(name, gen_id, _f) => f.write_str(&format!("(&macro {})", name)),
      CalcitFn(name, gen_id, _f) => f.write_str(&format!("(&fn {})", name)),
      CalcitSyntax(name, _f) => f.write_str(&format!("(&syntax {})", name)),
    }
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
        (*n as usize).hash(_state) // TODO inaccurate solution
      }
      CalcitSymbol(s) => {
        "symbol:".hash(_state);
        s.hash(_state);
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
      CalcitMacro(name, gen_id, _f) => {
        "macro:".hash(_state);
        name.hash(_state);
        gen_id.hash(_state)
      }
      CalcitFn(name, gen_id, _f) => {
        "fn:".hash(_state);
        name.hash(_state);
        gen_id.hash(_state);
      }
      CalcitSyntax(name, _f) => {
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

      (CalcitSymbol(a), CalcitSymbol(b)) => a.cmp(&b),
      (CalcitSymbol(_), _) => Less,
      (_, CalcitSymbol(_)) => Greater,

      (CalcitKeyword(a), CalcitKeyword(b)) => a.cmp(&b),
      (CalcitKeyword(_), _) => Less,
      (_, CalcitKeyword(_)) => Greater,

      (CalcitString(a), CalcitString(b)) => a.cmp(&b),
      (CalcitString(_), _) => Less,
      (_, CalcitString(_)) => Greater,

      // (CalcitThunk(a), CalcitThunk(b)) => a.cmp(b),
      // (CalcitThunk(_), _) => Less,
      // (_, CalcitThunk(_)) => Greater,
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
      (CalcitRecord(_, _, _), _) => Less,
      (_, CalcitRecord(_, _, _)) => Greater,

      (CalcitMacro(_, a, _), CalcitMacro(_, b, _)) => a.cmp(b),
      (CalcitMacro(_, _, _), _) => Less,
      (_, CalcitMacro(_, _, _)) => Greater,

      (CalcitFn(_, a, _), CalcitFn(_, b, _)) => a.cmp(&b), // compared with nanoid
      (CalcitFn(_, _, _), _) => Less,
      (_, CalcitFn(_, _, _)) => Greater,

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
      (CalcitSymbol(a), CalcitSymbol(b)) => a == b,
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
      (CalcitMacro(_, a, _), CalcitMacro(_, b, _)) => a == b,
      (CalcitFn(_, a, _), CalcitFn(_, b, _)) => a == b,
      (CalcitSyntax(a, _), CalcitSyntax(b, _)) => a == b,
      (_, _) => false,
    }
  }
}

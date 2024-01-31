use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;

use crate::{
  primes::{CalcitProc, CalcitSyntax},
  Calcit,
};

pub mod cirru;
pub mod edn;

pub fn data_to_calcit(x: &Calcit, ns: Arc<str>, at_def: Arc<str>) -> Result<Calcit, String> {
  match x {
    Calcit::Syntax(s, ns) => Ok(Calcit::Syntax(s.to_owned(), ns.to_owned())),
    Calcit::Proc(p) => Ok(Calcit::Proc(p.to_owned())),
    Calcit::Bool(b) => Ok(Calcit::Bool(*b)),
    Calcit::Number(n) => Ok(Calcit::Number(*n)),
    Calcit::Str(s) => Ok(Calcit::Str(s.to_owned())),
    Calcit::Tag(k) => Ok(Calcit::Tag(k.to_owned())),
    Calcit::CirruQuote(_) => Ok(Calcit::List(TernaryTreeList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Symbol { .. } => Ok(Calcit::List(TernaryTreeList::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Nil => Ok(Calcit::Nil),
    Calcit::Tuple(t, extra, _class) => {
      let mut ys = TernaryTreeList::from(&[Calcit::Proc(CalcitProc::NativeTuple)]);
      ys = ys.push_right(data_to_calcit(t, ns.to_owned(), at_def.to_owned())?);
      for x in extra {
        ys = ys.push_right(data_to_calcit(x, ns.to_owned(), at_def.to_owned())?);
      }
      Ok(Calcit::List(ys))
    }
    Calcit::List(xs) => {
      let mut ys = TernaryTreeList::from(&[Calcit::Proc(CalcitProc::List)]);
      for x in xs {
        ys = ys.push_right(data_to_calcit(x, ns.to_owned(), at_def.to_owned())?);
      }
      Ok(Calcit::List(ys))
    }
    Calcit::Set(xs) => {
      let mut ys = TernaryTreeList::from(&[Calcit::Proc(CalcitProc::Set)]);
      for x in xs {
        ys = ys.push_right(data_to_calcit(x, ns.to_owned(), at_def.to_owned())?);
      }
      Ok(Calcit::List(ys))
    }
    Calcit::Map(xs) => {
      let mut ys = TernaryTreeList::from(&[Calcit::Proc(CalcitProc::NativeMap)]);
      for (k, v) in xs {
        ys = ys.push_right(data_to_calcit(k, ns.to_owned(), at_def.to_owned())?);
        ys = ys.push_right(data_to_calcit(v, ns.to_owned(), at_def.to_owned())?);
      }
      Ok(Calcit::List(ys))
    }
    Calcit::Record(tag, fields, values, _class) => {
      let mut ys = TernaryTreeList::from(&[Calcit::Symbol {
        sym: "defrecord!".into(),
        info: Arc::new(crate::primes::CalcitSymbolInfo {
          ns: ns.to_owned(),
          at_def: at_def.to_owned(),
          resolved: None,
        }),
        location: None,
      }]);
      ys = ys.push_right(Calcit::Tag(tag.to_owned()));
      let size = fields.len();
      for i in 0..size {
        ys = ys.push(Calcit::List(TernaryTreeList::from(&[
          Calcit::Tag(fields[i].to_owned()),
          data_to_calcit(&values[i], ns.to_owned(), at_def.to_owned())?,
        ])))
      }
      Ok(Calcit::List(ys))
    }
    Calcit::Ref(_, _) => Err(format!("data_to_calcit not implemented for ref: {}", x)),
    Calcit::Thunk(code, _) => Ok((**code).to_owned()),
    Calcit::Buffer(_) => Err(format!("data_to_calcit not implemented for buffer: {}", x)),
    Calcit::Recur(_xs) => Err(format!("data_to_calcit not implemented for recur: {}", x)),
    Calcit::Macro { .. } => Err(format!("data_to_calcit not implemented for macro: {}", x)),
    Calcit::Fn { .. } => Err(format!("data_to_calcit not implemented for fn: {}", x)),
    Calcit::Method(..) => Ok(x.to_owned()),
    Calcit::RawCode(..) => Ok(x.to_owned()),
  }
}

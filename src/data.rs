use std::sync::Arc;

use crate::{
  calcit::{CalcitList, CalcitProc, CalcitSyntax},
  Calcit,
};

pub mod cirru;
pub mod edn;

pub fn data_to_calcit(x: &Calcit, ns: &str, at_def: &str) -> Result<Calcit, String> {
  match x {
    Calcit::Syntax(s, ns) => Ok(Calcit::Syntax(s.to_owned(), ns.to_owned())),
    Calcit::Proc(p) => Ok(Calcit::Proc(p.to_owned())),
    Calcit::Bool(b) => Ok(Calcit::Bool(*b)),
    Calcit::Number(n) => Ok(Calcit::Number(*n)),
    Calcit::Str(s) => Ok(Calcit::Str(s.to_owned())),
    Calcit::Tag(k) => Ok(Calcit::Tag(k.to_owned())),
    Calcit::CirruQuote(_) => Ok(Calcit::List(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Symbol { .. } => Ok(Calcit::List(CalcitList::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Local { .. } => Ok(Calcit::List(CalcitList::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Import { .. } => Ok(Calcit::List(CalcitList::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Registered(s) => Ok(Calcit::Registered(s.to_owned())),
    Calcit::Nil => Ok(Calcit::Nil),
    Calcit::Tuple(t, extra, _class) => {
      let mut ys = CalcitList::new_inner_from(&[Arc::new(Calcit::Proc(CalcitProc::NativeTuple))]);
      ys = ys.push_right(data_to_calcit(t, ns, at_def)?.into());
      for x in extra {
        ys = ys.push_right(data_to_calcit(x, ns, at_def)?.into());
      }
      Ok(Calcit::List(ys.into()))
    }
    Calcit::List(xs) => {
      let mut ys = CalcitList::new_inner_from(&[Arc::new(Calcit::Proc(CalcitProc::List))]);
      for x in xs {
        ys = ys.push_right(data_to_calcit(x, ns, at_def)?.into());
      }
      Ok(Calcit::List(ys.into()))
    }
    Calcit::Set(xs) => {
      let mut ys = CalcitList::new_inner_from(&[Arc::new(Calcit::Proc(CalcitProc::Set))]);
      for x in xs {
        ys = ys.push_right(data_to_calcit(x, ns, at_def)?.into());
      }
      Ok(Calcit::List(ys.into()))
    }
    Calcit::Map(xs) => {
      let mut ys = CalcitList::new_inner_from(&[Arc::new(Calcit::Proc(CalcitProc::NativeMap))]);
      for (k, v) in xs {
        ys = ys.push_right(data_to_calcit(k, ns, at_def)?.into());
        ys = ys.push_right(data_to_calcit(v, ns, at_def)?.into());
      }
      Ok(Calcit::List(ys.into()))
    }
    Calcit::Record {
      name: tag, fields, values, ..
    } => {
      let mut ys = CalcitList::new_inner_from(&[Arc::new(Calcit::Symbol {
        sym: "defrecord!".into(),
        info: Arc::new(crate::calcit::CalcitSymbolInfo {
          at_ns: Arc::from(ns),
          at_def: Arc::from(at_def),
        }),
        location: None,
      })]);
      ys = ys.push_right(Calcit::Tag(tag.to_owned()).into());
      let size = fields.len();
      for i in 0..size {
        ys = ys.push(Arc::new(Calcit::List(CalcitList::from(&[
          Calcit::Tag(fields[i].to_owned()),
          data_to_calcit(&values[i], ns, at_def)?,
        ]))))
      }
      Ok(Calcit::List(ys.into()))
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

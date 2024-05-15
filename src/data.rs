use std::sync::Arc;

use crate::{
  calcit::{CalcitList, CalcitProc, CalcitRecord, CalcitSyntax, CalcitTuple},
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
    Calcit::CirruQuote(_) => Ok(Calcit::from(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Symbol { .. } => Ok(Calcit::from(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Local { .. } => Ok(Calcit::from(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Import { .. } => Ok(Calcit::from(CalcitList::from(&[
      Calcit::Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Calcit::Registered(s) => Ok(Calcit::Registered(s.to_owned())),
    Calcit::Nil => Ok(Calcit::Nil),
    Calcit::Tuple(CalcitTuple { tag: t, extra, .. }) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::NativeTuple), data_to_calcit(t, ns, at_def)?];
      for x in extra {
        ys.push(data_to_calcit(x, ns, at_def)?);
      }
      Ok(Calcit::from(ys))
    }
    Calcit::List(xs) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::List)];
      xs.traverse_result::<String>(&mut |x| {
        ys.push(data_to_calcit(x, ns, at_def)?);
        Ok(())
      })?;
      Ok(Calcit::from(ys))
    }
    Calcit::Set(xs) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::Set)];
      for x in xs {
        ys.push(data_to_calcit(x, ns, at_def)?);
      }
      Ok(Calcit::from(ys))
    }
    Calcit::Map(xs) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::NativeMap)];
      for (k, v) in xs {
        ys.push(data_to_calcit(k, ns, at_def)?);
        ys.push(data_to_calcit(v, ns, at_def)?);
      }
      Ok(Calcit::from(ys))
    }
    Calcit::Record(CalcitRecord {
      name: tag, fields, values, ..
    }) => {
      let mut ys = vec![Calcit::Symbol {
        sym: "defrecord!".into(),
        info: Arc::new(crate::calcit::CalcitSymbolInfo {
          at_ns: Arc::from(ns),
          at_def: Arc::from(at_def),
        }),
        location: None,
      }];
      ys.push(Calcit::Tag(tag.to_owned()));
      let size = fields.len();
      for i in 0..size {
        ys.push(Calcit::from(CalcitList::from(&[
          Calcit::tag(fields[i].ref_str()),
          data_to_calcit(&values[i], ns, at_def)?,
        ])))
      }
      Ok(Calcit::from(ys))
    }
    Calcit::Ref(_, _) => Err(format!("data_to_calcit not implemented for ref: {}", x)),
    Calcit::Thunk(thunk) => Ok(thunk.get_code().to_owned()),
    Calcit::Buffer(_) => Err(format!("data_to_calcit not implemented for buffer: {}", x)),
    Calcit::Recur(_xs) => Err(format!("data_to_calcit not implemented for recur: {}", x)),
    Calcit::Macro { .. } => Err(format!("data_to_calcit not implemented for macro: {}", x)),
    Calcit::Fn { .. } => Err(format!("data_to_calcit not implemented for fn: {}", x)),
    Calcit::Method(..) => Ok(x.to_owned()),
    Calcit::RawCode(..) => Ok(x.to_owned()),
    Calcit::AnyRef(..) => unreachable!("data_to_calcit not implemented for any-ref: {}", x),
  }
}

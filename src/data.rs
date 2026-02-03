use std::sync::Arc;

use crate::{
  Calcit,
  calcit::{CalcitList, CalcitProc, CalcitRecord, CalcitStruct, CalcitSyntax, CalcitTuple},
};

pub mod cirru;
pub mod edn;

pub fn data_to_calcit(x: &Calcit, ns: &str, at_def: &str) -> Result<Calcit, String> {
  use Calcit::*;

  match x {
    Syntax(s, ns) => Ok(Calcit::Syntax(s.to_owned(), ns.to_owned())),
    Proc(p) => Ok(Calcit::Proc(p.to_owned())),
    Bool(b) => Ok(Calcit::Bool(*b)),
    Number(n) => Ok(Calcit::Number(*n)),
    Str(s) => Ok(Calcit::Str(s.to_owned())),
    Tag(k) => Ok(Calcit::Tag(k.to_owned())),
    CirruQuote(_) => Ok(Calcit::from(CalcitList::from(&[
      Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Symbol { .. } => Ok(Calcit::from(CalcitList::from(&[
      Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Local { .. } => Ok(Calcit::from(CalcitList::from(&[
      Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Import { .. } => Ok(Calcit::from(CalcitList::from(&[
      Syntax(CalcitSyntax::Quote, "quote".into()),
      x.to_owned(),
    ]))),
    Registered(s) => Ok(Calcit::Registered(s.to_owned())),
    Nil => Ok(Calcit::Nil),
    Tuple(CalcitTuple { tag: t, extra, .. }) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::NativeTuple), data_to_calcit(t, ns, at_def)?];
      for x in extra {
        ys.push(data_to_calcit(x, ns, at_def)?);
      }
      Ok(Calcit::from(ys))
    }
    List(xs) => {
      let mut ys = Vec::with_capacity(xs.len() + 1);
      ys.push(Calcit::Proc(CalcitProc::List));
      xs.traverse_result::<String>(&mut |x| {
        ys.push(data_to_calcit(x, ns, at_def)?);
        Ok(())
      })?;
      Ok(Calcit::from(ys))
    }
    Set(xs) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::Set)];
      for x in xs {
        ys.push(data_to_calcit(x, ns, at_def)?);
      }
      Ok(Calcit::from(ys))
    }
    Map(xs) => {
      let mut ys = vec![Calcit::Proc(CalcitProc::NativeMap)];
      for (k, v) in xs {
        ys.push(data_to_calcit(k, ns, at_def)?);
        ys.push(data_to_calcit(v, ns, at_def)?);
      }
      Ok(Calcit::from(ys))
    }
    Record(CalcitRecord { struct_ref, values, .. }) => {
      let mut ys = vec![Calcit::Symbol {
        sym: "defrecord!".into(),
        info: Arc::new(crate::calcit::CalcitSymbolInfo {
          at_ns: Arc::from(ns),
          at_def: Arc::from(at_def),
        }),
        location: None,
      }];
      ys.push(Calcit::Tag(struct_ref.name.to_owned()));
      let size = struct_ref.fields.len();
      for i in 0..size {
        ys.push(Calcit::from(CalcitList::from(&[
          Calcit::tag(struct_ref.fields[i].ref_str()),
          data_to_calcit(&values[i], ns, at_def)?,
        ])))
      }
      Ok(Calcit::from(ys))
    }
    Struct(CalcitStruct {
      name,
      fields,
      field_types,
      generics,
      classes,
    }) => {
      let mut ys = vec![Calcit::Symbol {
        sym: "defstruct".into(),
        info: Arc::new(crate::calcit::CalcitSymbolInfo {
          at_ns: Arc::from(ns),
          at_def: Arc::from(at_def),
        }),
        location: None,
      }];
      ys.push(Calcit::Tag(name.to_owned()));
      if !generics.is_empty() {
        let items = generics
          .iter()
          .map(|name| {
            Calcit::from(CalcitList::from(&[
              Calcit::Syntax(CalcitSyntax::Quote, Arc::from("quote")),
              Calcit::Symbol {
                sym: name.to_owned(),
                info: Arc::new(crate::calcit::CalcitSymbolInfo {
                  at_ns: Arc::from(ns),
                  at_def: Arc::from(at_def),
                }),
                location: None,
              },
            ]))
          })
          .collect::<Vec<_>>();
        ys.push(Calcit::from(CalcitList::from(items.as_slice())));
      }
      for (field, field_type) in fields.iter().zip(field_types.iter()) {
        ys.push(Calcit::from(CalcitList::from(&[
          Calcit::tag(field.ref_str()),
          field_type.to_calcit(),
        ])));
      }
      let struct_value = Calcit::from(ys);
      if let Some(class_record) = classes.first() {
        Ok(Calcit::from(CalcitList::from(&[
          Calcit::Proc(CalcitProc::NativeStructWithClass),
          struct_value,
          Calcit::Record((**class_record).clone()),
        ])))
      } else {
        Ok(struct_value)
      }
    }
    Enum(enum_def) => {
      let proto = enum_def.prototype();
      let mut ys = vec![Calcit::Symbol {
        sym: "defenum".into(),
        info: Arc::new(crate::calcit::CalcitSymbolInfo {
          at_ns: Arc::from(ns),
          at_def: Arc::from(at_def),
        }),
        location: None,
      }];
      ys.push(Calcit::Tag(proto.struct_ref.name.to_owned()));
      for (field, value) in proto.struct_ref.fields.iter().zip(proto.values.iter()) {
        if let Calcit::List(payloads) = value {
          let mut variant = vec![Calcit::tag(field.ref_str())];
          payloads.traverse(&mut |payload| {
            variant.push(payload.to_owned());
          });
          ys.push(Calcit::from(CalcitList::from(&variant[..])));
        } else if matches!(value, Calcit::Nil) {
          let variant = [Calcit::tag(field.ref_str())];
          ys.push(Calcit::from(CalcitList::from(&variant[..])));
        } else {
          ys.push(Calcit::from(CalcitList::from(&[
            Calcit::tag(field.ref_str()),
            data_to_calcit(value, ns, at_def)?,
          ])));
        }
      }
      let enum_value = Calcit::from(ys);
      if let Some(class_record) = enum_def.classes().first() {
        Ok(Calcit::from(CalcitList::from(&[
          Calcit::Proc(CalcitProc::NativeEnumWithClass),
          enum_value,
          Calcit::Record((**class_record).clone()),
        ])))
      } else {
        Ok(enum_value)
      }
    }
    Ref(_, _) => Err(format!("data_to_calcit not implemented for ref: {x}")),
    Thunk(thunk) => Ok(thunk.get_code().to_owned()),
    Buffer(_) => Err(format!("data_to_calcit not implemented for buffer: {x}")),
    Recur(_xs) => Err(format!("data_to_calcit not implemented for recur: {x}")),
    Macro { .. } => Err(format!("data_to_calcit not implemented for macro: {x}")),
    Fn { .. } => Err(format!("data_to_calcit not implemented for fn: {x}")),
    Method(..) => Ok(x.to_owned()),
    RawCode(..) => Ok(x.to_owned()),
    Trait(_) => Err(format!("data_to_calcit not implemented for trait: {x}")),
    AnyRef(..) => Err(format!("data_to_calcit not implemented for any-ref: {x}")),
  }
}

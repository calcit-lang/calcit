use std::sync::Arc;

use crate::calcit::{self, CalcitList, CalcitTuple};
use crate::calcit::{Calcit, CalcitRecord};
use crate::{calcit::MethodKind, data::cirru};

use cirru_edn::{Edn, EdnListView, EdnMapView, EdnRecordView, EdnSetView, EdnTag, EdnTupleView};

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Result<Edn, String> {
  match x {
    Calcit::Nil => Ok(Edn::Nil),
    Calcit::Bool(b) => Ok(Edn::Bool(*b)),
    Calcit::Str(s) => Ok(Edn::Str((**s).into())),
    Calcit::Number(n) => Ok(Edn::Number(*n)),
    Calcit::Tag(s) => Ok(Edn::Tag(s.to_owned())),
    Calcit::Symbol { sym, .. } => Ok(Edn::Symbol((**sym).into())),
    Calcit::List(xs) => {
      let mut ys = EdnListView::default();
      for x in xs {
        ys.push(calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Calcit::Set(xs) => {
      let mut ys = EdnSetView::default();
      for x in xs {
        ys.insert(calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Calcit::Map(xs) => {
      let mut ys = EdnMapView::default();
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k)?, calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Calcit::Record(CalcitRecord { name, fields, values, .. }) => {
      let mut entries = EdnRecordView::new(name.to_owned());
      for idx in 0..fields.len() {
        entries.insert(fields[idx].to_owned(), calcit_to_edn(&values[idx])?);
      }
      Ok(entries.into())
    }
    Calcit::Fn { info, .. } => {
      let def_ns = &info.def_ns;
      let name = &info.name;
      let args = &info.args;
      println!("[Warn] fn to EDN: {def_ns}/{name} {args:?}");
      Ok(Edn::str(x.to_string()))
    }
    Calcit::Proc(name) => Ok(Edn::Symbol(name.as_ref().into())),
    Calcit::Syntax(name, _ns) => Ok(Edn::sym(name.as_ref())),
    Calcit::Tuple(CalcitTuple { tag, extra, .. }) => {
      match &**tag {
        Calcit::Symbol { sym, .. } => {
          if &**sym == "quote" {
            let data = extra.first().ok_or(format!("quote expected 1 argument, got: {:?}", extra))?; // TODO more types to handle
            match cirru::calcit_data_to_cirru(data) {
              Ok(v) => Ok(Edn::Quote(v)),
              Err(e) => Err(format!("failed to create quote: {e}")), // TODO more types to handle
            }
          } else {
            Err(format!("unknown tag for EDN: {sym}")) // TODO more types to handle
          }
        }
        Calcit::Record(CalcitRecord { name, .. }) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          Ok(Edn::tuple(Edn::Tag(name.to_owned()), extra_values))
        }
        Calcit::Tag(tag) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          Ok(Edn::tuple(Edn::Tag(tag.to_owned()), extra_values))
        }
        v => {
          Err(format!("EDN tuple expected 'quote or record, unknown tag: {v}"))
          // TODO more types to handle
        }
      }
    }
    Calcit::Buffer(buf) => Ok(Edn::Buffer(buf.to_owned())),
    Calcit::CirruQuote(code) => Ok(Edn::Quote(code.to_owned())),
    Calcit::Method(name, kind) => match kind {
      MethodKind::Access => Ok(Edn::Symbol(format!(".-{name}").into())),
      MethodKind::InvokeNative => Ok(Edn::Symbol(format!(".!{name}").into())),
      MethodKind::Invoke => Ok(Edn::Symbol(format!(".{name}").into())),
      MethodKind::AccessOptional => Ok(Edn::Symbol(format!(".?-{name}").into())),
      MethodKind::InvokeNativeOptional => Ok(Edn::Symbol(format!(".?!{name}").into())),
    },
    a => Err(format!("not able to generate EDN: {a}")), // TODO more types to handle
  }
}

pub fn edn_to_calcit(x: &Edn, options: &Calcit) -> Calcit {
  match x {
    Edn::Nil => Calcit::Nil,
    Edn::Bool(b) => Calcit::Bool(*b),
    Edn::Number(n) => Calcit::Number(*n),
    Edn::Symbol(s) => Calcit::Symbol {
      sym: (**s).into(),
      info: Arc::new(crate::calcit::CalcitSymbolInfo {
        at_ns: calcit::GEN_NS.into(),
        at_def: calcit::GENERATED_DEF.into(),
      }),
      location: None,
    },
    Edn::Tag(s) => Calcit::Tag(s.to_owned()),
    Edn::Str(s) => Calcit::Str((**s).into()),
    Edn::Quote(nodes) => Calcit::CirruQuote(nodes.to_owned()),
    Edn::Tuple(EdnTupleView { tag, extra }) => Calcit::Tuple(CalcitTuple {
      tag: Arc::new(edn_to_calcit(tag, options)),
      extra: extra.iter().map(|x| edn_to_calcit(x, options)).collect(),
      class: None,
    }),
    Edn::List(EdnListView(xs)) => {
      let mut ys = CalcitList::new_inner();
      for x in xs {
        ys = ys.push_right(Arc::new(edn_to_calcit(x, options)))
      }
      Calcit::List(CalcitList(ys))
    }
    Edn::Set(EdnSetView(xs)) => {
      let mut ys: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for x in xs {
        ys.insert_mut(edn_to_calcit(x, options));
      }
      Calcit::Set(ys)
    }
    Edn::Map(EdnMapView(xs)) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for (k, v) in xs {
        ys.insert_mut(edn_to_calcit(k, options), edn_to_calcit(v, options));
      }
      Calcit::Map(ys)
    }
    Edn::Record(EdnRecordView { tag: name, pairs: entries }) => {
      let mut fields: Vec<EdnTag> = Vec::with_capacity(entries.len());
      let mut values: Vec<Calcit> = Vec::with_capacity(entries.len());
      let mut sorted = entries.to_owned();
      sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
      for v in sorted {
        fields.push(v.0.to_owned());
        values.push(edn_to_calcit(&v.1, options));
      }

      match find_record_in_options(&name.arc_str(), options) {
        Some(Calcit::Record(CalcitRecord {
          name: pre_name,
          fields: pre_fields,
          values: pre_values,
          class: pre_class,
        })) => {
          if fields == **pre_fields {
            Calcit::Record(CalcitRecord {
              name: pre_name.to_owned(),
              fields: pre_fields.clone(),
              values: pre_values.clone(),
              class: pre_class.clone(),
            })
          } else {
            unreachable!("record fields mismatch: {:?} vs {:?}", fields, pre_fields)
          }
        }
        _ => Calcit::Record(CalcitRecord {
          name: name.to_owned(),
          fields: Arc::new(fields),
          values: Arc::new(values),
          class: None,
        }),
      }
    }
    Edn::Buffer(buf) => Calcit::Buffer(buf.to_owned()),
  }
}
/// find a record field in options
fn find_record_in_options<'a>(name: &str, options: &'a Calcit) -> Option<&'a Calcit> {
  match options {
    Calcit::Map(ys) => ys.get(&Calcit::Tag(name.into())),
    _ => None,
  }
}

use std::sync::Arc;

use crate::calcit::{self, CalcitEnum, CalcitImport, CalcitList, CalcitLocal, CalcitStruct, CalcitTuple};
use crate::calcit::{Calcit, CalcitRecord};
use crate::{calcit::MethodKind, data::cirru};

use cirru_edn::{Edn, EdnListView, EdnMapView, EdnRecordView, EdnSetView, EdnTag, EdnTupleView};

// values does not fit are just represented with specical indicates
pub fn calcit_to_edn(x: &Calcit) -> Result<Edn, String> {
  use Calcit::*;
  match x {
    Nil => Ok(Edn::Nil),
    Bool(b) => Ok(Edn::Bool(*b)),
    Str(s) => Ok(Edn::Str((**s).into())),
    Number(n) => Ok(Edn::Number(*n)),
    Tag(s) => Ok(Edn::Tag(s.to_owned())),
    Symbol { sym, .. } => Ok(Edn::Symbol((**sym).into())),
    Local(CalcitLocal { sym, .. }) => Ok(Edn::Symbol((**sym).into())),
    Import(CalcitImport { def, .. }) => Ok(Edn::Symbol((**def).into())),
    Registered(def) => Ok(Edn::Symbol((**def).into())),
    List(xs) => {
      let mut ys = EdnListView::default();
      xs.traverse_result::<String>(&mut |x| {
        ys.push(calcit_to_edn(x)?);
        Ok(())
      })?;
      Ok(ys.into())
    }
    Set(xs) => {
      let mut ys = EdnSetView::default();
      for x in xs {
        ys.insert(calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Map(xs) => {
      let mut ys = EdnMapView::default();
      for (k, x) in xs {
        ys.insert(calcit_to_edn(k)?, calcit_to_edn(x)?);
      }
      Ok(ys.into())
    }
    Record(CalcitRecord { struct_ref, values, .. }) => {
      let mut entries = EdnRecordView::new(struct_ref.name.to_owned());
      for idx in 0..struct_ref.fields.len() {
        entries.insert(struct_ref.fields[idx].to_owned(), calcit_to_edn(&values[idx])?);
      }
      Ok(entries.into())
    }
    Fn { info, .. } => {
      let def_ns = &info.def_ns;
      let name = &info.name;
      let args = &info.args;
      eprintln!("[Warn] fn to EDN: {def_ns}/{name} {args:?}");
      Ok(Edn::str(x.to_string()))
    }
    Proc(name) => Ok(Edn::Symbol(name.as_ref().into())),
    Syntax(name, _ns) => Ok(Edn::sym(name.as_ref())),
    Tuple(CalcitTuple { tag, extra, sum_type, .. }) => {
      let enum_tag = sum_type.as_ref().map(|enum_def| Edn::Tag(enum_def.name().to_owned()));
      match &**tag {
        Symbol { sym, .. } => {
          if &**sym == "quote" {
            let data = extra.first().ok_or(format!("quote expected 1 argument, got: {extra:?}"))?; // TODO more types to handle
            match cirru::calcit_data_to_cirru(data) {
              Ok(v) => Ok(Edn::Quote(v)),
              Err(e) => Err(format!("failed to create quote: {e}")), // TODO more types to handle
            }
          } else {
            Err(format!("unknown tag for EDN: {sym}")) // TODO more types to handle
          }
        }
        Record(CalcitRecord { struct_ref, .. }) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          let tag_value = Edn::Tag(struct_ref.name.to_owned());
          Ok(match enum_tag.clone() {
            Some(enum_tag) => Edn::enum_tuple(enum_tag, tag_value, extra_values),
            None => Edn::tuple(tag_value, extra_values),
          })
        }
        Tag(tag) => {
          let mut extra_values = vec![];
          for item in extra {
            extra_values.push(calcit_to_edn(item)?);
          }
          let tag_value = Edn::Tag(tag.to_owned());
          Ok(match enum_tag.clone() {
            Some(enum_tag) => Edn::enum_tuple(enum_tag, tag_value, extra_values),
            None => Edn::tuple(tag_value, extra_values),
          })
        }
        v => {
          Err(format!("EDN tuple expected 'quote or record, unknown tag: {v}"))
          // TODO more types to handle
        }
      }
    }
    Buffer(buf) => Ok(Edn::Buffer(buf.to_owned())),
    CirruQuote(code) => Ok(Edn::Quote(code.to_owned())),
    Method(name, kind) => match kind {
      MethodKind::Access => Ok(Edn::Symbol(format!(".-{name}").into())),
      MethodKind::InvokeNative => Ok(Edn::Symbol(format!(".!{name}").into())),
      MethodKind::Invoke(_) => Ok(Edn::Symbol(format!(".{name}").into())),
      MethodKind::TagAccess => Ok(Edn::Symbol(format!(".:{name}").into())),
      MethodKind::AccessOptional => Ok(Edn::Symbol(format!(".?-{name}").into())),
      MethodKind::InvokeNativeOptional => Ok(Edn::Symbol(format!(".?!{name}").into())),
    },
    AnyRef(r) => Ok(Edn::AnyRef(r.to_owned())),
    Ref(_p, pair) => {
      let pair = pair.lock().expect("read ref");
      Ok(Edn::Atom(Box::new(calcit_to_edn(&pair.0)?)))
    }
    a => Err(format!("not able to generate EDN: {a:?}")), // TODO more types to handle
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
    Edn::Tuple(EdnTupleView { tag, enum_tag, extra }) => {
      let sum_type = enum_tag.as_ref().and_then(|enum_tag| resolve_enum_tag(enum_tag, options));
      Calcit::Tuple(CalcitTuple {
        tag: Arc::new(edn_to_calcit(tag, options)),
        extra: extra.iter().map(|x| edn_to_calcit(x, options)).collect(),
        impls: vec![],
        sum_type,
      })
    }
    Edn::List(EdnListView(xs)) => {
      let mut ys: Vec<Calcit> = vec![];
      for x in xs {
        ys.push(edn_to_calcit(x, options))
      }
      Calcit::from(CalcitList::Vector(ys))
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
          struct_ref: pre_struct,
          values: pre_values,
          impls: pre_impls,
        })) => {
          if fields == **pre_struct.fields {
            Calcit::Record(CalcitRecord {
              struct_ref: pre_struct.to_owned(),
              values: pre_values.to_owned(),
              impls: pre_impls.clone(),
            })
          } else {
            unreachable!("record fields mismatch: {:?} vs {:?}", fields, pre_struct.fields)
          }
        }
        _ => Calcit::Record(CalcitRecord {
          struct_ref: Arc::new(CalcitStruct::from_fields(name.to_owned(), fields)),
          values: Arc::new(values),
          impls: vec![],
        }),
      }
    }
    Edn::Buffer(buf) => Calcit::Buffer(buf.to_owned()),
    Edn::AnyRef(r) => Calcit::AnyRef(r.to_owned()),
    Edn::Atom(a) => crate::builtins::quick_build_atom(edn_to_calcit(a, options)),
  }
}
/// find a record field in options
fn find_record_in_options<'a>(name: &str, options: &'a Calcit) -> Option<&'a Calcit> {
  match options {
    Calcit::Map(ys) => ys.get(&Calcit::Tag(name.into())),
    _ => None,
  }
}

fn find_enum_in_options<'a>(name: &str, options: &'a Calcit) -> Option<&'a Calcit> {
  match options {
    Calcit::Map(ys) => ys.get(&Calcit::Tag(name.into())),
    _ => None,
  }
}

fn resolve_enum_tag(enum_tag: &Edn, options: &Calcit) -> Option<Arc<CalcitEnum>> {
  let enum_name = match enum_tag {
    Edn::Tag(tag) => tag.ref_str(),
    Edn::Symbol(sym) => sym.as_ref(),
    Edn::Str(s) => s.as_ref(),
    _ => return None,
  };
  match find_enum_in_options(enum_name, options) {
    Some(Calcit::Enum(enum_def)) => Some(Arc::new(enum_def.to_owned())),
    _ => None,
  }
}

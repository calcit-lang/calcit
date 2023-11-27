use std::sync::Arc;

use im_ternary_tree::TernaryTreeList;

use crate::primes;
use crate::primes::Calcit;
use crate::{data::cirru, primes::MethodKind};

use cirru_edn::{Edn, EdnListView, EdnMapView, EdnRecordView, EdnSetView, EdnTag};

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
    Calcit::Record(name, fields, values, _class) => {
      let mut entries = EdnRecordView::new(name.to_owned());
      for idx in 0..fields.len() {
        entries.insert(fields[idx].to_owned(), calcit_to_edn(&values[idx])?);
      }
      Ok(entries.into())
    }
    Calcit::Fn { name, def_ns, args, .. } => {
      println!("[Warn] fn to EDN: {def_ns}/{name} {args:?}");
      Ok(Edn::str(x.to_string()))
    }
    Calcit::Proc(name) => Ok(Edn::Symbol(name.to_string().into())),
    Calcit::Syntax(name, _ns) => Ok(Edn::sym(name.to_string())),
    Calcit::Tuple(tag, extra, _class) => {
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
        Calcit::Record(name, _, _, _) => {
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
      ns: primes::GEN_NS.into(),
      at_def: primes::GENERATED_DEF.into(),
      resolved: None,
      location: None,
    },
    Edn::Tag(s) => Calcit::Tag(s.to_owned()),
    Edn::Str(s) => Calcit::Str((**s).into()),
    Edn::Quote(nodes) => Calcit::CirruQuote(nodes.to_owned()),
    Edn::Tuple(tag, extra) => {
      let base_class = Calcit::Record(
        EdnTag::new("base"),
        Arc::new(Vec::new()),
        Arc::new(Vec::new()),
        Arc::new(Calcit::Nil),
      );
      Calcit::Tuple(
        Arc::new(edn_to_calcit(tag, options)),
        extra.iter().map(|x| edn_to_calcit(x, options)).collect(),
        Arc::new(base_class),
      )
    }
    Edn::List(xs) => {
      let mut ys: primes::CalcitItems = TernaryTreeList::Empty;
      for x in xs {
        ys = ys.push_right(edn_to_calcit(x, options))
      }
      Calcit::List(ys)
    }
    Edn::Set(xs) => {
      let mut ys: rpds::HashTrieSetSync<Calcit> = rpds::HashTrieSet::new_sync();
      for x in xs {
        ys.insert_mut(edn_to_calcit(x, options));
      }
      Calcit::Set(ys)
    }
    Edn::Map(xs) => {
      let mut ys: rpds::HashTrieMapSync<Calcit, Calcit> = rpds::HashTrieMap::new_sync();
      for (k, v) in xs {
        ys.insert_mut(edn_to_calcit(k, options), edn_to_calcit(v, options));
      }
      Calcit::Map(ys)
    }
    Edn::Record(name, entries) => {
      let mut fields: Vec<EdnTag> = Vec::with_capacity(entries.len());
      let mut values: Vec<Calcit> = Vec::with_capacity(entries.len());
      let mut sorted = entries.to_owned();
      sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
      for v in sorted {
        fields.push(v.0.to_owned());
        values.push(edn_to_calcit(&v.1, options));
      }

      match find_record_in_options(&name.to_str(), options) {
        Some(Calcit::Record(pre_name, pre_fields, pre_values, pre_class)) => {
          if fields == *pre_fields {
            Calcit::Record(pre_name, pre_fields, pre_values, pre_class)
          } else {
            unreachable!("record fields mismatch: {:?} vs {:?}", fields, pre_fields)
          }
        }
        _ => Calcit::Record(name.to_owned(), Arc::new(fields), Arc::new(values), Arc::new(Calcit::Nil)),
      }
    }
    Edn::Buffer(buf) => Calcit::Buffer(buf.to_owned()),
  }
}
/// find a record field in options
fn find_record_in_options(name: &str, options: &Calcit) -> Option<Calcit> {
  match options {
    Calcit::Map(ys) => ys.get(&Calcit::Tag(name.into())).map(ToOwned::to_owned),
    _ => None,
  }
}

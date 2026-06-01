use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::sync::Arc;

use cirru_edn::EdnAnyRef;

use super::{Calcit, CalcitEnum, CalcitFn, CalcitImpl, CalcitRecord, CalcitStruct, CalcitTrait};

pub(super) fn compare_calcit_trait_values(a: &CalcitTrait, b: &CalcitTrait) -> Ordering {
  match a.name.cmp(&b.name) {
    Equal => match a.methods.cmp(&b.methods) {
      Equal => match a.method_types.cmp(&b.method_types) {
        Equal => match compare_trait_requires(&a.requires, &b.requires) {
          Equal => compare_trait_defaults(&a.defaults, &b.defaults),
          ord => ord,
        },
        ord => ord,
      },
      ord => ord,
    },
    ord => ord,
  }
}

fn compare_trait_requires(a: &Arc<Vec<Arc<CalcitTrait>>>, b: &Arc<Vec<Arc<CalcitTrait>>>) -> Ordering {
  match a.len().cmp(&b.len()) {
    Equal => {
      for (left, right) in a.iter().zip(b.iter()) {
        match compare_calcit_trait_values(left.as_ref(), right.as_ref()) {
          Equal => continue,
          ord => return ord,
        }
      }
      Equal
    }
    ord => ord,
  }
}

fn compare_trait_defaults(a: &Arc<Vec<Option<Arc<CalcitFn>>>>, b: &Arc<Vec<Option<Arc<CalcitFn>>>>) -> Ordering {
  match a.len().cmp(&b.len()) {
    Equal => {
      for (left, right) in a.iter().zip(b.iter()) {
        match compare_trait_default_impl(left, right) {
          Equal => continue,
          ord => return ord,
        }
      }
      Equal
    }
    ord => ord,
  }
}

fn compare_trait_default_impl(left: &Option<Arc<CalcitFn>>, right: &Option<Arc<CalcitFn>>) -> Ordering {
  match (left, right) {
    (None, None) => Equal,
    (None, Some(_)) => Less,
    (Some(_), None) => Greater,
    (Some(left_fn), Some(right_fn)) => match (left_fn.def_ref.as_ref(), right_fn.def_ref.as_ref()) {
      (Some(a), Some(b)) => a
        .def_ns
        .cmp(&b.def_ns)
        .then(a.def_name.cmp(&b.def_name))
        .then(a.coord.cmp(&b.coord))
        .then(a.is_defn.cmp(&b.is_defn))
        .then(a.is_macro_gen.cmp(&b.is_macro_gen)),
      (Some(_), None) => Greater,
      (None, Some(_)) => Less,
      (None, None) => left_fn
        .name
        .cmp(&right_fn.name)
        .then(left_fn.def_ns.cmp(&right_fn.def_ns))
        .then(left_fn.args.as_ref().param_len().cmp(&right_fn.args.as_ref().param_len()))
        .then(left_fn.body.len().cmp(&right_fn.body.len())),
    },
  }
}

pub(super) fn compare_calcit_impl_values(a: &CalcitImpl, b: &CalcitImpl) -> Ordering {
  a.name
    .cmp(&b.name)
    .then(compare_impl_origin(a.origin.as_ref(), b.origin.as_ref()))
    .then(a.fields.cmp(&b.fields))
    .then(a.values.cmp(&b.values))
}

fn compare_impl_origin(a: Option<&Arc<CalcitTrait>>, b: Option<&Arc<CalcitTrait>>) -> Ordering {
  match (a, b) {
    (None, None) => Equal,
    (None, Some(_)) => Less,
    (Some(_), None) => Greater,
    (Some(left), Some(right)) => compare_calcit_trait_values(left.as_ref(), right.as_ref()),
  }
}

pub(super) fn compare_calcit_struct_values(a: &CalcitStruct, b: &CalcitStruct) -> Ordering {
  match a.name.cmp(&b.name) {
    Equal => match a.fields.cmp(&b.fields) {
      Equal => match a.field_types.cmp(&b.field_types) {
        Equal => match a.generics.cmp(&b.generics) {
          Equal => match a.where_bounds.cmp(&b.where_bounds) {
            Equal => compare_struct_impls(&a.impls, &b.impls),
            ord => ord,
          },
          ord => ord,
        },
        ord => ord,
      },
      ord => ord,
    },
    ord => ord,
  }
}

fn compare_struct_impls(a: &[Arc<CalcitImpl>], b: &[Arc<CalcitImpl>]) -> Ordering {
  match a.len().cmp(&b.len()) {
    Equal => {
      for (left, right) in a.iter().zip(b.iter()) {
        match compare_calcit_impl_values(left.as_ref(), right.as_ref()) {
          Equal => continue,
          ord => return ord,
        }
      }
      Equal
    }
    ord => ord,
  }
}

pub(super) fn compare_calcit_enum_values(a: &CalcitEnum, b: &CalcitEnum) -> Ordering {
  match a.name().cmp(b.name()) {
    Equal => a
      .generics()
      .cmp(b.generics())
      .then_with(|| a.where_bounds().cmp(b.where_bounds()))
      .then_with(|| {
        let av = a.variants();
        let bv = b.variants();
        av.len().cmp(&bv.len()).then_with(|| {
          for (va, vb) in av.iter().zip(bv.iter()) {
            let tag_ord = va.tag.cmp(&vb.tag);
            if tag_ord != Equal {
              return tag_ord;
            }
            let pa = va.payload_types();
            let pb = vb.payload_types();
            let len_ord = pa.len().cmp(&pb.len());
            if len_ord != Equal {
              return len_ord;
            }
            for (ta, tb) in pa.iter().zip(pb.iter()) {
              let t_ord = ta.to_calcit().cmp(&tb.to_calcit());
              if t_ord != Equal {
                return t_ord;
              }
            }
          }
          Equal
        })
      }),
    ord => ord,
  }
}

pub(super) fn compare_any_ref_values(a: &EdnAnyRef, b: &EdnAnyRef) -> Ordering {
  if a == b {
    Equal
  } else {
    format!("{:p}", Arc::as_ptr(&a.0)).cmp(&format!("{:p}", Arc::as_ptr(&b.0)))
  }
}

pub(super) fn compare_set_values(a: &rpds::HashTrieSetSync<Calcit>, b: &rpds::HashTrieSetSync<Calcit>) -> Ordering {
  match a.size().cmp(&b.size()) {
    Equal => {
      let mut left: Vec<&Calcit> = a.iter().collect();
      let mut right: Vec<&Calcit> = b.iter().collect();
      left.sort_unstable();
      right.sort_unstable();
      left.cmp(&right)
    }
    ord => ord,
  }
}

pub(super) fn compare_map_values(a: &rpds::HashTrieMapSync<Calcit, Calcit>, b: &rpds::HashTrieMapSync<Calcit, Calcit>) -> Ordering {
  match a.size().cmp(&b.size()) {
    Equal => {
      let mut left: Vec<(&Calcit, &Calcit)> = a.iter().collect();
      let mut right: Vec<(&Calcit, &Calcit)> = b.iter().collect();
      let sort_pair = |(ka, va): &(&Calcit, &Calcit), (kb, vb): &(&Calcit, &Calcit)| ka.cmp(kb).then(va.cmp(vb));
      left.sort_unstable_by(sort_pair);
      right.sort_unstable_by(sort_pair);
      left.cmp(&right)
    }
    ord => ord,
  }
}

pub(super) fn compare_record_values(a: &CalcitRecord, b: &CalcitRecord) -> Ordering {
  match a.struct_ref.name.cmp(&b.struct_ref.name) {
    Equal => match a.struct_ref.fields.cmp(&b.struct_ref.fields) {
      Equal => a.values.cmp(&b.values),
      ord => ord,
    },
    ord => ord,
  }
}

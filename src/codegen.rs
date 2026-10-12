use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use cirru_edn::EdnTag;

use crate::calcit::{self, Calcit, CalcitImport, CalcitLocal};

pub mod emit_js;
#[cfg(feature = "wasm")]
#[path = "wasm/emit_wasm.rs"]
pub mod emit_wasm;
pub mod gen_ir;

/// switch whether in codegen mode
static CODEGEN_MODE: AtomicBool = AtomicBool::new(true);

static CODEGEN_SKIP_ARITY_CHECK: AtomicBool = AtomicBool::new(false);

pub const COMPILE_ERRORS_FILE: &str = "calcit.build-errors";

pub fn codegen_mode() -> bool {
  CODEGEN_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// defaults to `true``
pub fn set_codegen_mode(b: bool) {
  CODEGEN_MODE.store(b, std::sync::atomic::Ordering::Relaxed);
}

/// whether to disable arity check in js codegen
pub fn set_code_gen_skip_arity_check(b: bool) {
  if b {
    println!("WARN: skip arity check in js codegen")
  }
  CODEGEN_SKIP_ARITY_CHECK.store(b, std::sync::atomic::Ordering::Relaxed)
}

/// read global flag for skipping arity check
pub fn skip_arity_check() -> bool {
  CODEGEN_SKIP_ARITY_CHECK.load(std::sync::atomic::Ordering::Relaxed)
}

/// A `match (destruct-list xs)` whose branches are exactly `(:none)` and
/// `(:some head tail)`, in either source order.
///
/// Backends may lower this shape to a direct length check plus first/rest on
/// `xs`: `destruct-list` only builds a `ListDestruct` that the match takes apart
/// right away, so skipping the intermediate enum keeps evaluation order, values
/// and failures unchanged. Every other shape keeps the generic `match` lowering.
pub(crate) struct DestructListMatch<'a> {
  /// The local holding the matched List; evaluating it has no effects.
  pub subject: &'a Calcit,
  pub none_body: &'a Calcit,
  /// Head binder, `None` for `_`.
  pub head: Option<&'a Arc<str>>,
  /// Tail binder, `None` for `_`.
  pub tail: Option<&'a Arc<str>>,
  pub some_body: &'a Calcit,
}

/// Recognizes the preprocessed indexed form `match value EnumDef table`
/// (`args` excludes the leading `match` syntax node).
pub(crate) fn destruct_list_match_shape(args: &[Calcit]) -> Option<DestructListMatch<'_>> {
  let [value, Calcit::EnumDef(enum_def), Calcit::List(table)] = args else {
    return None;
  };
  if enum_def.name().ref_str() != "ListDestruct" || enum_def.definition_ref().map(|r| r.as_ref()) != Some("calcit.core/ListDestruct") {
    return None;
  }

  let Calcit::List(call) = value else { return None };
  let (Some(Calcit::Import(CalcitImport { ns, def, .. })), Some(subject @ Calcit::Local(CalcitLocal { sym: subject_sym, .. })), 2) =
    (call.first(), call.get(1), call.len())
  else {
    return None;
  };
  if &**ns != calcit::CORE_NS || &**def != "destruct-list" {
    return None;
  }

  // A trailing wildcard would only catch arity mismatches; keep such matches generic.
  let none_idx = enum_def.variant_index(&EdnTag::from("none"))?;
  let some_idx = enum_def.variant_index(&EdnTag::from("some"))?;
  if table.len() != 3 || none_idx == some_idx || !matches!(table.get(2), Some(Calcit::Nil)) {
    return None;
  }

  let branch = |idx: usize, arity: usize| -> Option<(Vec<Option<&Arc<str>>>, &Calcit)> {
    let Calcit::List(pair) = table.get(idx)? else { return None };
    let (Some(Calcit::List(pattern)), Some(body), 2) = (pair.first(), pair.get(1), pair.len()) else {
      return None;
    };
    if pattern.len() != arity {
      return None;
    }
    let mut binders = Vec::with_capacity(arity - 1);
    for binder in pattern.iter().skip(1) {
      let (Calcit::Local(CalcitLocal { sym, .. }) | Calcit::Symbol { sym, .. }) = binder else {
        return None;
      };
      binders.push(if sym.as_ref() == "_" { None } else { Some(sym) });
    }
    Some((binders, body))
  };

  let (_, none_body) = branch(none_idx, 1)?;
  let (some_binders, some_body) = branch(some_idx, 3)?;
  let (head, tail) = (some_binders[0], some_binders[1]);
  // Binders that shadow the subject or each other need the generic path, which
  // reads every payload from the enum before binding. Backends that rename
  // locals must repeat this check on their own names (see the JS emitter).
  if head.is_some_and(|h| h == subject_sym) || tail.is_some_and(|t| t == subject_sym) || (head.is_some() && head == tail) {
    return None;
  }

  Some(DestructListMatch {
    subject,
    none_body,
    head,
    tail,
    some_body,
  })
}

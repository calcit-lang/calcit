mod calcit_impl;
mod calcit_struct;
mod calcit_trait;
mod compare;
mod fns;
mod list;
mod local;
mod proc_name;
mod record;
mod sum_type;
mod symbol;
mod syntax_name;
mod thunk;
mod tuple;
pub(crate) mod type_annotation;

use core::cmp::Ord;
use std::cmp::Eq;
use std::cmp::Ordering;
use std::cmp::Ordering::*;
use std::fmt;
use std::fmt::Display;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use cirru_edn::EdnAnyRef;
use cirru_edn::{Edn, EdnTag};
use cirru_parser::Cirru;
use im_ternary_tree::TernaryTreeList;

pub use calcit_impl::CalcitImpl;
pub use calcit_struct::CalcitStruct;
pub use calcit_trait::CalcitTrait;
pub use fns::{CalcitArgLabel, CalcitFn, CalcitFnArgs, CalcitFnDefRef, CalcitFnUsageMeta, CalcitMacro, CalcitScope};
pub use list::CalcitList;
pub use local::CalcitLocal;
pub use proc_name::{CalcitProc, ProcTypeSignature};
pub use record::CalcitRecord;
pub use sum_type::{CalcitEnum, EnumVariant};
pub use symbol::{CalcitImport, CalcitSymbolInfo, ImportInfo};
pub use syntax_name::{CalcitSyntax, SyntaxTypeSignature};
pub use thunk::{CalcitThunk, CalcitThunkInfo};
pub use tuple::CalcitTuple;
pub use type_annotation::{
  CalcitFnTypeAnnotation, CalcitGenericBound, CalcitTypeAnnotation, DYNAMIC_TYPE, SchemaKind, brief_type_of_value, clear_type_slots,
  pop_type_slot_override, push_type_slot_override, register_program_lookups, register_type_slot, resolve_type_slot,
  value_matches_type_annotation, with_type_annotation_warning_context,
};

use compare::{
  compare_any_ref_values, compare_calcit_enum_values, compare_calcit_impl_values, compare_calcit_struct_values,
  compare_calcit_trait_values, compare_map_values, compare_record_values, compare_set_values,
};

use crate::builtins::ValueAndListeners;
use crate::call_stack::CallStackList;

/// dead simple counter for ID generator, better use nanoid in business
static ID_GEN: AtomicUsize = AtomicUsize::new(0);

/// dynamic data defined in Calcit
#[derive(Debug, Clone)]
pub enum Calcit {
  Nil,
  Bool(bool),
  Number(f64),
  Symbol {
    sym: Arc<str>,
    info: Arc<CalcitSymbolInfo>,
    /// positions in the tree of Cirru
    location: Option<Arc<Vec<u16>>>,
  },
  /// local variable
  Local(CalcitLocal),
  /// things that can be looked up from program snapshot, also things in :require block
  Import(CalcitImport),
  /// registered in runtime
  Registered(Arc<str>),
  /// something between string and enum, used a key or weak identifier
  Tag(EdnTag),
  Str(Arc<str>),
  /// to compile to js, global variables are stored in thunks at first, rather than evaluated
  /// and it is still different from quoted data which was intentionally turned in to data.
  Thunk(CalcitThunk), // code, value
  /// atom, holding a path to its state, data inside remains during hot code swapping
  Ref(Arc<str>, Arc<Mutex<ValueAndListeners>>),
  /// more tagged union type, more like an internal structure
  Tuple(CalcitTuple),
  /// binary data, to be used by FFIs
  Buffer(Vec<u8>),
  /// mutable append-only list, for performance-sensitive accumulation patterns.
  /// Only supports push/concat at the tail, no head/middle mutation.
  BufList(Arc<Mutex<Vec<Calcit>>>),
  /// cirru quoted data, for faster meta programming
  CirruQuote(Cirru),
  /// not for data, but for recursion
  Recur(Vec<Calcit>),
  List(Arc<CalcitList>),
  Set(rpds::HashTrieSetSync<Calcit>),
  Map(rpds::HashTrieMapSync<Calcit, Calcit>),
  /// with only static and limited keys, for performance and checking
  /// size of keys are values should be kept consistent
  Record(CalcitRecord),
  /// struct definition value, carries field types and names
  Struct(CalcitStruct),
  /// enum definition value, wraps enum prototype and variants
  Enum(CalcitEnum),
  /// trait definition value, carries method signatures
  Trait(CalcitTrait),
  /// trait implementation value, carries methods and target trait name
  Impl(CalcitImpl),
  /// native functions that providing feature from Rust
  Proc(CalcitProc),
  Macro {
    id: Arc<str>,
    info: Arc<CalcitMacro>,
  },
  Fn {
    id: Arc<str>,
    info: Arc<CalcitFn>,
  },
  /// name, ns... notice that `ns` is a meta info
  Syntax(CalcitSyntax, Arc<str>),
  /// Method is kind like macro, it's handled during preprocessing, into `&invoke` or `&invoke-native`
  /// method name, method kind (Invoke variant may carry inferred receiver type)
  Method(Arc<str>, MethodKind),
  /// currently only JavaScript calls are handled
  RawCode(RawCodeType, Arc<str>),
  /// reference to native Rust data, not intended for Calcit usages
  AnyRef(EdnAnyRef),
}

#[derive(Debug, Clone)]
pub enum RawCodeType {
  Js,
}

impl fmt::Display for Calcit {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use Calcit::*;
    match self {
      Nil => f.write_str("nil"),
      Bool(v) => f.write_str(&format!("{v}")),
      Number(n) => f.write_str(&format!("{n}")),
      Symbol { sym, .. } => f.write_str(&format!("'{sym}")),
      Local(CalcitLocal { sym, .. }) => f.write_str(&format!("'{sym}")),
      Import(CalcitImport { ns, def, .. }) => f.write_str(&format!("{ns}/{def}")),
      Registered(alias) => f.write_str(&format!("{alias}")),
      Tag(s) => f.write_str(&format!(":{s}")),
      Str(s) => {
        if is_simple_str(s) {
          write!(f, "|{s}")
        } else {
          // write!(f, "\"|{}\"", s.escape_default())
          write!(f, "\"|")?;
          for c in s.chars() {
            if cirru_edn::is_simple_char(c) {
              write!(f, "{c}")?;
            } else {
              write!(f, "{}", c.escape_default())?;
            }
          }
          write!(f, "\"")
        }
      } // TODO, escaping choices
      Thunk(thunk) => match thunk {
        CalcitThunk::Code { code, .. } => f.write_str(&format!("(&thunk _ {code})")),
      },
      CirruQuote(code) => f.write_str(&format!("(&cirru-quote {code})")),
      Ref(name, _locked_pair) => f.write_str(&format!("(&ref {name} ...)")),
      Tuple(tuple) => match &tuple.sum_type {
        Some(sum_type) => {
          f.write_str("(%:: ")?;
          f.write_str(&tuple.tag.to_string())?;
          for item in &tuple.extra {
            f.write_char(' ')?;
            f.write_str(&item.to_string())?;
          }
          f.write_str(&format!(" (:enum {})", sum_type.name()))?;
          f.write_str(")")
        }
        None => {
          f.write_str("(:: ")?;
          f.write_str(&tuple.tag.to_string())?;
          for item in &tuple.extra {
            f.write_char(' ')?;
            f.write_str(&item.to_string())?;
          }
          f.write_str(")")
        }
      },
      Buffer(buf) => {
        f.write_str("(&buffer")?;
        if buf.len() > 8 {
          f.write_str(&format!(
            " {} {} {} {} {} {} {} {} ..+{}",
            buffer_bit_hex(buf[0]),
            buffer_bit_hex(buf[1]),
            buffer_bit_hex(buf[2]),
            buffer_bit_hex(buf[3]),
            buffer_bit_hex(buf[4]),
            buffer_bit_hex(buf[5]),
            buffer_bit_hex(buf[6]),
            buffer_bit_hex(buf[7]),
            buf.len() - 8
          ))?;
        } else {
          for b in buf {
            f.write_str(" ")?;
            f.write_str(&buffer_bit_hex(b.to_owned()))?;
          }
        }
        f.write_str(")")
      }
      BufList(items) => {
        let items = items.lock().expect("BufList lock");
        f.write_str(&format!("(&buf-list count={})", items.len()))
      }
      Recur(xs) => {
        f.write_str("(&recur")?;
        for x in &**xs {
          f.write_str(&format!(" {x}"))?;
        }
        f.write_str(")")
      }
      List(xs) => {
        f.write_str("([]")?;
        xs.traverse_result(&mut |x| match f.write_str(&format!(" {x}")) {
          Ok(_) => Ok(()),
          Err(e) => Err(e),
        })?;
        f.write_str(")")
      }
      Set(xs) => {
        f.write_str("(#{}")?;
        for x in xs {
          f.write_str(&format!(" {x}"))?;
        }
        f.write_str(")")
      }
      Map(xs) => {
        f.write_str("({}")?;
        for (k, v) in xs {
          f.write_str(&format!(" ({k} {v})"))?;
        }
        f.write_str(")")?;
        Ok(())
      }
      Record(CalcitRecord { struct_ref, values, .. }) => {
        if record::LOOSE_RECORD_NAME == struct_ref.name.ref_str() {
          f.write_str("(?{}")?;
        } else {
          f.write_str(&format!("(%{{}} {}", Tag(struct_ref.name.to_owned())))?;
        }
        for idx in 0..struct_ref.fields.len() {
          f.write_str(&format!(" ({} {})", Calcit::tag(struct_ref.fields[idx].ref_str()), values[idx]))?;
        }
        f.write_str(")")
      }
      Struct(CalcitStruct {
        name, fields, field_types, ..
      }) => {
        f.write_str("(%struct ")?;
        f.write_str(&format!(":{name}"))?;
        for (k, t) in fields.iter().zip(field_types.iter()) {
          f.write_char(' ')?;
          f.write_str(&format!("(:{k} {})", t.to_brief_string()))?;
        }
        f.write_char(')')
      }
      Enum(enum_def) => {
        f.write_str("(%enum ")?;
        f.write_str(&format!(":{}", enum_def.name()))?;
        for variant in enum_def.variants() {
          f.write_char(' ')?;
          f.write_str(&format!("(:{}", variant.tag))?;
          for t in variant.payload_types() {
            f.write_char(' ')?;
            f.write_str(&t.to_brief_string())?;
          }
          f.write_char(')')?;
        }
        f.write_char(')')
      }
      Trait(t) => write!(f, "{t}"),
      Impl(impl_def) => {
        f.write_str("(%impl ")?;
        f.write_str(&format!(":{}", impl_def.name()))?;
        for idx in 0..impl_def.fields().len() {
          f.write_str(&format!(
            " ({} {})",
            Calcit::tag(impl_def.fields()[idx].ref_str()),
            impl_def.values[idx]
          ))?;
        }
        f.write_str(")")
      }
      Proc(name) => f.write_str(&format!("(&proc {name})")),
      Macro { info, .. } => {
        let name = &info.name;
        f.write_str(&format!("(&macro {name} ("))?;
        let mut need_space = false;
        for a in &**info.args {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&a.to_string())?;
          need_space = true;
        }
        f.write_str(") (")?;
        need_space = false;
        for b in &*info.body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str("))")
      }
      Fn { info, .. } => {
        let name = &info.name;
        f.write_str(&format!("(&fn {name} ("))?;
        let mut need_space = false;
        match &*info.args {
          CalcitFnArgs::MarkedArgs(xs) => {
            for a in xs {
              if need_space {
                f.write_str(" ")?;
              }
              f.write_str(&a.to_string())?;
              need_space = true;
            }
          }
          CalcitFnArgs::Args(xs) => {
            for a in xs {
              if need_space {
                f.write_str(" ")?;
              }
              f.write_str(&CalcitLocal::read_name(*a))?;
              need_space = true;
            }
          }
        }
        f.write_str(") ")?;
        need_space = false;
        for b in &*info.body {
          if need_space {
            f.write_str(" ")?;
          }
          f.write_str(&format_to_lisp(b))?;
          need_space = true;
        }
        f.write_str(")")
      }
      Syntax(name, _ns) => f.write_str(&format!("(&syntax {name})")),
      Method(name, method_kind) => match method_kind {
        MethodKind::Invoke(_) => f.write_str(&format!(".{name}")),
        MethodKind::Access => f.write_str(&format!(".-{name}")),
        MethodKind::InvokeNative => f.write_str(&format!(".!{name}")),
        MethodKind::TagAccess => f.write_str(&format!(".:{name}")),
        MethodKind::AccessOptional => f.write_str(&format!(".?-{name}")),
        MethodKind::InvokeNativeOptional => f.write_str(&format!(".?!{name}")),
      },
      RawCode(_, code) => f.write_str(&format!("(&raw-code {code})")),
      AnyRef(_r) => f.write_str("(&any-ref ...)"),
    }
  }
}

fn is_simple_str(tok: &str) -> bool {
  for c in tok.chars() {
    if !cirru_edn::is_simple_char(c) {
      return false;
    }
  }
  true
}

/// encode as hex string like `ff`
fn buffer_bit_hex(n: u8) -> String {
  hex::encode(vec![n])
}

/// display data into Lisp style for readability
pub fn format_to_lisp(x: &Calcit) -> String {
  use Calcit::*;
  match x {
    List(ys) => {
      let mut s = String::from("(");
      for (idx, y) in ys.iter().enumerate() {
        if idx > 0 {
          s.push(' ');
        }
        s.push_str(&format_to_lisp(y));
      }
      s.push(')');
      s
    }
    Symbol { sym, .. } => sym.to_string(),
    Local(CalcitLocal { sym, .. }) => sym.to_string(),
    Import(CalcitImport { ns, def, .. }) => format!("{ns}/{def}"),
    Registered(alias) => format!("{alias}"),
    Tag(s) => format!(":{s}"),
    Syntax(s, _ns) => s.to_string(),
    Proc(s) => s.to_string(),
    a => format!("{a}"),
  }
}

impl Hash for Calcit {
  fn hash<H>(&self, _state: &mut H)
  where
    H: Hasher,
  {
    use Calcit::*;
    match self {
      Nil => "nil:".hash(_state),
      Bool(v) => {
        "bool:".hash(_state);
        v.hash(_state);
      }
      Number(n) => {
        "number:".hash(_state);
        // TODO https://stackoverflow.com/q/39638363/883571
        (*n as usize).hash(_state)
      }
      Symbol { sym, .. } => {
        "symbol:".hash(_state);
        sym.hash(_state);
        // probaly no need, also won't be used in hashing
        // ns.hash(_state);
      }
      Local(CalcitLocal { sym, .. }) => {
        "local:".hash(_state);
        sym.hash(_state);
      }
      Import(CalcitImport { ns, def, .. }) => {
        "import:".hash(_state);
        ns.hash(_state);
        def.hash(_state);
      }
      Registered(alias) => {
        "registered:".hash(_state);
        alias.hash(_state);
      }
      Tag(s) => {
        "tag:".hash(_state);
        s.hash(_state);
      }
      Str(s) => {
        "string:".hash(_state);
        s.hash(_state);
      }
      Thunk(..) => {
        unreachable!("thunk should not be used in hashing")
      }
      Ref(name, _locked_pair) => {
        "ref:".hash(_state);
        name.hash(_state);
      }
      Tuple(CalcitTuple { tag, extra, sum_type: _ }) => {
        "tuple:".hash(_state);
        tag.hash(_state);
        extra.hash(_state);
        // enum prototype is internal metadata, not used in hashing
      }
      Buffer(buf) => {
        "buffer:".hash(_state);
        buf.hash(_state);
      }
      BufList(items) => {
        "buf-list:".hash(_state);
        let items = items.lock().expect("BufList lock");
        items.hash(_state);
      }
      CirruQuote(code) => {
        "cirru-quote:".hash(_state);
        code.hash(_state);
      }
      Recur(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      List(v) => {
        "list:".hash(_state);
        v.hash(_state);
      }
      Set(v) => {
        "set:".hash(_state);
        let mut xs: Vec<_> = v.iter().collect();
        // sort to ensure stable result
        xs.sort();
        for x in xs {
          x.hash(_state)
        }
      }
      Map(v) => {
        "map:".hash(_state);
        // order for map is not stable
        let mut xs: Vec<_> = v.iter().collect();
        xs.sort();
        for x in xs {
          x.hash(_state)
        }
      }
      Record(CalcitRecord { struct_ref, values, .. }) => {
        "record:".hash(_state);
        struct_ref.name.hash(_state);
        struct_ref.fields.hash(_state);
        values.hash(_state);
      }
      Struct(CalcitStruct {
        name,
        fields,
        field_types,
        generics,
        where_bounds,
        impls,
        ..
      }) => {
        "struct:".hash(_state);
        name.hash(_state);
        fields.hash(_state);
        field_types.hash(_state);
        generics.hash(_state);
        where_bounds.hash(_state);
        for imp in impls {
          imp.name().hash(_state);
          imp.origin().hash(_state);
          imp.fields().hash(_state);
          imp.values.hash(_state);
        }
      }
      Enum(enum_def) => {
        "enum:".hash(_state);
        enum_def.name().hash(_state);
        enum_def.generics().hash(_state);
        enum_def.where_bounds().hash(_state);
        for v in enum_def.variants() {
          v.tag.hash(_state);
          for t in v.payload_types() {
            t.hash(_state);
          }
        }
      }
      Proc(name) => {
        "proc:".hash(_state);
        name.hash(_state);
      }
      Macro { id: gen_id, .. } => {
        "macro:".hash(_state);
        // name.hash(_state);
        gen_id.hash(_state);
      }
      Fn { id: gen_id, .. } => {
        "fn:".hash(_state);
        // name.hash(_state);
        gen_id.hash(_state);
      }
      Syntax(name, _ns) => {
        "syntax:".hash(_state);
        // syntax name can be used as identity
        name.to_string().hash(_state);
      }
      Method(name, call_native) => {
        "method:".hash(_state);
        name.hash(_state);
        call_native.hash(_state);
      }
      RawCode(_name, code) => {
        "raw-code:".hash(_state);
        code.hash(_state);
      }
      Trait(t) => {
        "trait:".hash(_state);
        t.hash(_state);
      }
      Impl(impl_def) => {
        "impl:".hash(_state);
        impl_def.name.hash(_state);
        impl_def.origin.hash(_state);
        impl_def.fields.hash(_state);
        impl_def.values.hash(_state);
      }
      AnyRef(_) => {
        "any-ref:".hash(_state);
      }
    }
  }
}

impl Ord for Calcit {
  fn cmp(&self, other: &Self) -> Ordering {
    use Calcit::*;

    match (self, other) {
      (Nil, Nil) => Equal,
      (Nil, _) => Less,
      (_, Nil) => Greater,

      (Bool(a), Bool(b)) => a.cmp(b),
      (Bool(_), _) => Less,
      (_, Bool(_)) => Greater,

      (Number(a), Number(b)) => {
        if a < b {
          Less
        } else if a > b {
          Greater
        } else {
          Equal
        }
      }
      (Number(_), _) => Less,
      (_, Number(_)) => Greater,

      (Symbol { sym: a, .. }, Symbol { sym: b, .. }) => a.cmp(b),
      (Symbol { .. }, _) => Less,
      (_, Symbol { .. }) => Greater,

      (Local(CalcitLocal { sym: a, .. }), Local(CalcitLocal { sym: b, .. })) => a.cmp(b),
      (Local { .. }, _) => Less,
      (_, Local { .. }) => Greater,

      (Import(CalcitImport { ns: a, def: a1, .. }), Import(CalcitImport { ns: b, def: b1, .. })) => a.cmp(b).then(a1.cmp(b1)),
      (Import { .. }, _) => Less,
      (_, Import { .. }) => Greater,

      (Registered(a), Registered(b)) => a.cmp(b),
      (Registered(_), _) => Less,
      (_, Registered(_)) => Greater,

      (Tag(a), Tag(b)) => a.cmp(b),
      (Tag(_), _) => Less,
      (_, Tag(_)) => Greater,

      (Str(a), Str(b)) => a.cmp(b),
      (Str(_), _) => Less,
      (_, Str(_)) => Greater,

      (Thunk(a), Thunk(b)) => a.cmp(b),
      (Thunk(_), _) => Less,
      (_, Thunk(_)) => Greater,

      (CirruQuote(a), CirruQuote(b)) => a.cmp(b),
      (CirruQuote(_), _) => Less,
      (_, CirruQuote(_)) => Greater,

      (Ref(a, _), Ref(b, _)) => a.cmp(b),
      (Ref(_, _), _) => Less,
      (_, Ref(_, _)) => Greater,

      (
        Tuple(CalcitTuple {
          tag: a0, extra: extra0, ..
        }),
        Tuple(CalcitTuple {
          tag: a1, extra: extra1, ..
        }),
      ) => match a0.cmp(a1) {
        Equal => extra0.cmp(extra1),
        v => v,
      },
      (Tuple { .. }, _) => Less,
      (_, Tuple { .. }) => Greater,

      (Buffer(buf1), Buffer(buf2)) => buf1.cmp(buf2),
      (Buffer(..), _) => Less,
      (_, Buffer(..)) => Greater,

      (BufList(a), BufList(b)) => {
        let a = a.lock().expect("BufList lock");
        let b = b.lock().expect("BufList lock");
        a.cmp(&*b)
      }
      (BufList(_), _) => Less,
      (_, BufList(_)) => Greater,

      (Recur(a), Recur(b)) => a.cmp(b),
      (Recur(_), _) => Less,
      (_, Recur(_)) => Greater,

      (List(a), List(b)) => a.cmp(b),
      (List(_), _) => Less,
      (_, List(_)) => Greater,

      (Set(a), Set(b)) => compare_set_values(a, b),
      (Set(_), _) => Less,
      (_, Set(_)) => Greater,

      (Map(a), Map(b)) => compare_map_values(a, b),
      (Map(_), _) => Less,
      (_, Map(_)) => Greater,

      (Record(a), Record(b)) => compare_record_values(a, b),
      (Record { .. }, _) => Less,
      (_, Record { .. }) => Greater,

      (Struct(a), Struct(b)) => compare_calcit_struct_values(a, b),
      (Struct { .. }, _) => Less,
      (_, Struct { .. }) => Greater,

      (Enum(a), Enum(b)) => compare_calcit_enum_values(a, b),
      (Enum { .. }, _) => Less,
      (_, Enum { .. }) => Greater,

      (Trait(a), Trait(b)) => compare_calcit_trait_values(a, b),
      (Trait { .. }, _) => Less,
      (_, Trait { .. }) => Greater,

      (Impl(a), Impl(b)) => compare_calcit_impl_values(a, b),
      (Impl { .. }, _) => Less,
      (_, Impl { .. }) => Greater,
      (Proc(a), Proc(b)) => a.cmp(b),
      (Proc(_), _) => Less,
      (_, Proc(_)) => Greater,

      (Macro { id: a, .. }, Macro { id: b, .. }) => a.cmp(b),
      (Macro { .. }, _) => Less,
      (_, Macro { .. }) => Greater,

      (Fn { id: a, .. }, Fn { id: b, .. }) => a.cmp(b), // compared with nanoid
      (Fn { .. }, _) => Less,
      (_, Fn { .. }) => Greater,

      (Syntax(a, _), Syntax(b, _)) => a.cmp(b),
      (Syntax(..), _) => Less,
      (_, Syntax(..)) => Greater,

      (Method(a, na), Method(b, nb)) => match a.cmp(b) {
        Equal => na.cmp(nb),
        v => v,
      },
      (Method(..), _) => Less,
      (_, Method(..)) => Greater,

      (RawCode(_, a), RawCode(_, b)) => a.cmp(b),
      (RawCode(..), _) => Less,
      (_, RawCode(..)) => Greater,

      (AnyRef(a), AnyRef(b)) => compare_any_ref_values(a, b),
    }
  }
}

impl PartialOrd for Calcit {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Eq for Calcit {}

impl PartialEq for Calcit {
  fn eq(&self, other: &Self) -> bool {
    use Calcit::*;

    match (self, other) {
      (Nil, Nil) => true,
      (Bool(a), Bool(b)) => a == b,
      (Number(a), Number(b)) => a == b,
      (Symbol { sym: a, .. }, Symbol { sym: b, .. }) => a == b,
      (Local(CalcitLocal { sym: a, .. }), Local(CalcitLocal { sym: b, .. })) => a == b,
      (Registered(a), Registered(b)) => a == b,

      (Import(CalcitImport { ns: a, def: a1, .. }), Import(CalcitImport { ns: b, def: b1, .. })) => a == b && a1 == b1,
      (Tag(a), Tag(b)) => a == b,
      (Str(a), Str(b)) => a == b,
      (Thunk(a), Thunk(b)) => a == b,
      (Ref(a, _), Ref(b, _)) => a == b,
      (Tuple(a), Tuple(b)) => a == b,
      (Buffer(b), Buffer(d)) => b == d,
      (BufList(a), BufList(b)) => {
        let a = a.lock().expect("BufList lock");
        let b = b.lock().expect("BufList lock");
        *a == *b
      }
      (CirruQuote(b), CirruQuote(d)) => b == d,
      (List(a), List(b)) => a == b,
      (Set(a), Set(b)) => a == b,
      (Map(a), Map(b)) => a == b,
      (Record(a), Record(b)) => a == b,
      (Struct(a), Struct(b)) => a == b,
      (Enum(a), Enum(b)) => a.name() == b.name() && a.variants() == b.variants(),
      (Trait(a), Trait(b)) => a == b,
      (Impl(a), Impl(b)) => a == b,
      (Proc(a), Proc(b)) => a == b,
      (Macro { id: a, .. }, Macro { id: b, .. }) => a == b,
      // functions compared with nanoid
      (Fn { id: a, .. }, Fn { id: b, .. }) => a == b,
      (Syntax(a, _), Syntax(b, _)) => a == b,
      (Method(a, b), Method(c, d)) => a == c && b == d,
      (AnyRef(a), AnyRef(b)) => a == b,
      (_, _) => false,
    }
  }
}

impl From<TernaryTreeList<Calcit>> for Calcit {
  fn from(xs: TernaryTreeList<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::List(xs)))
  }
}

impl From<Vec<Calcit>> for Calcit {
  fn from(xs: Vec<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::Vector(xs)))
  }
}

impl From<&TernaryTreeList<Calcit>> for Calcit {
  fn from(xs: &TernaryTreeList<Calcit>) -> Calcit {
    Calcit::List(Arc::new(CalcitList::from(xs)))
  }
}

pub const CORE_NS: &str = "calcit.core";
pub const CALCIT_INTERNAL_NS: &str = "calcit.internal";
pub const BUILTIN_IMPLS_ENTRY: &str = "&init-builtin-impls!";
pub const GEN_NS: &str = "calcit.gen";
pub const GENERATED_DEF: &str = "gen%";

impl Calcit {
  /// data converting, not displaying
  pub fn turn_string(&self) -> String {
    match self {
      Calcit::Nil => String::from(""),
      Calcit::Str(s) => (**s).to_owned(),
      Calcit::Method(name, method_kind) => match method_kind {
        MethodKind::Invoke(_) => format!(".{name}"),
        MethodKind::Access => format!(".-{name}"),
        MethodKind::InvokeNative => format!(".!{name}"),
        MethodKind::TagAccess => format!(".:{name}"),
        MethodKind::AccessOptional => format!(".?-{name}"),
        MethodKind::InvokeNativeOptional => format!(".?!{name}"),
      },
      _ => format!("{self}"),
    }
  }

  pub fn lisp_str(&self) -> String {
    format_to_lisp(self)
  }

  pub fn new_str<T: Into<String>>(s: T) -> Calcit {
    Calcit::Str(s.into().into())
  }

  /// makes sure that tag is from global dict, not created by fresh
  pub fn tag(s: &str) -> Self {
    Calcit::Tag(EdnTag::from(s))
  }

  /// currently only symbol has node location
  pub fn get_location(&self) -> Option<NodeLocation> {
    match self {
      Calcit::Symbol { info, location, .. } => Some(NodeLocation::new(
        info.at_ns.to_owned(),
        info.at_def.to_owned(),
        location.to_owned().unwrap_or_default(),
      )),
      Calcit::Local(CalcitLocal { info, location, .. }) => Some(NodeLocation::new(
        info.at_ns.to_owned(),
        info.at_def.to_owned(),
        location.to_owned().unwrap_or_default(),
      )),
      _ => None,
    }
  }

  /// during evaluation, maybe skip evaluation since evaluated data is already in the value
  pub fn is_expr_evaluated(&self) -> bool {
    !matches!(
      self,
      // variants that need to be further evaluated
      Calcit::Symbol { .. } | Calcit::Local { .. } | Calcit::Import(..) | Calcit::Thunk(..) | Calcit::List(..)
    )
  }

  /// a semantic description of what kind of Calcit value this is, for displaying in CLI echo
  pub fn describe_type(&self) -> String {
    use Calcit::*;
    match self {
      Nil => "**Calcit nil** (`nil`)".to_string(),
      Bool(b) => format!("**Calcit boolean** (`{b}`)"),
      Number(n) => format!("**Calcit number** (`{n}`)"),
      Symbol { sym, .. } => {
        format!("**Calcit symbol** (`{sym}`) — a bare identifier name")
      }
      Local(local) => format!("**Calcit local** (`{}`) — a local variable binding", local.sym),
      Import(import) => format!(
        "**Calcit import** (`{}/{}`) — a reference to imported definition",
        import.ns, import.def
      ),
      Registered(alias) => {
        format!("**Calcit registered** (`{alias}`) — a runtime-registered value")
      }
      Tag(tag) => {
        format!("**Calcit keyword/tag** (`:{tag}`) — used as keys and identifiers")
      }
      Str(s) => {
        format!("**Calcit string literal** (`|{s}`) — Cirru strings use `|` prefix")
      }
      Thunk(_) => "**Calcit thunk** — a delayed-evaluation expression".to_string(),
      Ref(name, _) => format!("**Calcit atom/ref** (`{name}`) — a mutable state container"),
      Tuple(tuple) => {
        format!("**Calcit tuple** (`{}`) — a tagged union", tuple.tag)
      }
      Buffer(buf) => format!("**Calcit buffer** ({} bytes) — binary data", buf.len()),
      BufList(items) => {
        format!(
          "**Calcit buffer list** ({} items) — mutable list",
          items.lock().expect("BufList lock").len()
        )
      }
      CirruQuote(code) => format!("**Calcit cirru-quoted** — quoted Cirru AST: `{code}`"),
      Recur(xs) => format!("**Calcit recur** ({} args) — for tail recursion", xs.len()),
      List(xs) => format!("**Calcit list** ({} items) — vector/list", xs.len()),
      Set(xs) => format!("**Calcit set** ({} items) — unique values set", xs.size()),
      Map(xs) => format!("**Calcit map** ({} pairs) — key-value map", xs.size()),
      Record(record) => format!("**Calcit record** (`{}`) — typed struct", record.name()),
      Struct(strukt) => format!("**Calcit struct** (`{}`) — struct definition", strukt.name),
      Enum(enm) => format!("**Calcit enum** (`{}`) — enum/sum-type definition", enm.name()),
      Trait(trt) => format!("**Calcit trait** (`{}`) — trait definition", trt.name),
      Impl(imp) => format!("**Calcit impl** (`{}`) — trait implementation", imp.name),
      Proc(p) => format!("**Calcit procedure** (`{p}`) — native function"),
      Macro { id, .. } => format!("**Calcit macro** (`{id}`)"),
      Fn { id, .. } => format!("**Calcit function** (`{id}`)"),
      Syntax(syntax, _ns) => format!("**Calcit special syntax** (`{syntax}`)"),
      Method(name, kind) => {
        let kind_label = match kind {
          crate::calcit::MethodKind::Access => "attribute access",
          crate::calcit::MethodKind::InvokeNative => "native method call",
          crate::calcit::MethodKind::Invoke(_) => "method call",
          crate::calcit::MethodKind::TagAccess => "tag attribute access",
          crate::calcit::MethodKind::AccessOptional => "optional attribute access",
          crate::calcit::MethodKind::InvokeNativeOptional => "optional native method call",
        };
        format!("**Calcit method shorthand** (`.{name}`, {kind_label})")
      }
      RawCode(_, code) => format!("**Calcit raw code** (`{code}`) — inline code snippet"),
      AnyRef(_) => "**Calcit any-ref** — reference to native Rust data".to_string(),
    }
  }
}

pub fn gen_core_id() -> Arc<str> {
  let c = ID_GEN.fetch_add(1, SeqCst);
  format!("gen_id_{c}").into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcitErrKind {
  Syntax,
  Type,
  Arity,
  Var,
  Effect,
  Unexpected,
  Unimplemented,
}

impl fmt::Display for CalcitErrKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use CalcitErrKind::*;

    f.write_str(match self {
      Syntax => "Syntax",
      Type => "Type",
      Arity => "Arity",
      Var => "Var",
      Effect => "Effect",
      Unexpected => "Unexpected",
      Unimplemented => "Unimplemented",
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitErr {
  pub kind: CalcitErrKind,
  pub msg: String,
  pub code: Option<String>,
  pub warnings: Box<Vec<LocatedWarning>>,
  pub location: Option<Arc<NodeLocation>>,
  pub stack: CallStackList,
  /// Additional hint for error, such as usage examples
  pub hint: Option<String>,
}

impl fmt::Display for CalcitErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "[{} Error] {}", self.kind, self.headline())?;
    if let Some(location) = &self.location {
      write!(f, "\n  at {location}")?;
    }
    if !self.warnings.is_empty() {
      f.write_str("\n")?;
      LocatedWarning::print_list(&self.warnings);
    }
    if let Some(hint) = &self.hint {
      write!(f, "\n\n{hint}")?;
    }
    Ok(())
  }
}

impl From<String> for CalcitErr {
  /// hope this does not add extra costs
  fn from(msg: String) -> Self {
    CalcitErr {
      kind: CalcitErrKind::Unexpected,
      msg,
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location: None,
      hint: None,
    }
  }
}

impl CalcitErr {
  pub fn use_str<T: Into<String>>(kind: CalcitErrKind, msg: T) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location: None,
      hint: None,
    }
  }

  pub fn err_str<T: Into<String>>(kind: CalcitErrKind, msg: T) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location: None,
      hint: None,
    })
  }

  pub fn err_nodes<T: Into<String>>(kind: CalcitErrKind, msg: T, nodes: &[Calcit]) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: format!("{} {}", msg.into(), CalcitList::from(nodes)),
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location: None,
      hint: None,
    })
  }

  pub fn err_str_location<T: Into<String>>(kind: CalcitErrKind, msg: T, location: Option<Arc<NodeLocation>>) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location,
      hint: None,
    })
  }
  pub fn use_msg_stack<T: Into<String>>(kind: CalcitErrKind, msg: T, stack: &CallStackList) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: stack.to_owned(),
      location: None,
      hint: None,
    }
  }
  pub fn use_msg_stack_location<T: Into<String>>(
    kind: CalcitErrKind,
    msg: T,
    stack: &CallStackList,
    location: Option<NodeLocation>,
  ) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: stack.to_owned(),
      location: location.map(Arc::new),
      hint: None,
    }
  }

  pub fn use_msg_stack_location_with_hint<T: Into<String>, H: Into<String>>(
    kind: CalcitErrKind,
    msg: T,
    stack: &CallStackList,
    location: Option<NodeLocation>,
    hint: H,
  ) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: stack.to_owned(),
      location: location.map(Arc::new),
      hint: Some(hint.into()),
    }
  }

  pub fn use_msg_stack_location_with_code<T: Into<String>, C: Into<String>>(
    kind: CalcitErrKind,
    msg: T,
    code: C,
    stack: &CallStackList,
    location: Option<NodeLocation>,
  ) -> Self {
    CalcitErr {
      kind,
      msg: msg.into(),
      code: Some(code.into()),
      warnings: Box::default(),
      stack: stack.to_owned(),
      location: location.map(Arc::new),
      hint: None,
    }
  }

  /// Create error with hint (for examples and usage guidance)
  pub fn err_str_with_hint<T: Into<String>, H: Into<String>>(kind: CalcitErrKind, msg: T, hint: H) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: msg.into(),
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location: None,
      hint: Some(hint.into()),
    })
  }

  /// Create error with hint and nodes display
  pub fn err_nodes_with_hint<T: Into<String>, H: Into<String>>(
    kind: CalcitErrKind,
    msg: T,
    nodes: &[Calcit],
    hint: H,
  ) -> Result<Calcit, Self> {
    Err(CalcitErr {
      kind,
      msg: format!("{} {}", msg.into(), CalcitList::from(nodes)),
      code: None,
      warnings: Box::default(),
      stack: CallStackList::default(),
      location: None,
      hint: Some(hint.into()),
    })
  }

  pub fn headline(&self) -> String {
    if let Some(code) = &self.code {
      format!("[{code}] {}", self.msg)
    } else {
      self.msg.clone()
    }
  }

  pub fn code(&self) -> Option<&str> {
    self.code.as_deref()
  }
}

/// location of node in Snapshot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLocation {
  pub ns: Arc<str>,
  pub def: Arc<str>,
  pub coord: Arc<Vec<u16>>,
}

impl From<NodeLocation> for Edn {
  fn from(v: NodeLocation) -> Self {
    Edn::map_from_iter([
      (Edn::tag("ns"), v.ns.into()),
      (Edn::tag("def"), v.def.into()),
      (Edn::tag("coord"), (*v.coord).to_owned().into()),
    ])
  }
}

impl From<&NodeLocation> for Edn {
  fn from(v: &NodeLocation) -> Self {
    v.to_owned().into()
  }
}

impl fmt::Display for NodeLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.coord.is_empty() {
      write!(f, "{}/{}", self.ns, self.def)
    } else {
      write!(
        f,
        "{}/{} @{}",
        self.ns,
        self.def,
        self.coord.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(".")
      )
    }
  }
}

impl NodeLocation {
  pub fn new(ns: Arc<str>, def: Arc<str>, coord: Arc<Vec<u16>>) -> Self {
    NodeLocation {
      ns,
      def,
      coord: coord.to_owned(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedWarning {
  message: String,
  location: NodeLocation,
  code: Option<String>,
  hint: Option<String>,
}

impl Display for LocatedWarning {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(code) = &self.code {
      write!(f, "[{code}] {} @{}", self.message, self.location)?;
    } else {
      write!(f, "{} @{}", self.message, self.location)?;
    }
    if let Some(hint) = &self.hint {
      write!(f, "\n  hint: {hint}")?;
    }
    Ok(())
  }
}

/// warning from static checking of macro expanding
impl LocatedWarning {
  pub fn new(msg: String, location: NodeLocation) -> Self {
    LocatedWarning {
      message: msg,
      location,
      code: None,
      hint: None,
    }
  }

  pub fn new_with_detail(msg: String, location: NodeLocation, code: Option<String>, hint: Option<String>) -> Self {
    LocatedWarning {
      message: msg,
      location,
      code,
      hint,
    }
  }

  pub fn as_json(&self) -> serde_json::Value {
    let code = self.code_value();
    serde_json::json!({
      "message": &self.message,
      "location": {
        "ns": self.location.ns.to_string(),
        "def": self.location.def.to_string(),
        "coord": self.location.coord.to_vec()
      },
      "code": code,
      "hint": &self.hint,
    })
  }

  pub fn message(&self) -> &str {
    &self.message
  }

  pub fn code(&self) -> Option<&str> {
    self.code.as_deref()
  }

  pub fn location(&self) -> &NodeLocation {
    &self.location
  }

  pub fn hint(&self) -> Option<&str> {
    self.hint.as_deref()
  }

  fn code_value(&self) -> Option<String> {
    self.code.clone()
  }

  /// create an empty list
  pub fn default_list() -> Vec<Self> {
    vec![]
  }

  pub fn print_list(list: &Vec<Self>) {
    for warn in list {
      eprintln!("{warn}");
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MethodKind {
  /// (.call a) - may carry inferred receiver type for validation
  Invoke(Arc<CalcitTypeAnnotation>),
  /// (.!f a)
  InvokeNative,
  /// (.?!f a)
  InvokeNativeOptional,
  /// (.:k a)
  TagAccess,
  /// (.-p a)
  Access,
  /// (.?-p a)
  AccessOptional,
}

impl fmt::Display for MethodKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      MethodKind::Invoke(_) => write!(f, "invoke"),
      MethodKind::InvokeNative => write!(f, "invoke-native"),
      MethodKind::InvokeNativeOptional => write!(f, "invoke-native-optional"),
      MethodKind::TagAccess => write!(f, "tag-access"),
      MethodKind::Access => write!(f, "access"),
      MethodKind::AccessOptional => write!(f, "access-optional"),
    }
  }
}

/// Format examples hint from program data for error messages
/// Returns a formatted string with examples if available, or None
pub fn format_examples_hint(ns: &str, def: &str) -> Option<String> {
  use crate::program;

  let examples = program::lookup_def_examples(ns, def)?;
  if examples.is_empty() {
    return None;
  }

  let mut hint = String::from("💡 Usage examples:\n");
  for (i, example) in examples.iter().enumerate() {
    // Format each example with cirru-parser
    if let Ok(formatted) = cirru_parser::format(std::slice::from_ref(example), true.into()) {
      hint.push_str(&format!("\n  Example {}:\n", i + 1));
      // Indent each line of the example
      for line in formatted.lines() {
        hint.push_str("    ");
        hint.push_str(line);
        hint.push('\n');
      }
    }
  }

  Some(hint)
}

/// Helper to format examples hint specifically for a proc
pub fn format_proc_examples_hint(proc: &CalcitProc) -> Option<String> {
  let (ns, def) = proc.get_ns_def();
  format_examples_hint(ns, def)
}

#[cfg(test)]
mod tests {
  use super::*;
  use cirru_edn::EdnTag;
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  use std::sync::Arc;

  fn calcit_hash(value: &Calcit) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
  }

  #[test]
  fn display_methods_with_literal_syntax() {
    let dynamic = Calcit::Method(Arc::from("ceil"), MethodKind::Invoke(DYNAMIC_TYPE.clone()));
    let typed = Calcit::Method(Arc::from("display-by"), MethodKind::Invoke(Arc::new(CalcitTypeAnnotation::Number)));

    assert_eq!(dynamic.to_string(), ".ceil");
    assert_eq!(typed.to_string(), ".display-by");
    assert_eq!(typed.turn_string(), ".display-by");
  }

  #[test]
  fn infers_warning_arity_from_expects_message() {
    let warning = LocatedWarning::new_with_detail(
      String::from("[Warn] sample"),
      NodeLocation::new(Arc::from("app.main"), Arc::from("gen%"), Arc::from(vec![])),
      Some(String::from("P_SAMPLE")),
      Some(String::from("hint text")),
    );
    let payload = warning.as_json();

    assert_eq!(payload.get("code").and_then(|v| v.as_str()), Some("P_SAMPLE"));
    assert_eq!(payload.get("hint").and_then(|v| v.as_str()), Some("hint text"));
  }

  #[test]
  fn infers_warning_arity_from_expected_message() {
    let warning = LocatedWarning::new(
      String::from("[Warn] fn expected 3 arguments, but received: 2"),
      NodeLocation::new(Arc::from("app.main"), Arc::from("gen%"), Arc::from(vec![])),
    );
    let payload = warning.as_json();

    assert_eq!(payload.get("code").and_then(|v| v.as_str()), None);
    assert_eq!(payload.get("hint").and_then(|v| v.as_str()), None);
  }

  #[test]
  fn cmp_sets_does_not_panic_on_same_size() {
    let mut left = rpds::HashTrieSet::new_sync();
    left.insert_mut(Calcit::Number(1.0));
    left.insert_mut(Calcit::Number(3.0));

    let mut right = rpds::HashTrieSet::new_sync();
    right.insert_mut(Calcit::Number(2.0));
    right.insert_mut(Calcit::Number(3.0));

    let lv = Calcit::Set(left);
    let rv = Calcit::Set(right);
    let ord = lv.cmp(&rv);
    assert_eq!(ord, rv.cmp(&lv).reverse());
  }

  #[test]
  fn cmp_maps_does_not_panic_on_same_size() {
    let mut left = rpds::HashTrieMap::new_sync();
    left.insert_mut(Calcit::tag("a"), Calcit::Number(1.0));
    left.insert_mut(Calcit::tag("b"), Calcit::Number(2.0));

    let mut right = rpds::HashTrieMap::new_sync();
    right.insert_mut(Calcit::tag("a"), Calcit::Number(1.0));
    right.insert_mut(Calcit::tag("b"), Calcit::Number(3.0));

    let lv = Calcit::Map(left);
    let rv = Calcit::Map(right);
    let ord = lv.cmp(&rv);
    assert_eq!(ord, rv.cmp(&lv).reverse());
  }

  #[test]
  fn cmp_records_compares_values_when_same_name() {
    let struct_ref = Arc::new(CalcitStruct::from_fields(EdnTag::new("Person"), vec![EdnTag::new("age")]));
    let left = Calcit::Record(CalcitRecord {
      struct_ref: struct_ref.clone(),
      values: Arc::new(vec![Calcit::Number(1.0)]),
    });
    let right = Calcit::Record(CalcitRecord {
      struct_ref,
      values: Arc::new(vec![Calcit::Number(2.0)]),
    });

    let ord = left.cmp(&right);
    assert_eq!(ord, right.cmp(&left).reverse());
  }

  #[test]
  fn cmp_any_ref_does_not_panic() {
    let left = Calcit::AnyRef(EdnAnyRef::new(1_i64));
    let right = Calcit::AnyRef(EdnAnyRef::new(2_i64));

    let ord = left.cmp(&right);
    assert_eq!(ord, right.cmp(&left).reverse());
  }

  #[test]
  fn hash_any_ref_does_not_panic() {
    let value = Calcit::AnyRef(EdnAnyRef::new(1_i64));

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let _hash = hasher.finish();
  }

  #[test]
  fn symbol_local_import_are_not_cross_variant_equal() {
    let symbol = Calcit::Symbol {
      sym: Arc::from("foo"),
      info: Arc::new(CalcitSymbolInfo {
        at_def: Arc::from("demo"),
        at_ns: Arc::from("app.main"),
      }),
      location: None,
    };
    let local = Calcit::Local(CalcitLocal {
      idx: 0,
      sym: Arc::from("foo"),
      info: Arc::new(CalcitSymbolInfo {
        at_def: Arc::from("demo"),
        at_ns: Arc::from("app.main"),
      }),
      location: None,
      type_info: DYNAMIC_TYPE.clone(),
    });
    let import = Calcit::Import(CalcitImport {
      ns: Arc::from("lib.demo"),
      def: Arc::from("foo"),
      info: Arc::new(ImportInfo::NsReferDef {
        at_def: Arc::from("demo"),
        at_ns: Arc::from("app.main"),
      }),
      def_id: None,
    });

    assert_ne!(symbol, local);
    assert_ne!(symbol, import);
    assert_ne!(local, import);
  }

  #[test]
  fn cmp_equal_matches_eq_for_complex_named_variants() {
    let trait_left = CalcitTrait::new(EdnTag::new("X"), vec![EdnTag::new("foo")], vec![DYNAMIC_TYPE.clone()]);
    let trait_right = CalcitTrait::new(EdnTag::new("X"), vec![EdnTag::new("bar")], vec![DYNAMIC_TYPE.clone()]);
    let trait_left_value = Calcit::Trait(trait_left.clone());
    let trait_right_value = Calcit::Trait(trait_right.clone());
    assert_ne!(trait_left_value, trait_right_value);
    assert_ne!(trait_left_value.cmp(&trait_right_value), Equal);

    let impl_left = Calcit::Impl(CalcitImpl {
      name: EdnTag::new("Box"),
      origin: Some(Arc::new(trait_left.clone())),
      fields: Arc::new(vec![EdnTag::new("foo")]),
      values: Arc::new(vec![Calcit::Number(1.0)]),
    });
    let impl_right = Calcit::Impl(CalcitImpl {
      name: EdnTag::new("Box"),
      origin: Some(Arc::new(trait_right.clone())),
      fields: Arc::new(vec![EdnTag::new("foo")]),
      values: Arc::new(vec![Calcit::Number(1.0)]),
    });
    assert_ne!(impl_left, impl_right);
    assert_ne!(impl_left.cmp(&impl_right), Equal);

    let struct_left = Calcit::Struct(CalcitStruct {
      name: EdnTag::new("Person"),
      fields: Arc::new(vec![EdnTag::new("age")]),
      field_types: Arc::new(vec![DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    let struct_right = Calcit::Struct(CalcitStruct {
      name: EdnTag::new("Person"),
      fields: Arc::new(vec![EdnTag::new("age")]),
      field_types: Arc::new(vec![DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });
    assert_ne!(struct_left, struct_right);
    assert_ne!(struct_left.cmp(&struct_right), Equal);

    let enum_left_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::new("Result"), vec![EdnTag::new("ok")])),
      values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::Vector(vec![])))]),
    };
    let enum_right_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(EdnTag::new("Result"), vec![EdnTag::new("ok")])),
      values: Arc::new(vec![Calcit::List(Arc::new(CalcitList::Vector(vec![Calcit::tag("string")])))]),
    };
    let enum_left = Calcit::Enum(CalcitEnum::from_record(enum_left_record).expect("valid enum"));
    let enum_right = Calcit::Enum(CalcitEnum::from_record(enum_right_record).expect("valid enum"));
    assert_ne!(enum_left, enum_right);
    assert_ne!(enum_left.cmp(&enum_right), Equal);
  }

  #[test]
  fn enum_display_includes_variants() {
    let enum_record = CalcitRecord {
      struct_ref: Arc::new(CalcitStruct::from_fields(
        EdnTag::new("Result"),
        vec![EdnTag::new("ok"), EdnTag::new("err")],
      )),
      values: Arc::new(vec![
        Calcit::List(Arc::new(CalcitList::Vector(vec![]))),
        Calcit::List(Arc::new(CalcitList::Vector(vec![Calcit::tag("string")]))),
      ]),
    };
    let enum_value = Calcit::Enum(CalcitEnum::from_record(enum_record).expect("valid enum"));
    let text = enum_value.to_string();
    assert!(text.starts_with("(%enum :Result"), "unexpected display: {text}");
    assert!(text.contains(":ok)"), "missing zero-payload variant: {text}");
    assert!(text.contains(":err :string)"), "missing payload variant: {text}");
  }

  #[test]
  fn hash_matches_eq_for_complex_named_variants() {
    let trait_value = Calcit::Trait(CalcitTrait::new(
      EdnTag::new("Display"),
      vec![EdnTag::new("show")],
      vec![DYNAMIC_TYPE.clone()],
    ));
    let impl_value = Calcit::Impl(CalcitImpl {
      name: EdnTag::new("Box"),
      origin: Some(Arc::new(CalcitTrait::new(
        EdnTag::new("Display"),
        vec![EdnTag::new("show")],
        vec![DYNAMIC_TYPE.clone()],
      ))),
      fields: Arc::new(vec![EdnTag::new("show")]),
      values: Arc::new(vec![Calcit::Fn {
        id: Arc::from("fn-1"),
        info: Arc::new(CalcitFn {
          name: Arc::from("show"),
          def_ns: Arc::from("app.main"),
          def_ref: None,
          usage: CalcitFnUsageMeta::default(),
          scope: Arc::new(CalcitScope::default()),
          args: Arc::new(CalcitFnArgs::Args(vec![])),
          body: vec![Calcit::Str(Arc::from("ok"))],
          generics: Arc::new(vec![]),
          where_bounds: Arc::new(vec![]),
          return_type: DYNAMIC_TYPE.clone(),
          arg_types: vec![],
          rest_type: None,
        }),
      }]),
    });
    let struct_value = Calcit::Struct(CalcitStruct {
      name: EdnTag::new("S"),
      fields: Arc::new(vec![EdnTag::new("v")]),
      field_types: Arc::new(vec![DYNAMIC_TYPE.clone()]),
      generics: Arc::new(vec![Arc::from("T")]),
      where_bounds: Arc::new(vec![]),
      impls: vec![],
    });

    for value in [trait_value, impl_value, struct_value] {
      let cloned = value.clone();
      assert_eq!(value, cloned);
      assert_eq!(value.cmp(&cloned), Equal);
      assert_eq!(calcit_hash(&value), calcit_hash(&cloned));
    }
  }

  #[test]
  fn node_location_uses_dot_separator() {
    let loc = NodeLocation::new(
      Arc::from("app.comp.sidebar"),
      Arc::from("comp-sidebar"),
      Arc::from(vec![3, 2, 1, 0]),
    );
    assert_eq!(loc.to_string(), "app.comp.sidebar/comp-sidebar @3.2.1.0");
  }
}

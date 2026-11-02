use std::sync::Arc;

use cirru_edn::EdnTag;

use crate::calcit::{Calcit, CalcitTypeAnnotation};

/// A closed, untagged union defined by `defunion`.
///
/// Members are distinguished by their runtime kind, so the value carries no
/// wrapper: a `CssValue` value is just a String or a Number. Values of a union
/// type can only be taken apart with type patterns in `match`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcitUnionDef {
  definition_ref: Option<Arc<str>>,
  pub name: EdnTag,
  /// Member type forms as written, kept for display and serialization.
  pub member_forms: Arc<Vec<Calcit>>,
}

impl CalcitUnionDef {
  pub fn new(name: EdnTag, member_forms: Vec<Calcit>) -> Self {
    Self {
      definition_ref: None,
      name,
      member_forms: Arc::new(member_forms),
    }
  }

  pub fn definition_ref(&self) -> Option<&Arc<str>> {
    self.definition_ref.as_ref()
  }

  pub fn with_definition_ref(mut self, ns: &str, def: &str) -> Self {
    self.definition_ref = Some(Arc::from(format!("{ns}/{def}")));
    self
  }

  pub fn set_definition_ref(&mut self, reference: Option<Arc<str>>) {
    self.definition_ref = reference;
  }

  /// Parsed member types; a runtime member `(List Number)` reads as the
  /// compact type it was quoted from. Named members stay symbolic (`TypeRef`) until a
  /// relation resolves them, exactly like Struct field types.
  pub fn members(&self) -> Vec<Arc<CalcitTypeAnnotation>> {
    self
      .member_forms
      .iter()
      .map(|form| CalcitTypeAnnotation::parse_type_annotation_form_with_generics(&super::declared_type_form(form), &[]))
      .collect()
  }

  /// Same identity rule as nominal enums: the definition reference when both
  /// sides know it, otherwise the name.
  pub fn same_definition(&self, other: &CalcitUnionDef) -> bool {
    match (&self.definition_ref, &other.definition_ref) {
      (Some(a), Some(b)) => a == b,
      _ => self.name == other.name,
    }
  }
}

/// The runtime kind that tells union members apart. Each kind may appear at
/// most once per union, so a shallow runtime check selects exactly one member.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnionMemberKind {
  Bool,
  Number,
  String,
  Tag,
  Symbol,
  List,
  Map,
  Set,
  Fn,
  Ref,
  Buffer,
  CirruQuote,
  /// A named struct, keyed by its definition reference or name.
  Struct(Arc<str>),
  /// A named enum, keyed by its definition reference or name.
  Enum(Arc<str>),
}

impl UnionMemberKind {
  pub fn describe(&self) -> String {
    match self {
      Self::Struct(name) => format!("struct {name}"),
      Self::Enum(name) => format!("enum {name}"),
      other => format!("{other:?}"),
    }
  }
}

/// One checked member of a union: its declared type, runtime kind and the
/// name a `match` type pattern uses for it.
#[derive(Debug, Clone)]
pub struct UnionMember {
  pub annotation: Arc<CalcitTypeAnnotation>,
  pub kind: UnionMemberKind,
  pub pattern_name: Arc<str>,
  /// `(ns, def)` of a named Struct/Enum member, used to test the definition at runtime.
  pub definition: Option<(Arc<str>, Arc<str>)>,
}

fn short_name(reference: &str) -> Arc<str> {
  Arc::from(reference.rsplit_once('/').map(|(_, def)| def).unwrap_or(reference))
}

fn mentions_dynamic(annotation: &CalcitTypeAnnotation) -> bool {
  use CalcitTypeAnnotation as T;
  match annotation {
    T::Dynamic | T::DynFn | T::AnonymousEnum | T::Custom(_) | T::JsObject => true,
    T::List(inner) | T::Set(inner) | T::Ref(inner) | T::Nullable(inner, _) | T::Variadic(inner) => mentions_dynamic(inner),
    T::Map(key, value) => mentions_dynamic(key) || mentions_dynamic(value),
    T::Struct(_, args) | T::Enum(_, args) | T::TypeRef(_, args) | T::Tuple(args) => args.iter().any(|arg| mentions_dynamic(arg)),
    T::Fn(signature) => {
      signature.arg_types.iter().any(|arg| mentions_dynamic(arg))
        || signature.rest_type.as_ref().is_some_and(|rest| mentions_dynamic(rest))
        || mentions_dynamic(&signature.return_type)
    }
    _ => false,
  }
}

impl CalcitUnionDef {
  fn display_name(&self) -> String {
    self
      .definition_ref
      .as_ref()
      .map(|reference| reference.to_string())
      .unwrap_or_else(|| self.name.ref_str().to_owned())
  }

  /// Check every member and return them in declaration order. Errors name
  /// the offending member and the rule it breaks.
  pub fn checked_members(&self) -> Result<Vec<UnionMember>, String> {
    let mut visiting = vec![self.display_name()];
    let members = self.collect_members(&mut visiting, None)?;
    if members.len() < 2 {
      return Err(format!("defunion {} needs at least two member types", self.display_name()));
    }
    for (index, member) in members.iter().enumerate() {
      if let Some(previous) = members[..index].iter().find(|other| other.pattern_name == member.pattern_name) {
        return Err(format!(
          "defunion {} members `{}` and `{}` share the pattern name `{}`, so a `match` type pattern could not tell them apart",
          self.display_name(),
          previous.annotation.to_brief_string(),
          member.annotation.to_brief_string(),
          member.pattern_name
        ));
      }
      if let Some(previous) = members[..index].iter().find(|other| other.kind == member.kind) {
        return Err(format!(
          "defunion {} members `{}` and `{}` share the runtime kind {}, so a value cannot be told apart at runtime",
          self.display_name(),
          previous.annotation.to_brief_string(),
          member.annotation.to_brief_string(),
          member.kind.describe()
        ));
      }
    }
    Ok(members)
  }

  fn collect_members(&self, visiting: &mut Vec<String>, fallback_namespace: Option<&str>) -> Result<Vec<UnionMember>, String> {
    let declaring_namespace = self
      .definition_ref()
      .and_then(|reference| reference.rsplit_once('/'))
      .map(|(ns, _)| ns)
      .or(fallback_namespace);
    let mut members = vec![];
    for annotation in self.members() {
      let annotation = match declaring_namespace {
        Some(ns) => super::type_refs::qualify_namespace_type_refs(annotation, ns),
        None => annotation,
      };
      if let Some(nested) = annotation.resolve_to_union() {
        let nested_name = nested.display_name();
        if visiting.contains(&nested_name) {
          return Err(format!(
            "defunion {} refers to itself through {nested_name}; recursive unions are not supported yet",
            self.display_name()
          ));
        }
        visiting.push(nested_name);
        members.extend(nested.collect_members(visiting, declaring_namespace)?);
        visiting.pop();
        continue;
      }
      members.push(self.check_member(annotation)?);
    }
    Ok(members)
  }

  fn check_member(&self, annotation: Arc<CalcitTypeAnnotation>) -> Result<UnionMember, String> {
    use CalcitTypeAnnotation as T;
    let union_name = self.display_name();
    let reject = |reason: &str| {
      Err(format!(
        "defunion {union_name} member `{}` is not allowed: {reason}",
        annotation.to_brief_string()
      ))
    };
    if mentions_dynamic(&annotation) {
      return reject("members may not contain Dynamic at any depth; write the element types (bare List/Map/Fn are Dynamic)");
    }
    let builtin = |kind: UnionMemberKind, name: &str| {
      Ok(UnionMember {
        annotation: annotation.clone(),
        kind,
        pattern_name: Arc::from(name),
        definition: None,
      })
    };
    match annotation.as_ref() {
      T::Bool => builtin(UnionMemberKind::Bool, "Bool"),
      T::Number => builtin(UnionMemberKind::Number, "Number"),
      T::String => builtin(UnionMemberKind::String, "String"),
      T::Tag => builtin(UnionMemberKind::Tag, "Tag"),
      T::Symbol => builtin(UnionMemberKind::Symbol, "Symbol"),
      T::List(_) => builtin(UnionMemberKind::List, "List"),
      T::Tuple(_) => builtin(UnionMemberKind::List, "Tuple"),
      T::Map(_, _) => builtin(UnionMemberKind::Map, "Map"),
      T::Set(_) => builtin(UnionMemberKind::Set, "Set"),
      T::Fn(_) => builtin(UnionMemberKind::Fn, "Fn"),
      T::Ref(_) => builtin(UnionMemberKind::Ref, "Ref"),
      T::Buffer => builtin(UnionMemberKind::Buffer, "Buffer"),
      T::CirruQuote => builtin(UnionMemberKind::CirruQuote, "CirruQuote"),
      T::Nil | T::Unit => reject("absence is written as `Option U` outside the union"),
      T::Never => reject("Never has no values"),
      T::TypeVar(_) | T::TypeSlot(_) => reject("unions take no type parameters"),
      T::Numeric(_) => reject("a numeric refinement cannot be told apart from Number at runtime"),
      T::Trait(_) | T::TraitSet(_) => reject("trait objects have no closed runtime kind"),
      _ => {
        if let Some((enum_def, path)) = annotation.resolve_to_enum_with_ref() {
          let reference: Arc<str> = enum_def
            .definition_ref()
            .cloned()
            .unwrap_or_else(|| Arc::from(enum_def.name().ref_str()));
          if matches!(
            reference.as_ref(),
            "calcit.core/Option" | "calcit.core/Result" | "Option" | "Result"
          ) {
            return reject("Option and Result stay outside unions; write `Option U` for absence");
          }
          let definition = path.or_else(|| reference.rsplit_once('/').map(|(ns, def)| (Arc::from(ns), Arc::from(def))));
          if definition.is_none() {
            return reject("anonymous enums have no definition to test at runtime");
          }
          return Ok(UnionMember {
            annotation: annotation.clone(),
            pattern_name: short_name(&reference),
            kind: UnionMemberKind::Enum(reference),
            definition,
          });
        }
        if let Some((struct_def, path)) = annotation.resolve_to_struct_with_ref() {
          let reference: Arc<str> = struct_def
            .definition_ref
            .clone()
            .unwrap_or_else(|| Arc::from(struct_def.name.ref_str()));
          let definition = path.or_else(|| reference.rsplit_once('/').map(|(ns, def)| (Arc::from(ns), Arc::from(def))));
          if definition.is_none() {
            return reject("anonymous structs have no definition to test at runtime");
          }
          return Ok(UnionMember {
            annotation: annotation.clone(),
            pattern_name: short_name(&reference),
            kind: UnionMemberKind::Struct(reference),
            definition,
          });
        }
        reject("only built-in value types, named structs, named enums and other unions can be members")
      }
    }
  }
}

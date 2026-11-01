use std::sync::Arc;

use super::{CalcitFnTypeAnnotation, CalcitTrait, CalcitTypeAnnotation, MacroExpansionType, MacroSignature, MacroSyntaxType};
use crate::program;

/// Transform symbolic type references while preserving type variables and concrete annotation kinds.
pub fn map_type_references<F>(annotation: Arc<CalcitTypeAnnotation>, resolve_type_ref: &F) -> Arc<CalcitTypeAnnotation>
where
  F: Fn(&Arc<str>, Arc<Vec<Arc<CalcitTypeAnnotation>>>) -> Arc<CalcitTypeAnnotation>,
{
  map_type_and_trait_references(annotation, resolve_type_ref, &|trait_def| trait_def)
}

/// Transform symbolic type references and resolved trait bounds without falling back to source text matching.
pub fn map_schema_references<F, G>(
  annotation: Arc<CalcitTypeAnnotation>,
  resolve_type_ref: &F,
  resolve_trait_bound: &G,
) -> Arc<CalcitTypeAnnotation>
where
  F: Fn(&Arc<str>, Arc<Vec<Arc<CalcitTypeAnnotation>>>) -> Arc<CalcitTypeAnnotation>,
  G: Fn(Arc<CalcitTrait>) -> Arc<CalcitTrait>,
{
  map_type_and_trait_references(annotation, resolve_type_ref, resolve_trait_bound)
}

fn map_type_and_trait_references<F, G>(
  annotation: Arc<CalcitTypeAnnotation>,
  resolve_type_ref: &F,
  resolve_trait_bound: &G,
) -> Arc<CalcitTypeAnnotation>
where
  F: Fn(&Arc<str>, Arc<Vec<Arc<CalcitTypeAnnotation>>>) -> Arc<CalcitTypeAnnotation>,
  G: Fn(Arc<CalcitTrait>) -> Arc<CalcitTrait>,
{
  let map_bounds = |bounds: &Arc<Vec<crate::calcit::CalcitGenericBound>>| {
    Arc::new(
      bounds
        .iter()
        .map(|bound| crate::calcit::CalcitGenericBound {
          name: bound.name.clone(),
          traits: Arc::new(bound.traits.iter().cloned().map(resolve_trait_bound).collect()),
        })
        .collect(),
    )
  };
  match annotation.as_ref() {
    CalcitTypeAnnotation::TypeRef(name, args) => {
      let resolved_args = Arc::new(
        args
          .iter()
          .map(|arg| map_type_and_trait_references(arg.clone(), resolve_type_ref, resolve_trait_bound))
          .collect::<Vec<_>>(),
      );
      resolve_type_ref(name, resolved_args)
    }
    CalcitTypeAnnotation::List(inner) => Arc::new(CalcitTypeAnnotation::List(map_type_and_trait_references(
      inner.clone(),
      resolve_type_ref,
      resolve_trait_bound,
    ))),
    CalcitTypeAnnotation::Tuple(slots) => Arc::new(CalcitTypeAnnotation::Tuple(Arc::new(
      slots
        .iter()
        .map(|slot| map_type_and_trait_references(slot.clone(), resolve_type_ref, resolve_trait_bound))
        .collect(),
    ))),
    CalcitTypeAnnotation::Map(key, value) => Arc::new(CalcitTypeAnnotation::Map(
      map_type_and_trait_references(key.clone(), resolve_type_ref, resolve_trait_bound),
      map_type_and_trait_references(value.clone(), resolve_type_ref, resolve_trait_bound),
    )),
    CalcitTypeAnnotation::Set(inner) => Arc::new(CalcitTypeAnnotation::Set(map_type_and_trait_references(
      inner.clone(),
      resolve_type_ref,
      resolve_trait_bound,
    ))),
    CalcitTypeAnnotation::Ref(inner) => Arc::new(CalcitTypeAnnotation::Ref(map_type_and_trait_references(
      inner.clone(),
      resolve_type_ref,
      resolve_trait_bound,
    ))),
    CalcitTypeAnnotation::Nullable(inner, origin) => Arc::new(CalcitTypeAnnotation::Nullable(
      map_type_and_trait_references(inner.clone(), resolve_type_ref, resolve_trait_bound),
      *origin,
    )),
    CalcitTypeAnnotation::Variadic(inner) => Arc::new(CalcitTypeAnnotation::Variadic(map_type_and_trait_references(
      inner.clone(),
      resolve_type_ref,
      resolve_trait_bound,
    ))),
    CalcitTypeAnnotation::Fn(signature) => Arc::new(CalcitTypeAnnotation::Fn(Arc::new(CalcitFnTypeAnnotation {
      runtime_arity: signature.runtime_arity,
      generics: signature.generics.clone(),
      where_bounds: map_bounds(&signature.where_bounds),
      arg_types: signature
        .arg_types
        .iter()
        .map(|arg| map_type_and_trait_references(arg.clone(), resolve_type_ref, resolve_trait_bound))
        .collect(),
      return_type: map_type_and_trait_references(signature.return_type.clone(), resolve_type_ref, resolve_trait_bound),
      fn_kind: signature.fn_kind,
      rest_type: signature
        .rest_type
        .as_ref()
        .map(|rest| map_type_and_trait_references(rest.clone(), resolve_type_ref, resolve_trait_bound)),
      features: signature.features.clone(),
    }))),
    CalcitTypeAnnotation::Macro(signature) => {
      let map_contract = |contract: &MacroSyntaxType| match contract {
        MacroSyntaxType::Expr(semantic) => MacroSyntaxType::Expr(map_type_and_trait_references(
          semantic.clone(),
          resolve_type_ref,
          resolve_trait_bound,
        )),
        other => other.clone(),
      };
      let expansion = match &signature.expansion {
        MacroExpansionType::Expr(semantic) => MacroExpansionType::Expr(map_type_and_trait_references(
          semantic.clone(),
          resolve_type_ref,
          resolve_trait_bound,
        )),
        MacroExpansionType::Definition(semantic) => MacroExpansionType::Definition(map_type_and_trait_references(
          semantic.clone(),
          resolve_type_ref,
          resolve_trait_bound,
        )),
        other => other.clone(),
      };
      Arc::new(CalcitTypeAnnotation::Macro(Arc::new(MacroSignature {
        generics: signature.generics.clone(),
        where_bounds: map_bounds(&signature.where_bounds),
        required_inputs: Arc::new(signature.required_inputs.iter().map(map_contract).collect()),
        optional_inputs: Arc::new(signature.optional_inputs.iter().map(map_contract).collect()),
        rest_input: signature.rest_input.as_ref().map(map_contract),
        expansion,
        capabilities: signature.capabilities.clone(),
        features: signature.features.clone(),
      })))
    }
    CalcitTypeAnnotation::Syntax(contract) => Arc::new(CalcitTypeAnnotation::Syntax(Arc::new(match contract.as_ref() {
      MacroSyntaxType::Expr(semantic) => MacroSyntaxType::Expr(map_type_and_trait_references(
        semantic.clone(),
        resolve_type_ref,
        resolve_trait_bound,
      )),
      other => other.clone(),
    }))),
    CalcitTypeAnnotation::Struct(struct_def, args) => Arc::new(CalcitTypeAnnotation::Struct(
      struct_def.clone(),
      Arc::new(
        args
          .iter()
          .map(|arg| map_type_and_trait_references(arg.clone(), resolve_type_ref, resolve_trait_bound))
          .collect(),
      ),
    )),
    CalcitTypeAnnotation::Enum(enum_def, args) => Arc::new(CalcitTypeAnnotation::Enum(
      enum_def.clone(),
      Arc::new(
        args
          .iter()
          .map(|arg| map_type_and_trait_references(arg.clone(), resolve_type_ref, resolve_trait_bound))
          .collect(),
      ),
    )),
    CalcitTypeAnnotation::Trait(trait_def) => Arc::new(CalcitTypeAnnotation::Trait(resolve_trait_bound(trait_def.clone()))),
    CalcitTypeAnnotation::TraitSet(traits) => Arc::new(CalcitTypeAnnotation::TraitSet(Arc::new(
      traits.iter().cloned().map(resolve_trait_bound).collect(),
    ))),
    _ => annotation,
  }
}

/// Qualify nominal type references from the namespace where their declaration
/// was written. Nested Struct fields often use concise forms such as
/// `'Router`; once that field type flows into a caller in another namespace,
/// retaining only `Router` is ambiguous and prevents required-field lowering.
/// Source migrations also use this resolver before treating a short nominal name as core.
pub fn qualify_namespace_type_refs(annotation: Arc<CalcitTypeAnnotation>, declaring_ns: &str) -> Arc<CalcitTypeAnnotation> {
  map_type_references(annotation, &|name, resolved_args| {
    let qualified_name = match program::resolve_type_name_in_ns(declaring_ns, name) {
      Some((ns, def)) => Arc::from(format!("{ns}/{def}")),
      None => name.clone(),
    };
    Arc::new(CalcitTypeAnnotation::TypeRef(qualified_name, resolved_args))
  })
}

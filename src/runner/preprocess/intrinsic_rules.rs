//! Summaries of the built-in checker rules behind `calcit.core` signatures
//! marked `(:open :intrinsic)` (#2649).
//!
//! Such a signature keeps the open shape the runtime accepts, while the
//! checker decides argument and result types per receiver kind (see
//! `checked_call_contract`, `type_checking` and `type_inference`). The
//! summaries here are what `query peek` / `query schema` show for them; a
//! test keeps this table and the marked core schemas in step.

/// One line per receiver kind: `receiver: argument and result types`.
pub fn intrinsic_rule_summary(def: &str) -> Option<&'static [&'static str]> {
  let lines: &'static [&'static str] = match def {
    "first" | "last" => &[
      "List T -> Option T",
      "String -> Option String",
      "enum value -> Option Dynamic (payload by position)",
    ],
    "nth" => &[
      "List T, Number -> Option T",
      "String, Number -> Option String",
      "Tuple: literal index -> that slot's type",
    ],
    "get" => &[
      "Map K V, K -> Option V",
      "List T, Number -> Option T",
      "String, Number -> Option String",
      "enum value, Number -> Option Dynamic",
    ],
    "any?" | "every?" => &[
      "List T | Set T, (Fn T (-> Bool)) -> Bool",
      "Map K V, (Fn (List Dynamic) (-> Bool)) -> Bool",
    ],
    "each" => &[
      "List T | Set T, (Fn T (-> R)) -> Unit",
      "Map K V, (Fn (List Dynamic) (-> R)) -> Unit",
    ],
    "filter" => &[
      "List T, (Fn T (-> Bool)) -> List T",
      "Set T, (Fn T (-> Bool)) -> Set T",
      "Map K V, (Fn (Tuple K V) (-> Bool)) -> Map K V",
    ],
    "map" => &[
      "List T, (Fn T (-> U)) -> List U",
      "Set T, (Fn T (-> U)) -> Set U",
      "Map K V, (Fn (Tuple K V) (-> (Tuple K2 V2))) -> Map K2 V2",
    ],
    "update" => &[
      "List T, Number, (Fn T (-> T)) -> List T",
      "Map K V, K, (Fn V (-> V)) -> Map K V",
      "Struct S, field tag, (Fn F (-> F)) -> S, F being the field's type",
    ],
    "assoc" => &[
      "List T, Number, T -> List T",
      "Map K V, K, V -> Map K V",
      "Struct S, field tag, the field's type -> S",
      "enum value, Number, payload -> the same enum",
    ],
    "dissoc" => &["Map K V, & K -> Map K V", "List T, Number -> List T"],
    "includes?" => &[
      "List T | Set T, T -> Bool",
      "Map K V, V -> Bool (checks values)",
      "String, String -> Bool",
    ],
    "get-in" => &[
      "nested Map / List / Struct with a literal path -> Result of the leaf type and PathError",
      "otherwise -> Result Dynamic PathError",
    ],
    "contains-in?" => &["nested Map / List / Struct, a path whose Struct steps are literal field tags -> Bool"],
    "assoc-in" => &["nested Map with a literal path, the leaf value type -> the same Map type"],
    "update-in" => &["nested Map with a literal path, (Fn (Option Leaf) (-> Leaf)) -> the same Map type"],
    "dissoc-in" => &["nested Map / List with a literal path -> the receiver's type"],
    "pairs-map" => &["List (List T) -> Map T T"],
    "apply" => &["(Fn A... (-> R)), a List literal or List of matching arguments -> R"],
    "&list:sort-by" => &["List T, (Fn T (-> K)) -> List T", "List T, field tag -> List T"],
    "&fs-read-dir" => &[
      "由已知 enum 定义和单 String 字段的路径 struct 推断结果，成功分支包含该 struct 的 List",
      "构造器不透明或形状无法证明时，返回 Dynamic",
    ],
    "impl-traits" => &["struct or enum definition, & Impl -> the same definition with the impls attached"],
    "with-type-slot" => &["compile-time form: the slot override is erased; the body's type is the result"],
    "defn" | "defmacro" => &["definition syntax: the compiler builds the function from its parameters and body"],
    _ => return None,
  };
  Some(lines)
}

#[cfg(test)]
mod tests {
  use super::intrinsic_rule_summary;
  use crate::calcit::{CalcitTypeAnnotation, FnOpenReason};
  use std::collections::BTreeSet;

  /// Every calcit.core signature marked `(:open :intrinsic)` has a rule
  /// summary, and every summary belongs to a marked signature.
  #[test]
  fn intrinsic_marks_and_rule_summaries_match() {
    let core_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cirru/calcit-core.cirru");
    let content = std::fs::read_to_string(core_path).expect("read calcit-core.cirru");
    let edn = cirru_edn::parse(&content).expect("parse calcit-core.cirru");
    let snapshot = crate::snapshot::load_snapshot_data(&edn, "calcit-core.cirru").expect("load core snapshot");
    let core = &snapshot.files[crate::calcit::CORE_NS];
    let mut marked = BTreeSet::new();
    for (name, entry) in &core.defs {
      if let CalcitTypeAnnotation::Fn(signature) = entry.schema.as_ref()
        && signature.open_reason.0 == Some(FnOpenReason::Intrinsic)
      {
        marked.insert(name.as_str());
        assert!(
          intrinsic_rule_summary(name).is_some(),
          "calcit.core/{name} is marked `(:open :intrinsic)` but has no rule summary"
        );
      }
    }
    for name in core.defs.keys() {
      if intrinsic_rule_summary(name).is_some() {
        assert!(
          marked.contains(name.as_str()),
          "calcit.core/{name} has a rule summary but its schema is not marked `(:open :intrinsic)`"
        );
      }
    }
    // Receiver-specialized collection contracts are all marked.
    for name in [
      "first",
      "last",
      "nth",
      "get",
      "any?",
      "every?",
      "each",
      "filter",
      "map",
      "update",
      "assoc-in",
      "update-in",
      "pairs-map",
    ] {
      assert!(marked.contains(name), "calcit.core/{name} should be marked `(:open :intrinsic)`");
    }
  }
}

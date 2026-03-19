use crate::builtins::{err_arity, err_arity_with_hint, meta::type_of};
use crate::calcit::{Calcit, CalcitErr, CalcitErrKind, CalcitProc, CalcitRecord, CalcitTuple, format_proc_examples_hint};
use crate::util::number::is_integer;
use cirru_parser::Cirru;
use serde_json::{Map, Number, Value};

pub fn parse(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs {
    [Calcit::Str(content)] => {
      let value: Value = serde_json::from_str(content).map_err(|err| {
        CalcitErr::use_str(
          CalcitErrKind::Type,
          format!("json-parse expected valid JSON string, got parse error: {err}"),
        )
      })?;
      json_to_calcit(value)
    }
    [value] => {
      let msg = format!(
        "json-parse expected a string, but received: {}",
        type_of(&[value.to_owned()])?.lisp_str()
      );
      let hint = format_proc_examples_hint(&CalcitProc::JsonParse).unwrap_or_default();
      CalcitErr::err_str_with_hint(CalcitErrKind::Type, msg, hint)
    }
    _ => err_arity("json-parse expected 1 argument", xs),
  }
}

pub fn stringify(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs {
    [value] => {
      let json = calcit_to_json(value)?;
      Ok(Calcit::new_str(serde_json::to_string(&json).expect("serialize json string")))
    }
    _ => err_arity_with_hint(
      "json-stringify expected 1 argument",
      xs,
      format_proc_examples_hint(&CalcitProc::JsonStringify).unwrap_or_default(),
    ),
  }
}

pub fn pretty(xs: &[Calcit]) -> Result<Calcit, CalcitErr> {
  match xs {
    [value] => {
      let json = calcit_to_json(value)?;
      Ok(Calcit::new_str(
        serde_json::to_string_pretty(&json).expect("serialize pretty json string"),
      ))
    }
    _ => err_arity_with_hint(
      "json-pretty expected 1 argument",
      xs,
      format_proc_examples_hint(&CalcitProc::JsonPretty).unwrap_or_default(),
    ),
  }
}

fn json_to_calcit(value: Value) -> Result<Calcit, CalcitErr> {
  match value {
    Value::Null => Ok(Calcit::Nil),
    Value::Bool(flag) => Ok(Calcit::Bool(flag)),
    Value::Number(number) => number
      .as_f64()
      .map(Calcit::Number)
      .ok_or_else(|| CalcitErr::use_str(CalcitErrKind::Type, format!("json number cannot fit into calcit number: {number}"))),
    Value::String(text) => Ok(Calcit::new_str(text)),
    Value::Array(items) => {
      let mut values = Vec::with_capacity(items.len());
      for item in items {
        values.push(json_to_calcit(item)?);
      }
      Ok(Calcit::from(values))
    }
    Value::Object(entries) => {
      let mut map = rpds::HashTrieMap::new_sync();
      for (key, value) in entries {
        map.insert_mut(Calcit::tag(&key), json_to_calcit(value)?);
      }
      Ok(Calcit::Map(map))
    }
  }
}

fn calcit_to_json(value: &Calcit) -> Result<Value, CalcitErr> {
  match value {
    Calcit::Nil => Ok(Value::Null),
    Calcit::Bool(flag) => Ok(Value::Bool(*flag)),
    Calcit::Number(number) => calcit_number_to_json(*number),
    Calcit::Str(text) => Ok(Value::String(text.to_string())),
    Calcit::Tag(tag) => Ok(Value::String(tag.ref_str().to_owned())),
    Calcit::Symbol { sym, .. } => Ok(Value::String(sym.to_string())),
    Calcit::List(list) => {
      let mut items = Vec::with_capacity(list.len());
      for item in &**list {
        items.push(calcit_to_json(item)?);
      }
      Ok(Value::Array(items))
    }
    Calcit::Set(values) => {
      let mut items = Vec::with_capacity(values.size());
      for item in values {
        items.push(calcit_to_json(item)?);
      }
      Ok(Value::Array(items))
    }
    Calcit::Map(entries) => {
      let mut object = Map::new();
      for (key, value) in entries {
        object.insert(json_key(key)?, calcit_to_json(value)?);
      }
      Ok(Value::Object(object))
    }
    Calcit::Tuple(CalcitTuple { tag, extra, .. }) => {
      let mut items = Vec::with_capacity(extra.len() + 1);
      items.push(calcit_to_json(tag)?);
      for item in extra {
        items.push(calcit_to_json(item)?);
      }
      Ok(Value::Array(items))
    }
    Calcit::CirruQuote(code) => cirru_to_json(code),
    Calcit::Buffer(buffer) => Ok(Value::String(format!("0x{}", hex::encode(buffer)))),
    Calcit::Record(CalcitRecord { struct_ref, values }) => {
      let mut object = Map::with_capacity(struct_ref.fields.len());
      for (idx, field) in struct_ref.fields.iter().enumerate() {
        object.insert(field.ref_str().to_owned(), calcit_to_json(&values[idx])?);
      }
      Ok(Value::Object(object))
    }
    Calcit::Ref(..)
    | Calcit::Thunk(..)
    | Calcit::Recur(..)
    | Calcit::Struct(..)
    | Calcit::Enum(..)
    | Calcit::Trait(..)
    | Calcit::Impl(..)
    | Calcit::Proc(..)
    | Calcit::Macro { .. }
    | Calcit::Fn { .. }
    | Calcit::Syntax(..)
    | Calcit::Method(..)
    | Calcit::RawCode(..)
    | Calcit::Import(..)
    | Calcit::Registered(..)
    | Calcit::Local(..)
    | Calcit::AnyRef(..) => {
      let msg = format!(
        "json-stringify cannot encode value of type: {}",
        type_of(&[value.to_owned()])?.lisp_str()
      );
      Err(CalcitErr::use_str(CalcitErrKind::Type, msg))
    }
  }
}

fn calcit_number_to_json(number: f64) -> Result<Value, CalcitErr> {
  if !number.is_finite() {
    return Err(CalcitErr::use_str(
      CalcitErrKind::Type,
      format!("json-stringify cannot encode number: {number}"),
    ));
  }

  if is_integer(number) {
    if number >= i64::MIN as f64 && number <= i64::MAX as f64 {
      return Ok(Value::Number(Number::from(number as i64)));
    }
    if number >= 0.0 && number <= u64::MAX as f64 {
      return Ok(Value::Number(Number::from(number as u64)));
    }
  }

  Number::from_f64(number)
    .map(Value::Number)
    .ok_or_else(|| CalcitErr::use_str(CalcitErrKind::Type, format!("json-stringify cannot encode number: {number}")))
}

fn json_key(value: &Calcit) -> Result<String, CalcitErr> {
  match value {
    Calcit::Tag(tag) => Ok(tag.ref_str().to_owned()),
    Calcit::Str(text) => Ok(text.to_string()),
    other => {
      let msg = format!(
        "json-stringify expected object keys to be tags or strings, but received: {}",
        type_of(&[other.to_owned()])?.lisp_str()
      );
      Err(CalcitErr::use_str(CalcitErrKind::Type, msg))
    }
  }
}

fn cirru_to_json(code: &Cirru) -> Result<Value, CalcitErr> {
  match code {
    Cirru::Leaf(text) => Ok(Value::String(text.to_string())),
    Cirru::List(items) => {
      let mut values = Vec::with_capacity(items.len());
      for item in items {
        values.push(cirru_to_json(item)?);
      }
      Ok(Value::Array(values))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::util::string::wrap_js_str;

  #[test]
  fn json_parse_turns_object_keys_into_tags() {
    let value = parse(&[Calcit::new_str("{\"a\":1,\"nested\":{\"ok\":true}}")]).expect("parse json");

    let Calcit::Map(root) = value else {
      panic!("expected map from json parse");
    };
    assert_eq!(root.get(&Calcit::tag("a")), Some(&Calcit::Number(1.0)));
    match root.get(&Calcit::tag("nested")) {
      Some(Calcit::Map(nested)) => {
        assert_eq!(nested.get(&Calcit::tag("ok")), Some(&Calcit::Bool(true)));
      }
      other => panic!("expected nested map, got {other:?}"),
    }
  }

  #[test]
  fn json_parse_rejects_non_string_input() {
    let error = parse(&[Calcit::Number(1.0)]).expect_err("json-parse should reject non-string input");
    assert_eq!(error.kind, CalcitErrKind::Type);
    assert!(error.msg.contains("json-parse expected a string"));
  }

  #[test]
  fn json_stringify_rejects_wrong_arity() {
    let error = stringify(&[]).expect_err("json-stringify should reject wrong arity");
    assert_eq!(error.kind, CalcitErrKind::Arity);
    assert!(error.msg.contains("json-stringify expected 1 argument"));
  }

  #[test]
  fn json_pretty_rejects_wrong_arity() {
    let error = pretty(&[Calcit::Nil, Calcit::Nil]).expect_err("json-pretty should reject wrong arity");
    assert_eq!(error.kind, CalcitErrKind::Arity);
    assert!(error.msg.contains("json-pretty expected 1 argument"));
  }

  #[test]
  fn json_pretty_uses_two_spaces() {
    let value = Calcit::Map(rpds::HashTrieMap::new_sync().insert(Calcit::tag("a"), Calcit::Number(1.0)));
    let result = pretty(&[value]).expect("pretty json");
    assert_eq!(result, Calcit::new_str("{\n  \"a\": 1\n}"));
  }

  #[test]
  fn json_stringify_turns_tags_into_plain_strings() {
    let result = stringify(&[Calcit::tag("ok")]).expect("stringify tag");
    assert_eq!(result, Calcit::new_str(wrap_js_str("ok")));
  }

  #[test]
  fn json_stringify_formats_integer_numbers_without_decimal_suffix() {
    let result = stringify(&[Calcit::Number(1.0)]).expect("stringify integer number");
    assert_eq!(result, Calcit::new_str("1"));
  }
}

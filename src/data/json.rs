use serde_json::Value;

use crate::primes::{Calcit, CalcitItems};

pub fn json_to_calcit(data: &Value) -> Calcit {
  match data {
    Value::Null => Calcit::Nil,
    Value::Bool(b) => Calcit::Bool(*b),
    Value::Number(n) => Calcit::Number(n.as_f64().unwrap()), // why f64
    Value::String(s) => {
      if s.starts_with(':') {
        // special logic to parse keyword
        Calcit::Keyword(s.strip_prefix(":").unwrap().to_string())
      } else {
        Calcit::Str(s.clone())
      }
    }
    Value::Array(xs) => {
      let mut ys: CalcitItems = im::vector![];
      for x in xs {
        ys.push_back(json_to_calcit(x));
      }
      Calcit::List(ys)
    }
    Value::Object(xs) => {
      let mut ys: im::HashMap<Calcit, Calcit> = im::HashMap::new();
      for (k, v) in xs {
        let key = if k.starts_with(':') {
          Calcit::Keyword(k.strip_prefix(":").unwrap().to_string())
        } else {
          Calcit::Str(k.clone())
        };
        ys.insert(key, json_to_calcit(v));
      }
      Calcit::Map(ys)
    }
  }
}

/// option for "add colon to keyword"
pub fn calcit_to_json(data: &Calcit, add_colon: bool) -> Result<Value, String> {
  match data {
    Calcit::Nil => Ok(Value::Null),
    Calcit::Bool(b) => Ok(Value::Bool(*b)),
    Calcit::Number(n) => match serde_json::value::Number::from_f64(*n) {
      Some(v) => Ok(Value::Number(v)),
      None => Err(format!("failed to convert to number: {}", n)),
    },
    Calcit::Symbol(s, ..) => Ok(Value::String(s.to_owned())),
    Calcit::Keyword(s) => {
      if add_colon {
        Ok(Value::String(format!(":{}", s)))
      } else {
        Ok(Value::String(s.to_owned()))
      }
    }
    Calcit::Str(s) => Ok(Value::String(s.to_owned())),
    Calcit::List(xs) => {
      let mut ys: Vec<Value> = vec![];
      for x in xs {
        ys.push(calcit_to_json(x, add_colon)?);
      }
      Ok(Value::Array(ys))
    }
    Calcit::Map(xs) => {
      let mut data = serde_json::Map::new();
      for (k, v) in xs {
        match k {
          Calcit::Str(s) => {
            data.insert(s.to_owned(), calcit_to_json(v, add_colon)?);
          }
          Calcit::Keyword(s) => {
            if add_colon {
              data.insert(format!(":{}", s), calcit_to_json(v, add_colon)?);
            } else {
              data.insert(s.to_owned(), calcit_to_json(v, add_colon)?);
            }
          }
          a => return Err(format!("expected string/keyword for json keys, got: {}", a)),
        }
      }

      Ok(Value::Object(data))
    }
    Calcit::Record(_, fields, values) => {
      let mut data = serde_json::Map::new();
      for idx in 0..fields.len() {
        data.insert(fields[idx].clone(), calcit_to_json(&values[idx], add_colon)?);
      }
      Ok(Value::Object(data))
    }
    a => Err(format!("cannot convert to json: {}", a)),
  }
}

/// public interface to builtins
pub fn parse_json(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(Calcit::Str(s)) => match serde_json::from_str::<Value>(s.as_str()) {
      Ok(v) => Ok(json_to_calcit(&v)),
      Err(e) => Err(format!("failed to parse JSON: {}", e)),
    },
    Some(a) => Err(format!("parse-json expected a string, got: {}", a)),
    None => Err(String::from("parse-json expected 1 argument, got nothing")),
  }
}

pub fn stringify_json(xs: &CalcitItems) -> Result<Calcit, String> {
  match xs.get(0) {
    Some(x) => {
      let add_colon = match xs.get(1) {
        Some(Calcit::Bool(b)) => *b,
        Some(a) => return Err(format!("expected a bool, got: {}", a)),
        None => false,
      };
      let ret = calcit_to_json(x, add_colon)?;
      match serde_json::to_string(&ret) {
        Ok(s) => Ok(Calcit::Str(s)),
        Err(e) => Err(format!("failed to generate string: {}", e)),
      }
    }
    None => Err(String::from("expected a value")),
  }
}

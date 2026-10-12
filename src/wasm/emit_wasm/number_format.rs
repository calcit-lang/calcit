use super::*;
use wasm_encoder::reencode::{Error as ReencodeError, Reencode};
use wasmparser::{DataKind, Operator, Parser, Payload, TypeRef};

pub(super) const NUMBER_TABLE_BASE: i32 = 8192;
pub(super) const NUMBER_STACK_TOP: i32 = 2048;
pub(super) const NUMBER_SCRATCH: i32 = 4096;

/// The embedded object is regenerated from wasm-runtime/number-format.
/// It has one imported memory, nine local functions, one stack global, and
/// one read-only data segment. No host functions survive the link.
const NUMBER_OBJECT: &[u8] = include_bytes!("number-format.wasm");

struct NumberReencoder {
  function_base: u32,
  stack_global: u32,
  function_count: u32,
}

impl Reencode for NumberReencoder {
  type Error = String;

  fn function_index(&mut self, index: u32) -> Result<u32, ReencodeError<Self::Error>> {
    if index >= self.function_count {
      return Err(ReencodeError::UserError(format!(
        "Number formatter calls external function {index}"
      )));
    }
    Ok(self.function_base + index)
  }

  fn global_index(&mut self, index: u32) -> Result<u32, ReencodeError<Self::Error>> {
    if index != 0 {
      return Err(ReencodeError::UserError(format!("Number formatter uses unexpected global {index}")));
    }
    Ok(self.stack_global)
  }
}

pub(super) fn load_number_functions(function_base: u32, stack_global: u32) -> Result<(Vec<CompiledFn>, Vec<u8>), String> {
  if NUMBER_STACK_TOP >= NUMBER_SCRATCH || NUMBER_SCRATCH + 400 > NUMBER_TABLE_BASE {
    return Err("Number formatter scratch memory overlaps its stack or read-only table".into());
  }
  wasmparser::Validator::new()
    .validate_all(NUMBER_OBJECT)
    .map_err(|error| format!("invalid embedded Number formatter: {error}"))?;

  let mut signatures: Vec<(Vec<ValType>, Vec<ValType>)> = Vec::new();
  let mut function_types = Vec::new();
  let mut bodies = Vec::new();
  let mut data = None;
  let mut imports = 0;
  let mut globals = 0;
  let mut format_export = false;
  for payload in Parser::new(0).parse_all(NUMBER_OBJECT) {
    match payload.map_err(|error| error.to_string())? {
      Payload::TypeSection(section) => {
        for signature in section.into_iter_err_on_gc_types() {
          let signature = signature.map_err(|error| error.to_string())?;
          let mut converter = wasm_encoder::reencode::RoundtripReencoder;
          let params = signature
            .params()
            .iter()
            .map(|ty| converter.val_type(*ty).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
          let results = signature
            .results()
            .iter()
            .map(|ty| converter.val_type(*ty).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
          signatures.push((params, results));
        }
      }
      Payload::ImportSection(section) => {
        for import in section.into_imports() {
          let import = import.map_err(|error| error.to_string())?;
          if import.module != "env" || import.name != "memory" || !matches!(import.ty, TypeRef::Memory(_)) {
            return Err(format!("Number formatter has unexpected import {}.{}", import.module, import.name));
          }
          imports += 1;
        }
      }
      Payload::FunctionSection(section) => {
        for index in section {
          function_types.push(index.map_err(|error| error.to_string())? as usize);
        }
      }
      Payload::GlobalSection(section) => {
        globals += section.count();
      }
      Payload::ExportSection(section) => {
        for export in section {
          let export = export.map_err(|error| error.to_string())?;
          if export.name != "format_f64" || export.kind != wasmparser::ExternalKind::Func || export.index != 0 {
            return Err(format!("Number formatter has unexpected export {}", export.name));
          }
          format_export = true;
        }
      }
      Payload::CodeSectionEntry(body) => bodies.push(body),
      Payload::DataSection(section) => {
        for segment in section {
          let segment = segment.map_err(|error| error.to_string())?;
          let DataKind::Active {
            memory_index: 0,
            offset_expr,
          } = segment.kind
          else {
            return Err("Number formatter has a nonconstant data segment".into());
          };
          let mut operators = offset_expr.get_operators_reader();
          if !matches!(operators.read().map_err(|error| error.to_string())?, Operator::I32Const { value } if value == NUMBER_TABLE_BASE)
            || !matches!(operators.read().map_err(|error| error.to_string())?, Operator::End)
          {
            return Err("Number formatter data table has an unexpected offset".into());
          }
          if data.replace(segment.data.to_vec()).is_some() {
            return Err("Number formatter has multiple data segments".into());
          }
        }
      }
      Payload::TableSection(_) | Payload::MemorySection(_) | Payload::ElementSection(_) | Payload::StartSection { .. } => {
        return Err("Number formatter has unexpected table, memory, element, or start section".into());
      }
      _ => {}
    }
  }
  if imports != 1 || globals != 1 || !format_export || data.is_none() || function_types.len() != bodies.len() {
    return Err("Number formatter object does not match its pinned ABI".into());
  }
  if NUMBER_TABLE_BASE as usize + data.as_ref().expect("validated data segment").len() > HEAP_BASE as usize {
    return Err("Number formatter read-only table overlaps Calcit heap".into());
  }
  let mut converter = NumberReencoder {
    function_base,
    stack_global,
    function_count: bodies.len() as u32,
  };
  let mut functions = Vec::with_capacity(bodies.len());
  for (body, type_index) in bodies.into_iter().zip(function_types) {
    let (params, results) = signatures
      .get(type_index)
      .ok_or_else(|| format!("Number formatter type {type_index} is missing"))?
      .clone();
    let mut locals = Vec::new();
    for local in body.get_locals_reader().map_err(|error| error.to_string())? {
      let (count, ty) = local.map_err(|error| error.to_string())?;
      let ty = converter.val_type(ty).map_err(|error| error.to_string())?;
      locals.extend(std::iter::repeat_n(ty, count as usize));
    }
    let mut instructions = Vec::new();
    let mut reader = body.get_operators_reader().map_err(|error| error.to_string())?;
    while !reader.eof() {
      let operator = reader.read().map_err(|error| error.to_string())?;
      instructions.push(converter.instruction(operator).map_err(|error| error.to_string())?);
    }
    if !matches!(instructions.pop(), Some(Instruction::End)) {
      return Err("Number formatter function is missing its final end".into());
    }
    functions.push(CompiledFn {
      export_name: None,
      params,
      results,
      locals,
      instructions,
    });
  }
  if functions
    .first()
    .is_none_or(|function| function.params != [ValType::F64, ValType::I32] || function.results != [ValType::I32])
  {
    return Err("Number formatter entry has an unexpected signature".into());
  }
  Ok((functions, data.expect("validated data segment")))
}

pub(super) fn number_table_data() -> Result<&'static [u8], String> {
  for payload in Parser::new(0).parse_all(NUMBER_OBJECT) {
    if let Payload::DataSection(section) = payload.map_err(|error| error.to_string())? {
      let mut segments = section.into_iter();
      let segment = segments
        .next()
        .ok_or_else(|| "Number formatter data segment is missing".to_string())?
        .map_err(|error| error.to_string())?;
      if segments.next().is_some() {
        return Err("Number formatter has multiple data segments".into());
      }
      return Ok(segment.data);
    }
  }
  Err("Number formatter data section is missing".into())
}

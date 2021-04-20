use crate::primes::{CalcitData, CalcitData::*};
use crate::program;
use crate::runner;
use std::collections::HashSet;

// TODO
pub fn preprocess_expr(
  expr: &CalcitData,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Ok(CalcitNil)
}

// TODO
pub fn preprocess_ns_def(
  ns: &str,
  def: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Ok(CalcitNil)
}

// tradition rule for processing exprs
pub fn preprocess_each_items(
  expr: &CalcitData,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Ok(CalcitNil)
}

pub fn preprocess_defn(
  expr: &CalcitData,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Ok(CalcitNil)
}

pub fn preprocess_call_let(
  expr: &CalcitData,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Ok(CalcitNil)
}

pub fn preprocess_quote(
  expr: &CalcitData,
  scope_defs: &HashSet<String>,
  file_ns: &str,
  program_code: &program::ProgramCodeData,
) -> Result<CalcitData, String> {
  Ok(CalcitNil)
}

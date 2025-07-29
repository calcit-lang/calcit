use std::sync::Arc;

use crate::{Calcit, CalcitErr, call_stack::CallStackList, program, runner::evaluate_expr};

use super::CalcitScope;

/// thunk is currently bound to namespace/definition
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalcitThunkInfo {
  pub ns: Arc<str>,
  pub def: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CalcitThunk {
  Code { code: Arc<Calcit>, info: Arc<CalcitThunkInfo> },
  Evaled { code: Arc<Calcit>, value: Arc<Calcit> },
}

impl CalcitThunk {
  pub fn get_code(&self) -> &Calcit {
    match self {
      Self::Code { code, .. } => code,
      Self::Evaled { code, .. } => code,
    }
  }

  /// evaluate the thunk, and write back to program state
  pub fn evaluated(&self, scope: &CalcitScope, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
    match self {
      Self::Evaled { value, .. } => Ok((**value).to_owned()),
      Self::Code { code, info } => {
        // println!("from thunk: {}", sym);
        let evaled_v = evaluate_expr(code, scope, &info.ns, call_stack)?;
        // and write back to program state to fix duplicated evalution
        program::write_evaled_def(
          &info.ns,
          &info.def,
          Calcit::Thunk(Self::Evaled {
            code: code.to_owned(),
            value: Arc::new(evaled_v.to_owned()),
          }),
        )?;
        Ok(evaled_v)
      }
    }
  }
}

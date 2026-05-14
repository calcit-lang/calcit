use std::sync::Arc;

use crate::{Calcit, CalcitErr, CalcitErrKind, call_stack::CallStackList, program, runner::evaluate_expr};

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
}

impl CalcitThunk {
  pub fn get_code(&self) -> &Calcit {
    match self {
      Self::Code { code, .. } => code,
    }
  }

  pub fn evaluated_default(&self, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
    self.evaluated(&CalcitScope::default(), call_stack)
  }

  /// evaluate the thunk, and write back to program state
  pub fn evaluated(&self, scope: &CalcitScope, call_stack: &CallStackList) -> Result<Calcit, CalcitErr> {
    match self {
      Self::Code { code, info } => {
        program::mark_runtime_def_resolving(&info.ns, &info.def);

        // println!("from thunk: {}", sym);
        let runtime_value = match evaluate_expr(code, scope, &info.ns, call_stack) {
          Ok(value) => value,
          Err(e) => {
            program::mark_runtime_def_errored(&info.ns, &info.def, Arc::from(e.to_string()));
            return Err(e);
          }
        };

        // and write back to program state to fix duplicated evalution
        if let Err(e) = program::write_runtime_ready(&info.ns, &info.def, runtime_value.to_owned()) {
          program::mark_runtime_def_errored(&info.ns, &info.def, Arc::from(e.as_str()));
          return Err(CalcitErr::use_msg_stack(CalcitErrKind::Unexpected, e, call_stack));
        }

        Ok(runtime_value)
      }
    }
  }
}

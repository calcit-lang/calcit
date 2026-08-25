use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use crate::calcit::{CalcitErr, CalcitErrKind, CalcitProc, CalcitSyntax, MacroCapability, MethodKind, NodeLocation};
use crate::call_stack::{CallStackList, StackKind};

#[derive(Debug, Clone)]
struct MacroExecutionContext {
  macro_name: Arc<str>,
  declared: Arc<HashSet<MacroCapability>>,
  call_location: Option<NodeLocation>,
}

thread_local! {
  static MACRO_EXECUTION_STACK: RefCell<Vec<MacroExecutionContext>> = const { RefCell::new(vec![]) };
}

struct ContextGuard;

impl Drop for ContextGuard {
  fn drop(&mut self) {
    MACRO_EXECUTION_STACK.with(|stack| {
      stack.borrow_mut().pop();
    });
  }
}

/// Execute a strict macro body under its declared compile-time capability
/// policy. A thread-local stack makes calls through arbitrary helper functions
/// auditable without adding policy parameters to every evaluator function.
pub fn with_macro_context<T>(
  macro_name: Arc<str>,
  declared: Arc<HashSet<MacroCapability>>,
  call_location: Option<NodeLocation>,
  f: impl FnOnce() -> Result<T, CalcitErr>,
) -> Result<T, CalcitErr> {
  MACRO_EXECUTION_STACK.with(|stack| {
    stack.borrow_mut().push(MacroExecutionContext {
      macro_name,
      declared,
      call_location,
    });
  });
  let _guard = ContextGuard;
  f()
}

#[derive(Debug, Clone, Copy)]
enum CapabilityPolicy {
  Requires(MacroCapability),
  Forbidden(MacroCapability),
}

impl CapabilityPolicy {
  fn capability(self) -> MacroCapability {
    match self {
      Self::Requires(capability) | Self::Forbidden(capability) => capability,
    }
  }
}

fn proc_policy(proc: CalcitProc) -> Option<CapabilityPolicy> {
  use CalcitProc::*;
  let required = match proc {
    GetEnv => MacroCapability::EnvRead,
    ReadFile | ReadDir => MacroCapability::FsRead,
    NativeGetOs | NativeGetCalcitBackend | NativeGetCalcitRunningMode => MacroCapability::PlatformRead,
    UnixTimeMs | CpuTime => MacroCapability::ClockRead,
    GenerateId
    | NativeResetGenSymIndex
    | RegisterCalcitBuiltinImpls
    | NativeBufListNew
    | NativeBufListPush
    | NativeBufListConcat
    | Atom
    | AtomDeref
    | AddWatch
    | RemoveWatch => MacroCapability::MutableState,
    WriteFile => return Some(CapabilityPolicy::Forbidden(MacroCapability::FsWrite)),
    Quit => return Some(CapabilityPolicy::Forbidden(MacroCapability::Process)),
    _ => return None,
  };
  Some(CapabilityPolicy::Requires(required))
}

fn syntax_policy(syntax: &CalcitSyntax) -> Option<CapabilityPolicy> {
  match syntax {
    CalcitSyntax::Eval => Some(CapabilityPolicy::Requires(MacroCapability::DynamicEval)),
    CalcitSyntax::Defatom | CalcitSyntax::Reset => Some(CapabilityPolicy::Requires(MacroCapability::MutableState)),
    _ => None,
  }
}

fn helper_chain(call_stack: &CallStackList) -> String {
  let chain = call_stack
    .0
    .iter()
    .filter(|frame| matches!(frame.kind, StackKind::Fn | StackKind::Macro | StackKind::Syntax | StackKind::Method))
    .map(|frame| format!("{}/{}", frame.ns, frame.def))
    .collect::<Vec<_>>();
  if chain.is_empty() {
    "(direct call)".to_owned()
  } else {
    chain.join(" -> ")
  }
}

fn check_policy(policy: CapabilityPolicy, operation: &str, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  let context = MACRO_EXECUTION_STACK.with(|stack| stack.borrow().last().cloned());
  let Some(context) = context else {
    return Ok(());
  };
  let capability = policy.capability();
  let (code, reason) = match policy {
    CapabilityPolicy::Forbidden(_) => (
      "E_MACRO_CAPABILITY_DISALLOWED",
      format!("compile-time capability :{} is disallowed by policy", capability.as_str()),
    ),
    CapabilityPolicy::Requires(_) if !context.declared.contains(&capability) => (
      "E_MACRO_CAPABILITY_MISSING",
      format!("compile-time capability :{} was not declared", capability.as_str()),
    ),
    CapabilityPolicy::Requires(_) => return Ok(()),
  };
  let message = format!(
    "macro `{}` cannot execute `{operation}`: {reason}\n  helper-chain: {}",
    context.macro_name,
    helper_chain(call_stack)
  );
  let mut error = CalcitErr::use_msg_stack_location_with_code(CalcitErrKind::Effect, message, code, call_stack, context.call_location);
  error.hint = Some(if capability.is_allowed() {
    format!(
      "Declare `:capabilities $ #{{}} :{}` on the strict Macro signature, or move the effect into generated runtime code.",
      capability.as_str()
    )
  } else {
    format!(
      "Compile-time :{} cannot be enabled. Move the operation into generated runtime code or remove the host effect.",
      capability.as_str()
    )
  });
  Err(error)
}

pub fn check_proc(proc: CalcitProc, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  match proc_policy(proc) {
    Some(policy) => check_policy(policy, proc.as_ref(), call_stack),
    None => Ok(()),
  }
}

pub fn check_syntax(syntax: &CalcitSyntax, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  match syntax_policy(syntax) {
    Some(policy) => check_policy(policy, syntax.as_ref(), call_stack),
    None => Ok(()),
  }
}

pub fn check_registered(alias: &str, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  check_policy(CapabilityPolicy::Forbidden(MacroCapability::HostFfi), alias, call_stack)
}

pub fn check_host_ffi(operation: &str, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  check_policy(CapabilityPolicy::Forbidden(MacroCapability::HostFfi), operation, call_stack)
}

pub fn check_method(name: &str, kind: &MethodKind, call_stack: &CallStackList) -> Result<(), CalcitErr> {
  if matches!(kind, MethodKind::Invoke(_) | MethodKind::TagAccess) {
    Ok(())
  } else {
    check_policy(
      CapabilityPolicy::Forbidden(MacroCapability::HostFfi),
      &format!(".{name}"),
      call_stack,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{Calcit, CalcitScope};

  #[test]
  fn pure_context_rejects_env_read() {
    let error = with_macro_context(Arc::from("app/read-env"), Arc::new(HashSet::new()), None, || {
      check_proc(CalcitProc::GetEnv, &CallStackList::default())
    })
    .expect_err("env read must need a declaration");
    assert_eq!(error.code(), Some("E_MACRO_CAPABILITY_MISSING"));
    assert!(error.msg.contains(":env-read"));
  }

  #[test]
  fn declared_env_read_is_allowed() {
    let declared = Arc::new(HashSet::from([MacroCapability::EnvRead]));
    with_macro_context(Arc::from("app/read-env"), declared, None, || {
      check_proc(CalcitProc::GetEnv, &CallStackList::default())
    })
    .expect("declared env read should pass");
  }

  #[test]
  fn buf_list_procedures_require_mutable_state() {
    for proc in [
      CalcitProc::NativeBufListNew,
      CalcitProc::NativeBufListPush,
      CalcitProc::NativeBufListConcat,
    ] {
      let error = with_macro_context(Arc::from("app/mutate-buffer"), Arc::new(HashSet::new()), None, || {
        check_proc(proc, &CallStackList::default())
      })
      .expect_err("mutable BufList procedures must need a declaration");
      assert_eq!(error.code(), Some("E_MACRO_CAPABILITY_MISSING"));
      assert!(error.msg.contains(":mutable-state"));
    }
  }

  #[test]
  fn dangerous_capability_stays_forbidden_when_declared() {
    let declared = Arc::new(HashSet::from([MacroCapability::FsWrite]));
    let error = with_macro_context(Arc::from("app/write"), declared, None, || {
      check_proc(CalcitProc::WriteFile, &CallStackList::default())
    })
    .expect_err("file writes are not an opt-in escape hatch");
    assert_eq!(error.code(), Some("E_MACRO_CAPABILITY_DISALLOWED"));
  }

  #[test]
  fn generated_runtime_effect_syntax_remains_pure() {
    let emitted_runtime_call = Calcit::from(vec![Calcit::Proc(CalcitProc::GetEnv), Calcit::new_str("MODE")]);
    let quoted = Calcit::from(vec![
      Calcit::Syntax(CalcitSyntax::Quote, Arc::from("tests.capability")),
      emitted_runtime_call,
    ]);
    with_macro_context(Arc::from("tests.capability/emit-env"), Arc::new(HashSet::new()), None, || {
      crate::runner::evaluate_expr(&quoted, &CalcitScope::default(), "tests.capability", &CallStackList::default())
    })
    .expect("quoting a runtime effect must not perform that effect during expansion");
  }

  #[test]
  fn diagnostic_keeps_transitive_helper_chain() {
    let stack = CallStackList::default()
      .extend("tests.capability", "outer-macro", StackKind::Macro, &Calcit::Nil, &[])
      .extend("tests.capability", "read-mode-helper", StackKind::Fn, &Calcit::Nil, &[]);
    let error = with_macro_context(Arc::from("tests.capability/outer-macro"), Arc::new(HashSet::new()), None, || {
      check_proc(CalcitProc::GetEnv, &stack)
    })
    .expect_err("transitive helper effects must be checked");
    assert!(error.msg.contains("read-mode-helper"));
    assert!(error.msg.contains("outer-macro"));
  }
}

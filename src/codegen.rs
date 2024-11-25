use std::sync::atomic::AtomicBool;

pub mod emit_js;
pub mod gen_ir;

/// switch whether in codegen mode
static CODEGEN_MODE: AtomicBool = AtomicBool::new(true);

static CODEGEN_SKIP_ARITY_CHECK: AtomicBool = AtomicBool::new(false);

pub const COMPILE_ERRORS_FILE: &str = "calcit.build-errors";

pub fn codegen_mode() -> bool {
  CODEGEN_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// defaults to `true``
pub fn set_codegen_mode(b: bool) {
  CODEGEN_MODE.store(b, std::sync::atomic::Ordering::Relaxed);
}

/// whether to disable arity check in js codegen
pub fn set_code_gen_skip_arity_check(b: bool) {
  if b {
    println!("WARN: skip arity check in js codegen")
  }
  CODEGEN_SKIP_ARITY_CHECK.store(b, std::sync::atomic::Ordering::Relaxed)
}

/// read global flag for skipping arity check
pub fn skip_arity_check() -> bool {
  CODEGEN_SKIP_ARITY_CHECK.load(std::sync::atomic::Ordering::Relaxed)
}

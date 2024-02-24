use std::sync::atomic::AtomicBool;

pub mod emit_js;
pub mod gen_ir;

lazy_static! {
  /// switch whether in codegen mode
  static ref CODEGEN_MODE: AtomicBool = AtomicBool::new(true);
}

pub const COMPILE_ERRORS_FILE: &str = "calcit.build-errors";

pub fn codegen_mode() -> bool {
  CODEGEN_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// defaults to `true``
pub fn set_codegen_mode(b: bool) {
  CODEGEN_MODE.store(b, std::sync::atomic::Ordering::Relaxed);
}

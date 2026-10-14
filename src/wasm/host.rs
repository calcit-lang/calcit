//! Host facade: the only place under `src/wasm/` that names main-crate internals.
//!
//! Every other file in this directory reaches `calcit`, `runner::preprocess`, `builtins::syntax`,
//! `program`, `util` and friends through `crate::wasm::host::…`, so the dependency surface of the
//! WASM backend and FFI ABI modules is visible in one list. `wasm::boundary_tests` rejects any
//! other `crate::<internal module>` reference in this directory.
//!
//! Items needed with and without the `wasm` feature are re-exported unconditionally; the rest is
//! gated so a no-WASM build does not carry unused imports.

// Used by the FFI interface IR, which compiles without the `wasm` feature.
pub(crate) use crate::{calcit, data, snapshot};

// Used by the code generator and CLI driver only.
#[cfg(feature = "wasm")]
pub(crate) use crate::{
  ProgramEntries, builtins, call_stack, codegen, js_ffi_source, load_core_snapshot, load_module, merge_project_module_files, program,
  project_module_folder, runner, util, validate_snapshot_path,
};

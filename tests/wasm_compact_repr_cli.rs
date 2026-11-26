#![cfg(feature = "wasm")]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use wasmparser::{DataKind, ExternalKind, Operator, Parser, Payload, TypeRef, ValType};
use wasmtime::{Engine, ExternType, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val as CoreVal};

const HEAP_MAGIC: i32 = 0xCA1C_17A9u32 as i32;
const EIGHT_MIB: usize = 8 * 1024 * 1024;
const SEMI_SPACE_BYTES: i32 = 1024 * 1024;

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create(label: &str) -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-{label}-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary test directory should create");
    Self(path)
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn compile_wasm_with_boundary(dir: &TestDirectory, source: &str, boundary: &str) -> PathBuf {
  let snapshot_path = dir.0.join("calcit.cirru");
  fs::write(&snapshot_path, source).expect("test snapshot should write");
  let out_dir = dir.0.join("out");
  fs::create_dir(&out_dir).expect("wasm output directory should create");
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["wasm", "--boundary", boundary, "--emit-path"])
    .arg(&out_dir)
    .output()
    .expect("WASM compilation command should run");
  assert!(
    compile.status.success(),
    "WASM ({boundary}) compilation failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );
  out_dir.join("program.wasm")
}

fn instantiate_core_with_limit(wasm_path: &Path, max_memory_bytes: usize) -> (Store<StoreLimits>, Instance) {
  let engine = Engine::default();
  let module = Module::from_file(&engine, wasm_path).expect("generated Core WASM module should load");
  let mut linker = Linker::new(&engine);
  let mut registered = HashSet::new();
  for import in module.imports() {
    let key = (import.module().to_owned(), import.name().to_owned());
    if !registered.insert(key) {
      continue;
    }
    if let ExternType::Func(func_ty) = import.ty() {
      let result_tys: Vec<_> = func_ty.results().collect();
      linker
        .func_new(import.module(), import.name(), func_ty, move |_caller, _params, results| {
          for (slot, ty) in results.iter_mut().zip(&result_tys) {
            *slot = match ty {
              wasmtime::ValType::I32 => CoreVal::I32(0),
              wasmtime::ValType::I64 => CoreVal::I64(0),
              wasmtime::ValType::F32 => CoreVal::F32(0),
              wasmtime::ValType::F64 => CoreVal::F64(0.0f64.to_bits()),
              _ => CoreVal::I32(0),
            };
          }
          Ok(())
        })
        .expect("stub import should define");
    }
  }
  let limits = StoreLimitsBuilder::new().memory_size(max_memory_bytes).build();
  let mut store = Store::new(&engine, limits);
  store.limiter(|state| state);
  let instance = linker
    .instantiate(&mut store, &module)
    .expect("Core WASM module should instantiate");
  (store, instance)
}

fn get_i32_global(store: &mut Store<StoreLimits>, instance: &Instance, name: &str) -> i32 {
  instance
    .get_global(&mut *store, name)
    .unwrap_or_else(|| panic!("expected global export `{name}`"))
    .get(&mut *store)
    .i32()
    .unwrap_or_else(|| panic!("global `{name}` should be i32"))
}

struct ParsedFunction {
  params: Vec<ValType>,
  results: Vec<ValType>,
  locals: Vec<ValType>,
  calls: Vec<u32>,
  i32_consts: Vec<i32>,
  has_call_indirect: bool,
}

struct ParsedModule {
  num_func_imports: u32,
  type_count: usize,
  exports: HashMap<String, u32>,
  functions: HashMap<u32, ParsedFunction>,
  data_offsets: Vec<i32>,
  data_bytes: usize,
}

fn parse_wasm_module(bytes: &[u8]) -> ParsedModule {
  wasmparser::Validator::new()
    .validate_all(bytes)
    .expect("generated WASM module must validate cleanly");

  let mut type_sigs: Vec<(Vec<ValType>, Vec<ValType>)> = Vec::new();
  let mut num_func_imports = 0u32;
  let mut fn_type_indices = Vec::new();
  let mut exports = HashMap::new();
  let mut bodies = Vec::new();
  let mut data_offsets = Vec::new();
  let mut data_bytes = 0usize;

  for payload in Parser::new(0).parse_all(bytes) {
    match payload.expect("valid wasm payload") {
      Payload::TypeSection(section) => {
        for sig in section.into_iter_err_on_gc_types() {
          let sig = sig.expect("valid core type");
          type_sigs.push((sig.params().to_vec(), sig.results().to_vec()));
        }
      }
      Payload::ImportSection(section) => {
        for import in section.into_imports() {
          let import = import.expect("valid import");
          if matches!(import.ty, TypeRef::Func(_)) {
            num_func_imports += 1;
          }
        }
      }
      Payload::FunctionSection(section) => {
        for type_idx in section {
          fn_type_indices.push(type_idx.expect("valid function type index"));
        }
      }
      Payload::ExportSection(section) => {
        for export in section {
          let export = export.expect("valid export");
          if export.kind == ExternalKind::Func {
            exports.insert(export.name.to_owned(), export.index);
          }
        }
      }
      Payload::CodeSectionEntry(body) => {
        bodies.push(body);
      }
      Payload::DataSection(section) => {
        for segment in section {
          let segment = segment.expect("valid data segment");
          data_bytes += segment.data.len();
          if let DataKind::Active { offset_expr, .. } = segment.kind {
            let mut ops = offset_expr.get_operators_reader();
            if let Ok(Operator::I32Const { value }) = ops.read() {
              data_offsets.push(value);
            }
          }
        }
      }
      _ => {}
    }
  }

  assert_eq!(fn_type_indices.len(), bodies.len(), "function/code section lengths must match");
  let mut functions = HashMap::new();
  for (pos, (type_idx, body)) in fn_type_indices.into_iter().zip(bodies).enumerate() {
    let fn_index = num_func_imports + pos as u32;
    let (params, results) = type_sigs[type_idx as usize].clone();
    let mut locals = Vec::new();
    let locals_reader = body.get_locals_reader().expect("valid locals");
    for entry in locals_reader {
      let (count, val_type) = entry.expect("valid local entry");
      for _ in 0..count {
        locals.push(val_type);
      }
    }
    let mut calls = Vec::new();
    let mut i32_consts = Vec::new();
    let mut has_call_indirect = false;
    let mut ops = body.get_operators_reader().expect("valid operators");
    while !ops.eof() {
      match ops.read().expect("valid wasm operator") {
        Operator::Call { function_index } => calls.push(function_index),
        Operator::I32Const { value } => i32_consts.push(value),
        Operator::CallIndirect { .. } => has_call_indirect = true,
        _ => {}
      }
    }
    functions.insert(
      fn_index,
      ParsedFunction {
        params,
        results,
        locals,
        calls,
        i32_consts,
        has_call_indirect,
      },
    );
  }

  ParsedModule {
    num_func_imports,
    type_count: type_sigs.len(),
    exports,
    functions,
    data_offsets,
    data_bytes,
  }
}

#[test]
fn wasmparser_verifies_unboxed_signatures_locals_and_zero_heap_probes_on_closed_paths() {
  let dir = TestDirectory::create("wasm-compact-sigs");
  let snapshot = r#"
{} (:package |compact-sigs)
  :about "|Compact scalar/heap signatures test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-sigs.main/main!) (:mode :native) (:reload-fn 'compact-sigs.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-sigs.main
    %{} 'FileEntry
      :defs $ {}
        'Packet $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Packet (:u Unit) (:b Bool) (:t Tag) (:i Int32) (:big Int64)
          :examples $ []
          :schema $ quote StructDef
        'flip-bool $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn flip-bool (b)
            let ((inv (not b)))
              if inv false true
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
        'step-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn step-tag (t)
            let ((next (if (= t :alpha) :beta :alpha)))
              , next
          :examples $ []
          :schema $ quote $ Fn Tag (-> Tag)
        'keep-unit $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn keep-unit (u)
            let ((copy u))
              if (= u copy) copy &unit
          :examples $ []
          :schema $ quote $ Fn Unit (-> Unit)
        'pick-i32 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn pick-i32 (x)
            let ((alt (if (= x (assert-type 10 Int32)) (assert-type 42 Int32) x)))
              , alt
          :examples $ []
          :schema $ quote $ Fn Int32 (-> Int32)
        'pick-u64 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn pick-u64 (a b)
            let ((chosen (if (= a (assert-type 9 UInt64)) a (if (= b (assert-type 0 Int64)) (assert-type 55 UInt64) a))))
              , chosen
          :examples $ []
          :schema $ quote $ Fn UInt64 Int64 (-> UInt64)
        'check-packet $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-packet (p)
            let
                b (:b p)
                t (:t p)
                u (:u p)
                i (:i p)
              and b (= t :beta) (= u &unit) (identical? i (assert-type 10 Int32))
          :examples $ []
          :schema $ quote $ Fn Packet (-> Bool)
        'check-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-str (s)
            if (.empty? s) false (= (&str:count s) 5)
          :examples $ []
          :schema $ quote $ Fn String (-> Bool)
        'run-closed-inner $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn run-closed-inner (p s-hi s-empty)
            let
                b-ok $ not (flip-bool false)
                t-ok $ = (step-tag :alpha) :beta
                u-ok $ = (keep-unit &unit) &unit
                i-ok $ = (pick-i32 (assert-type 10 Int32)) (assert-type 42 Int32)
                u64-ok $ = (pick-u64 (assert-type 9 UInt64) (assert-type -1 Int64)) (assert-type 9 UInt64)
                p-ok $ check-packet p
                s-ok $ and (check-str s-hi) (not (check-str s-empty))
                big-ok $ = (:big p) (assert-type 9007199254740991 Int64)
              and b-ok t-ok u-ok i-ok u64-ok p-ok s-ok big-ok
          :examples $ []
          :schema $ quote $ Fn Packet String String (-> Bool)
        'run-closed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export run-closed ()
            let ((p (Packet :u &unit :b true :t :beta :i (assert-type 10 Int32) :big (assert-type 9007199254740991 Int64))))
              run-closed-inner p |hello |
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (run-closed) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-sigs.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let parsed = parse_wasm_module(&bytes);

  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let run_closed = instance
    .get_typed_func::<(), f64>(&mut store, "run-closed")
    .expect("run-closed export");
  assert_eq!(run_closed.call(&mut store, ()).expect("run-closed executes"), 1.0);

  let is_heap_ptr_fns: HashSet<u32> = parsed
    .functions
    .iter()
    .filter(|(_, f)| {
      f.params.as_slice() == [ValType::F64] && f.results.as_slice() == [ValType::I32] && f.i32_consts.contains(&HEAP_MAGIC)
    })
    .map(|(idx, _)| *idx)
    .collect();
  assert!(
    is_heap_ptr_fns.is_empty(),
    "closed module without heap probes should prune unused __rt_is_heap_ptr, found {is_heap_ptr_fns:?}"
  );

  let export_idx = *parsed.exports.get("run-closed").expect("run-closed export index");
  let export_fn = &parsed.functions[&export_idx];
  let wrapper_targets: Vec<u32> = export_fn
    .calls
    .iter()
    .copied()
    .filter(|callee| {
      parsed
        .functions
        .get(callee)
        .is_some_and(|f| f.params.is_empty() && f.results.as_slice() == [ValType::F64])
    })
    .collect();
  assert_eq!(wrapper_targets.len(), 1, "export wrapper must target run-closed");

  let inner_calls: Vec<u32> = parsed.functions[&wrapper_targets[0]]
    .calls
    .iter()
    .copied()
    .filter(|callee| {
      parsed
        .functions
        .get(callee)
        .is_some_and(|f| f.params.as_slice() == [ValType::I32, ValType::I32, ValType::I32] && f.results.as_slice() == [ValType::I32])
    })
    .collect();
  let actual_calls: Vec<_> = parsed.functions[&wrapper_targets[0]]
    .calls
    .iter()
    .map(|callee| (*callee, parsed.functions.get(callee).map(|f| (&f.params, &f.results))))
    .collect();
  assert!(
    !inner_calls.is_empty(),
    "run-closed must directly call run-closed-inner with unboxed (i32, i32, i32)->i32 signature; actual calls: {actual_calls:?}"
  );
  let inner_idx = inner_calls[0];
  let mut unique_callees: Vec<u32> = Vec::new();
  for &callee in &parsed.functions[&inner_idx].calls {
    if !unique_callees.contains(&callee) {
      unique_callees.push(callee);
    }
  }
  assert_eq!(
    unique_callees.len(),
    7,
    "run-closed-inner must call only the 7 named helpers in source order: {unique_callees:?}"
  );
  struct HelperSpec {
    name: &'static str,
    params: &'static [ValType],
    results: &'static [ValType],
    local: Option<ValType>,
  }
  let expected_sigs = [
    HelperSpec {
      name: "flip-bool",
      params: &[ValType::I32],
      results: &[ValType::I32],
      local: Some(ValType::I32),
    },
    HelperSpec {
      name: "step-tag",
      params: &[ValType::I32],
      results: &[ValType::I32],
      local: Some(ValType::I32),
    },
    HelperSpec {
      name: "keep-unit",
      params: &[ValType::I32],
      results: &[ValType::I32],
      local: Some(ValType::I32),
    },
    HelperSpec {
      name: "pick-i32",
      params: &[ValType::I32],
      results: &[ValType::I32],
      local: Some(ValType::I32),
    },
    HelperSpec {
      name: "pick-u64",
      params: &[ValType::I64, ValType::I64],
      results: &[ValType::I64],
      local: Some(ValType::I64),
    },
    HelperSpec {
      name: "check-packet",
      params: &[ValType::I32],
      results: &[ValType::I32],
      local: Some(ValType::I32),
    },
    HelperSpec {
      name: "check-str",
      params: &[ValType::I32],
      results: &[ValType::I32],
      local: None,
    },
  ];
  for (fn_idx, spec) in unique_callees.iter().zip(expected_sigs) {
    let helper = &parsed.functions[fn_idx];
    assert_eq!(helper.params.as_slice(), spec.params, "{} params must be unboxed", spec.name);
    assert_eq!(helper.results.as_slice(), spec.results, "{} result must be unboxed", spec.name);
    if let Some(vt) = spec.local {
      assert!(helper.locals.contains(&vt), "{} must allocate unboxed local {vt:?}", spec.name);
    }
    assert!(
      helper.calls.iter().all(|c| !is_heap_ptr_fns.contains(c)),
      "closed helper {} must not call __rt_is_heap_ptr",
      spec.name
    );
    assert!(
      !helper.i32_consts.contains(&HEAP_MAGIC),
      "non-allocating closed helper {} must not embed HEAP_MAGIC probes",
      spec.name
    );
  }
}

#[test]
fn table_adapters_bridge_native_signature_functions_via_call_indirect() {
  let dir = TestDirectory::create("wasm-compact-indirect");
  let snapshot = r#"
{} (:package |compact-indirect)
  :about "|Compact table adapters test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-indirect.main/main!) (:mode :native) (:reload-fn 'compact-indirect.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-indirect.main
    %{} 'FileEntry
      :defs $ {}
        'Item $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Item (:tag Tag) (:code Int32)
          :examples $ []
          :schema $ quote StructDef
        'flip $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn flip (b) (not b)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
        'keep $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn keep (b) b
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
        'is-alpha? $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn is-alpha? (t) (= t :alpha)
          :examples $ []
          :schema $ quote $ Fn Tag (-> Bool)
        'is-beta? $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn is-beta? (t) (= t :beta)
          :examples $ []
          :schema $ quote $ Fn Tag (-> Bool)
        'add-big $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn add-big (a b)
            if (= b (assert-type 0 Int64)) a (assert-type 9007199254740991 Int64)
          :examples $ []
          :schema $ quote $ Fn Int64 Int64 (-> Int64)
        'sub-big $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn sub-big (a b)
            if (= a b) (assert-type 0 Int64) a
          :examples $ []
          :schema $ quote $ Fn Int64 Int64 (-> Int64)
        'item-code $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn item-code (it) (:code it)
          :examples $ []
          :schema $ quote $ Fn Item (-> Int32)
        'other-code $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn other-code (it) (if (= (:tag it) :alpha) (assert-type 99 Int32) (:code it))
          :examples $ []
          :schema $ quote $ Fn Item (-> Int32)
        'call-indirect-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export call-indirect-all (idx)
            let
                bool-fns $ [] flip keep
                tag-fns $ [] is-alpha? is-beta?
                big-fns $ [] add-big sub-big
                item-fns $ [] item-code other-code
                b-fn $ &list:nth bool-fns idx
                t-fn $ &list:nth tag-fns idx
                i-fn $ &list:nth big-fns idx
                s-fn $ &list:nth item-fns idx
                it $ Item :tag :alpha :code (assert-type 17 Int32)
                b-res $ b-fn true
                t-res $ t-fn :alpha
                i-res $ i-fn (assert-type 42 Int64) (assert-type 1 Int64)
                s-res $ s-fn it
              if (= idx 0)
                if (and (not b-res) t-res (= i-res (assert-type 9007199254740991 Int64)) (= s-res (assert-type 17 Int32))) 1 0
                if (and b-res (not t-res) (= i-res (assert-type 42 Int64)) (= s-res (assert-type 99 Int32))) 2 0
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (call-indirect-all 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-indirect.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let parsed = parse_wasm_module(&bytes);
  assert!(
    !parsed.functions.values().any(|f| f.has_call_indirect),
    "native-signature closure dispatch must use BrTable trampolines without CallIndirect instructions"
  );

  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let call_indirect = instance
    .get_typed_func::<f64, f64>(&mut store, "call-indirect-all")
    .expect("call-indirect-all export");
  assert_eq!(call_indirect.call(&mut store, 0.0).expect("idx 0 succeeds"), 1.0);
  assert_eq!(call_indirect.call(&mut store, 1.0).expect("idx 1 succeeds"), 2.0);
}

#[test]
fn boxed_json_scalar_edge_cases_preserve_distinction_alongside_compact_paths() {
  let dir = TestDirectory::create("wasm-compact-json");
  let snapshot = r#"
{} (:package |compact-json)
  :about "|Boxed JSON scalars alongside compact paths test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-json.main/main!) (:mode :native) (:reload-fn 'compact-json.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-json.main
    %{} 'FileEntry
      :defs $ {}
        'ClosedBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ClosedBox (:flag Bool) (:kind Tag)
          :examples $ []
          :schema $ quote StructDef
        'flip-closed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn flip-closed (b) (not b)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Bool)
        'verify-json-and-closed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-json-and-closed ()
            let
                parsed $ &try-parse-json Result "|[false,0,null,true,1]"
              match parsed
                (:err _) false
                (:ok raw)
                  let
                      xs $ decode-map-as raw (List Dynamic)
                      j-false $ assert-type (&list:nth xs 0) Bool
                      j-zero $ assert-type (&list:nth xs 1) Number
                      j-null $ assert-type (&list:nth xs 2) Nil
                      j-true $ assert-type (&list:nth xs 3) Bool
                      j-one $ assert-type (&list:nth xs 4) Number
                      closed $ ClosedBox :flag (flip-closed false) :kind :alpha
                      flag-v $ :flag closed
                    and
                      &= j-false false
                      not (&= j-false 0)
                      not (&= j-false nil)
                      &= j-zero 0
                      not (&= j-zero false)
                      &= j-null nil
                      nil? j-null
                      not (nil? j-false)
                      not (nil? j-zero)
                      &= j-true true
                      not (&= j-true 1)
                      &= j-one 1
                      , flag-v
                      = (:kind closed) :alpha
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (verify-json-and-closed) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-json.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let verify = instance
    .get_typed_func::<(), f64>(&mut store, "verify-json-and-closed")
    .expect("verify-json-and-closed export");
  assert_eq!(verify.call(&mut store, ()).expect("verify executes"), 1.0);
}

#[test]
fn compact_struct_and_enum_footprint_benchmark_and_defref_compaction() {
  let size_dir = TestDirectory::create("wasm-compact-sizes");
  let size_snapshot = r#"
{} (:package |compact-sizes)
  :about "|Exact single-object Scratch footprint without defref root nodes"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-sizes.main/main!) (:mode :native) (:reload-fn 'compact-sizes.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-sizes.main
    %{} 'FileEntry
      :defs $ {}
        'EmptyBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct EmptyBox
          :examples $ []
          :schema $ quote StructDef
        'CompactFlag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct CompactFlag (:u Unit) (:b Bool) (:t Tag) (:small UInt8)
          :examples $ []
          :schema $ quote StructDef
        'CompactRecord $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct CompactRecord (:u Unit) (:b1 Bool) (:b2 Bool) (:i8 Int8) (:u16 UInt16) (:t Tag) (:s String) (:n Number) (:u64 UInt64)
          :examples $ []
          :schema $ quote StructDef
        'make-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-empty () (EmptyBox)
          :examples $ []
          :schema $ quote $ Fn (-> EmptyBox)
        'make-flag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-flag () (CompactFlag :u &unit :b true :t :ready :small (assert-type 7 UInt8))
          :examples $ []
          :schema $ quote $ Fn (-> CompactFlag)
        'make-record $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-record ()
            CompactRecord :u &unit :b1 true :b2 false :i8 (assert-type -5 Int8) :u16 (assert-type 512 UInt16) :t :active :s |label :n 12.5 :u64 (assert-type 9007199254740991 UInt64)
          :examples $ []
          :schema $ quote $ Fn (-> CompactRecord)
        'make-none $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-none () , None
          :examples $ []
          :schema $ quote $ Fn (-> (Option Number))
        'make-some $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-some () (Some 42)
          :examples $ []
          :schema $ quote $ Fn (-> (Option Number))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-sizes.main
"#;
  let size_wasm_path = compile_wasm_with_boundary(&size_dir, size_snapshot, "native");
  let _ = parse_wasm_module(&fs::read(&size_wasm_path).expect("size wasm reads"));
  let (mut size_store, size_instance) = instantiate_core_with_limit(&size_wasm_path, EIGHT_MIB);
  let size_reset = size_instance
    .get_typed_func::<(), ()>(&mut size_store, "__arena_reset")
    .expect("__arena_reset");

  let mut measure_single = |name: &str| -> i32 {
    size_reset.call(&mut size_store, ()).expect("arena reset");
    let before = get_i32_global(&mut size_store, &size_instance, "__heap_ptr");
    let f = size_instance.get_typed_func::<(), f64>(&mut size_store, name).expect("export fn");
    let _ptr = f.call(&mut size_store, ()).expect("export executes");
    let after = get_i32_global(&mut size_store, &size_instance, "__heap_ptr");
    after - before
  };

  // Single-object footprint including 8-byte [HEAP_MAGIC:u32][type_tag:u32] GC header:
  let empty_bytes = measure_single("make-empty");
  let flag_bytes = measure_single("make-flag");
  let record_bytes = measure_single("make-record");
  let none_bytes = measure_single("make-none");
  let some_bytes = measure_single("make-some");

  assert_eq!(empty_bytes, 16, "EmptyBox: 8 GC + 8 header = 16 B (legacy 24 B)");
  assert_eq!(
    flag_bytes, 24,
    "CompactFlag (4 typed fields): 8 GC + 16 compact = 24 B (legacy 56 B)"
  );
  assert_eq!(
    record_bytes, 48,
    "CompactRecord (9 mixed fields): 8 GC + 40 compact = 48 B (legacy 96 B)"
  );
  assert_eq!(none_bytes, 16, "Option :none: 8 GC + 8 header = 16 B (legacy 24 B)");
  assert_eq!(some_bytes, 24, "Option :some: 8 GC + 8 header + 8 payload = 24 B (legacy 32 B)");

  let dir = TestDirectory::create("wasm-compact-bench");
  let snapshot = r#"
{} (:package |compact-bench)
  :about "|Compact Struct/Enum footprint benchmark"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-bench.main/main!) (:mode :native) (:reload-fn 'compact-bench.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-bench.main
    %{} 'FileEntry
      :defs $ {}
        'CompactFlag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct CompactFlag (:u Unit) (:b Bool) (:t Tag) (:small UInt8)
          :examples $ []
          :schema $ quote StructDef
        'FlagNode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct FlagNode (:item CompactFlag) (:next (Option compact-bench.main/FlagNode))
          :examples $ []
          :schema $ quote StructDef
        '*persisted $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *persisted $ []
          :examples $ []
          :schema $ quote $ Ref (List (Option compact-bench.main/FlagNode))
        '*churn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *churn |seed
          :examples $ []
          :schema $ quote $ Ref String
        'build-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn build-chain (outer per-chain)
            loop ((i 0) (acc None))
              if (= i per-chain)
                , acc
                let
                    f $ CompactFlag :u &unit :b (= (&number:rem (&+ outer i) 2) 0) :t :ready :small (assert-type 9 UInt8)
                  recur (&+ i 1) (Some (FlagNode :item f :next acc))
          :examples $ []
          :schema $ quote $ Fn Number Number (-> (Option FlagNode))
        'count-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn count-chain (outer per-chain start)
            loop ((cur start) (cnt 0))
              match cur
                (:none) cnt
                (:some node)
                  let
                      item (:item node)
                      expected-b (= (&number:rem (&+ outer (&- (&- per-chain 1) cnt)) 2) 0)
                    if (and (= (:u item) &unit) (= (:b item) expected-b) (= (:t item) :ready) (= (:small item) (assert-type 9 UInt8)))
                      recur (:next node) (&+ cnt 1)
                      , -1
          :examples $ []
          :schema $ quote $ Fn Number Number (Option FlagNode) (-> Number)
        'alloc-compact-scratch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-compact-scratch (chains per-chain)
            loop ((outer 0) (lists $ []))
              if (= outer chains)
                , lists
                recur (&+ outer 1) (append lists (build-chain outer per-chain))
          :examples $ []
          :schema $ quote $ Fn Number Number (-> (List (Option FlagNode)))
        'alloc-compact-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-compact-chain (chains per-chain)
            loop ((outer 0) (lists $ []))
              if (= outer chains)
                do (reset! *persisted lists) (&list:count lists)
                recur (&+ outer 1) (append lists (build-chain outer per-chain))
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'pump-churn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-churn (n)
            reset! *churn (&str:pad-right (&str:concat |churn- (to-string n)) 32768 |Z)
            &str:count @*churn
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'verify-persisted-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-persisted-chain (expected-total per-chain)
            loop ((items @*persisted) (idx 0) (total 0))
              if (= idx (&list:count items))
                = total expected-total
                let ((n (count-chain idx per-chain (&list:nth items idx))))
                  if (< n 0)
                    , false
                    recur items (&+ idx 1) (&+ total n)
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-bench.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset");

  // Measure Scratch Arena footprint using an export that returns the Scratch-backed graph directly before reset.
  arena_reset.call(&mut store, ()).expect("arena reset");
  let scratch_before = get_i32_global(&mut store, &instance, "__heap_ptr");
  let alloc_scratch = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "alloc-compact-scratch")
    .expect("alloc-compact-scratch");
  let _ = alloc_scratch
    .call(&mut store, (20.0, 50.0))
    .expect("alloc 1000 scratch compact nodes");
  let scratch_after = get_i32_global(&mut store, &instance, "__heap_ptr");
  arena_reset.call(&mut store, ()).expect("arena reset");

  // Separately measure promoted persistent footprint across 1,000 linked nodes in 20 chains
  // (keeping per-chain depth below MAX_PROMOTE_DEPTH = 256):
  let pers_before = get_i32_global(&mut store, &instance, "__persistent_ptr");
  let alloc_chain = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "alloc-compact-chain")
    .expect("alloc-compact-chain");
  assert_eq!(alloc_chain.call(&mut store, (20.0, 50.0)).expect("alloc 1000 compact nodes"), 20.0);
  let pers_after = get_i32_global(&mut store, &instance, "__persistent_ptr");
  arena_reset.call(&mut store, ()).expect("arena reset");

  let scratch_used = scratch_after - scratch_before;
  let pers_used = pers_after - pers_before;
  // Per node: FlagNode (8 GC + 8 header + 4 item + 4 Option next = 24 B) + Some (24 B) + CompactFlag (24 B) = 72 B
  // plus 20 None terminals (320 B) and the 20-element List allocation (176 B) = 72,496 B.
  let legacy_1000_bytes = 1000 * (40 + 32 + 56) + 24;
  eprintln!(
    "[wasm compact benchmark] 1000*(CompactFlag+FlagNode+Some): scratch={scratch_used} B, persistent={pers_used} B (legacy={legacy_1000_bytes} B, empty={empty_bytes} B, flag={flag_bytes} B, record={record_bytes} B, none={none_bytes} B, some={some_bytes} B)"
  );
  assert!(
    pers_used <= 73_500 && pers_used < legacy_1000_bytes * 3 / 5,
    "compact persistent graph ({pers_used} B) must stay <= 73500 B and substantially smaller than legacy ({legacy_1000_bytes} B)"
  );
  assert!(
    (72 * 1000..=72 * 1000 + 8192).contains(&scratch_used),
    "unexpected live scratch footprint: {scratch_used}"
  );

  // Trigger persistent semi-space compaction into Semi-Space B and verify every node remains valid.
  let pump_churn = instance.get_typed_func::<f64, f64>(&mut store, "pump-churn").expect("pump-churn");
  let mut compacted = false;
  for i in 1..=25 {
    pump_churn.call(&mut store, i as f64).expect("pump churn");
    if get_i32_global(&mut store, &instance, "__persistent_ptr") >= space_b_base {
      compacted = true;
      break;
    }
  }
  assert!(compacted, "expected persistent heap compaction into semi-space B");

  let verify_chain = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "verify-persisted-chain")
    .expect("verify-persisted-chain");
  assert_eq!(
    verify_chain.call(&mut store, (1000.0, 50.0)).expect("verify persisted chain"),
    1.0,
    "all 1000 compact structs and enums (including alternating :b parity) must remain intact after defref semi-space compaction"
  );
}

#[test]
fn compact_struct_enum_semantics_equality_hashing_to_map_update_and_codecs() {
  let dir = TestDirectory::create("wasm-compact-semantics");
  let snapshot = r#"
{} (:package |compact-semantics)
  :about "|Compact Struct/Enum semantics and codecs test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-semantics.main/main!) (:mode :native) (:reload-fn 'compact-semantics.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-semantics.main
    %{} 'FileEntry
      :defs $ {}
        'UnitBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct UnitBox (:b Bool) (:u Unit)
          :examples $ []
          :schema $ quote StructDef
        'ItemKey $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ItemKey (:b Bool) (:t Tag) (:n Int16) (:label String)
          :examples $ []
          :schema $ quote StructDef
        'Status $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Status (:Idle) (:Busy Int32 String)
          :examples $ []
          :schema $ quote EnumDef
        'run-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export run-all ()
            let
                ub1 $ UnitBox :b true :u &unit
                ub2 $ &struct:assoc-at ub1 0 :b false
                k1 $ ItemKey :b true :t :alpha :n (assert-type 12 Int16) :label |v1
                k1-dup $ ItemKey :b true :t :alpha :n (assert-type 12 Int16) :label (&str:concat |v |1)
                k1-other $ ItemKey :b false :t :alpha :n (assert-type 12 Int16) :label |v1
                k-updated $ update k1 :label $ fn (s) (&str:concat s |-u)
                k-assoc $ &struct:assoc-at k1 0 :b false
                k-with $ &struct:with-at k1 0 :b false 2 :n (assert-type 99 Int16)
                e1 $ %:: Status :Busy (assert-type 5 Int32) |run
                e1-dup $ %:: Status :Busy (assert-type 5 Int32) (&str:concat |r |un)
                e0 $ %:: Status :Idle
                e-assoc $ &enum:assoc e1 2 |done
                m-struct $ &{} k1 e1
                m-enum $ &{} e0 100
                as-map $ .to-map k1
                edn-roundtrip $ try-parse-cirru-edn-as (format-cirru-edn k1) ItemKey
                map-decoded $ decode-map-as (encode-map-as k1 ItemKey) ItemKey
                eq-ok $ and (= k1 k1-dup) (not= k1 k1-other) (= e1 e1-dup) (not= e1 e0) (not= ub1 ub2)
                empty-k1? $ .empty? k1
                contains-k? $ &map:contains? m-struct k1-dup
                contains-e? $ &map:contains? m-enum e0
                struct-ops-ok $ and
                  = (:u ub1) &unit
                  = (:u ub2) &unit
                  = (:b ub2) false
                  = (&struct:count ub1) 2
                  = (:b k-assoc) false
                  = (:b k-with) false
                  = (:n k-with) (assert-type 99 Int16)
                  = (:label k-updated) |v1-u
                  = (&struct:count k1) 4
                  not empty-k1?
                enum-ops-ok $ and
                  = (assert-type (&enum:nth e0 0) Tag) :Idle
                  = (assert-type (&enum:nth e-assoc 2) String) |done
                  = (&enum:count e0) 1
                  = (&enum:count e1) 3
                hash-lookup-ok $ and
                  , contains-k?
                  , contains-e?
                  = (&map:get m-struct k1-dup) (Some e1-dup)
                  = (&map:get m-enum e0) (Some 100)
                codec-ok $ and
                  = (&map:count as-map) 4
                  &map:contains? as-map :t
                  &map:contains? as-map :n
                  match (get as-map :t)
                    (:some v) (= (assert-type v Tag) :alpha)
                    (:none) false
                  match (get as-map :n)
                    (:some v) (= (assert-type v Int16) (assert-type 12 Int16))
                    (:none) false
                  = edn-roundtrip (Result :ok k1)
                  = map-decoded k1
              and eq-ok struct-ops-ok enum-ops-ok hash-lookup-ok codec-ok
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (run-all) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-semantics.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let run_all = instance.get_typed_func::<(), f64>(&mut store, "run-all").expect("run-all export");
  assert_eq!(run_all.call(&mut store, ()).expect("run-all executes"), 1.0);
}

#[test]
fn component_abi_compact_struct_and_variant_round_trip() {
  let dir = TestDirectory::create("wasm-compact-component");
  let snapshot = r#"
{} (:package |compact-comp)
  :about "|Compact Component ABI test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-comp.main/main!) (:mode :native) (:reload-fn 'compact-comp.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-comp.main
    %{} 'FileEntry
      :defs $ {}
        'Mixed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Mixed (:u Unit) (:b Bool) (:i8 Int8) (:u16 UInt16) (:i32 Int32) (:u64 UInt64) (:label String)
          :examples $ []
          :schema $ quote StructDef
        'Action $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Action (:Noop) (:Apply compact-comp.main/Mixed)
          :examples $ []
          :schema $ quote EnumDef
        'echo-mixed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-mixed (m) m
          :examples $ []
          :schema $ quote $ Fn Mixed (-> Mixed)
        'echo-action $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export echo-action (act) act
          :examples $ []
          :schema $ quote $ Fn Action (-> Action)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-comp.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "component");
  let bytes = fs::read(&wasm_path).expect("component core wasm should read");
  let _ = parse_wasm_module(&bytes);

  let engine = Engine::default();
  let module = Module::new(&engine, &bytes).expect("component core module loads");
  let mut store = Store::new(&engine, ());
  let instance = Instance::new(&mut store, &module, &[]).expect("instantiate component module");
  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  let realloc = instance
    .get_typed_func::<(i32, i32, i32, i32), i32>(&mut store, "cabi_realloc")
    .expect("cabi_realloc");

  // Component ABI lowering writes the canonical result into retPtr and returns retPtr.
  // Canonical layout for Mixed (:u Unit, :b Bool, :i8 Int8, :u16 UInt16, :i32 Int32, :u64 UInt64, :label String):
  // fields sorted/padded by Canonical ABI:
  // Inspect component_wasm_cli / canonical offsets:
  // u64@8? Wait: Component record field order follows struct field order!
  // Let's write canonical fields in declared order:
  // u (0 bytes, align 1)
  // b (1 byte, align 1) -> @0
  // i8 (1 byte, align 1) -> @1
  // u16 (2 bytes, align 2) -> @2
  // i32 (4 bytes, align 4) -> @4
  // u64 (8 bytes, align 8) -> @8
  // label (ptr:i32 @16, len:i32 @20) -> size 24, align 8.
  // Alphabetical Mixed fields: (:b Bool), (:i32 Int32), (:i8 Int8), (:label String), (:u Unit), (:u16 UInt16), (:u64 UInt64).
  // Canonical layout: b@0(u8), i32@4(i32), i8@8(i8), label@12(ptr:i32,len:i32), u16@20(u16), u64@24(u64), size=32.
  // Flat params for echo-mixed: (b:i32, i32:i32, i8:i32, label_ptr:i32, label_len:i32, u16:i32, u64:i64).
  let label_bytes = b"compact";
  let label_ptr = realloc
    .call(&mut store, (0, 0, 1, label_bytes.len() as i32))
    .expect("realloc label");
  memory.write(&mut store, label_ptr as usize, label_bytes).expect("write label");

  // Lowered Action (`defenum Action` variants are sorted alphabetically: 0 = :Apply(Mixed), 1 = :Noop):
  // discriminant u8@0, Mixed@8..40 (total 40 bytes, align 8).
  let echo_action = instance
    .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32, i64), i32>(&mut store, "echo-action")
    .expect("echo-action export");
  let post_action = instance
    .get_typed_func::<i32, ()>(&mut store, "cabi_post_echo-action")
    .expect("cabi_post_echo-action");

  let ret = echo_action.call(&mut store, (1, 0, 0, 0, 0, 0, 0, 0)).expect("echo-action :Noop");
  let mut disc = [0u8; 1];
  memory.read(&store, ret as usize, &mut disc).expect("read :Noop discriminant");
  assert_eq!(disc[0], 1);
  post_action.call(&mut store, ret).expect("post-return :Noop");

  let ret_apply = echo_action
    .call(
      &mut store,
      (
        0,
        1,
        123456,
        -7,
        label_ptr,
        label_bytes.len() as i32,
        1025,
        9_007_199_254_740_991i64,
      ),
    )
    .expect("echo-action :Apply");
  let mut action_buf = [0u8; 40];
  memory.read(&store, ret_apply as usize, &mut action_buf).expect("read :Apply");
  assert_eq!(action_buf[0], 0);
  assert_eq!(action_buf[8], 1);
  assert_eq!(i32::from_le_bytes(action_buf[12..16].try_into().unwrap()), 123456);
  assert_eq!(action_buf[16] as i8, -7);
  let act_ptr = u32::from_le_bytes(action_buf[20..24].try_into().unwrap()) as usize;
  let act_len = u32::from_le_bytes(action_buf[24..28].try_into().unwrap()) as usize;
  let mut act_label = vec![0u8; act_len];
  memory.read(&store, act_ptr, &mut act_label).expect("read action label");
  assert_eq!(act_label, label_bytes);
  assert_eq!(u16::from_le_bytes(action_buf[28..30].try_into().unwrap()), 1025);
  assert_eq!(u64::from_le_bytes(action_buf[32..40].try_into().unwrap()), 9_007_199_254_740_991u64);
  post_action.call(&mut store, ret_apply).expect("post-return :Apply");

  let label_ptr2 = realloc
    .call(&mut store, (0, 0, 1, label_bytes.len() as i32))
    .expect("realloc label 2");
  memory.write(&mut store, label_ptr2 as usize, label_bytes).expect("write label 2");

  let echo_mixed = instance
    .get_typed_func::<(i32, i32, i32, i32, i32, i32, i64), i32>(&mut store, "echo-mixed")
    .expect("echo-mixed export");
  let post_mixed = instance
    .get_typed_func::<i32, ()>(&mut store, "cabi_post_echo-mixed")
    .expect("cabi_post_echo-mixed");

  let mixed_ret = echo_mixed
    .call(
      &mut store,
      (1, 123456, -7, label_ptr2, label_bytes.len() as i32, 1025, 9_007_199_254_740_991i64),
    )
    .expect("echo-mixed call");
  let mut buf = [0u8; 32];
  memory.read(&store, mixed_ret as usize, &mut buf).expect("read Mixed return");
  assert_eq!(buf[0], 1, "b should be true");
  assert_eq!(i32::from_le_bytes(buf[4..8].try_into().unwrap()), 123456);
  assert_eq!(buf[8] as i8, -7, "i8 should be -7");
  let out_ptr = u32::from_le_bytes(buf[12..16].try_into().unwrap()) as usize;
  let out_len = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
  let mut out_label = vec![0u8; out_len];
  memory.read(&store, out_ptr, &mut out_label).expect("read out label");
  assert_eq!(out_label, label_bytes);
  assert_eq!(u16::from_le_bytes(buf[20..22].try_into().unwrap()), 1025);
  assert_eq!(u64::from_le_bytes(buf[24..32].try_into().unwrap()), 9_007_199_254_740_991u64);
  post_mixed.call(&mut store, mixed_ret).expect("post-return Mixed");
}

#[test]
fn boxed_json_parse_proc_and_bool_unboxing_paths() {
  let dir = TestDirectory::create("wasm-json-proc-bool");
  // Uses ONLY `json-parse` (not `&try-parse-json` or `.parse-json`) to verify
  // `CalcitProc::JsonParse` enables boxed JSON scalar unboxing across branches,
  // static `map` adapters, and `match` payload bindings.
  let snapshot = r#"
{} (:package |compact-json-proc)
  :about "|json-parse proc boxed scalar unboxing test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-json-proc.main/main!) (:mode :native) (:reload-fn 'compact-json-proc.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-json-proc.main
    %{} 'FileEntry
      :defs $ {}
        'flag->num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn flag->num (b) (if b 1 2)
          :examples $ []
          :schema $ quote $ Fn Bool (-> Number)
        'verify-json-proc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-json-proc ()
            let
                x $ json-parse "|false"
                c1 $ if (bool? x) (if (assert-type x Bool) 1 2) 3
                xs $ assert-type (json-parse "|[false,true]") (List Bool)
                mapped $ map xs flag->num
                opt-b $ if (bool? x) (Some (assert-type x Bool)) None
                matched $ match opt-b
                  (:none) 0
                  (:some b) (if b 10 20)
              and
                = c1 2
                = (&list:nth mapped 0) 2
                = (&list:nth mapped 1) 1
                = matched 20
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (verify-json-proc) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-json-proc.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let verify = instance
    .get_typed_func::<(), f64>(&mut store, "verify-json-proc")
    .expect("verify-json-proc export");
  assert_eq!(verify.call(&mut store, ()).expect("verify-json-proc executes"), 1.0);
}

#[test]
fn typed_spread_recur_matches_native_local_representations() {
  let dir = TestDirectory::create("wasm-spread-recur");
  let snapshot = r#"
{} (:package |compact-recur)
  :about "|Typed spread recur representation test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-recur.main/main!) (:mode :native) (:reload-fn 'compact-recur.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-recur.main
    %{} 'FileEntry
      :defs $ {}
        'spread-rest-bool $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn spread-rest-bool (flag & xs)
            if flag
              recur false & ([] 7 9)
              and (= (&list:count xs) 2) (= (&list:nth xs 0) 7) (= (&list:nth xs 1) 9)
          :examples $ []
          :schema $ quote $ Fn Bool (& Number) (-> Bool)
        'spread-unpack-typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn spread-unpack-typed (flag code & xs)
            if flag
              recur false & ([] (assert-type 42 Int32) 11 22)
              and (= code (assert-type 42 Int32)) (= (&list:count xs) 2) (= (&list:nth xs 0) 11)
          :examples $ []
          :schema $ quote $ Fn Bool Int32 (& Number) (-> Bool)
        'run-spread-recur $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export run-spread-recur ()
            and
              spread-rest-bool true 1
              spread-unpack-typed true (assert-type 1 Int32) 2
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (run-spread-recur) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-recur.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let run = instance
    .get_typed_func::<(), f64>(&mut store, "run-spread-recur")
    .expect("run-spread-recur export");
  assert_eq!(run.call(&mut store, ()).expect("spread recur succeeds"), 1.0);
}

#[test]
fn enum_nth_rejects_invalid_literal_overflow_and_out_of_bounds_indices() {
  let dir = TestDirectory::create("wasm-enum-nth-bounds");
  let snapshot = r#"
{} (:package |compact-enum-bounds)
  :about "|Enum nth bounds and overflow test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-enum-bounds.main/main!) (:mode :native) (:reload-fn 'compact-enum-bounds.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-enum-bounds.main
    %{} 'FileEntry
      :defs $ {}
        'Boxed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Boxed (:Empty) (:One Number)
          :examples $ []
          :schema $ quote EnumDef
        'valid-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export valid-tag ()
            = (assert-type (&enum:nth (Boxed :One 42) 0) Tag) :One
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'valid-payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export valid-payload ()
            assert-type (&enum:nth (Boxed :One 42) 1) Number
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'lit-oob $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export lit-oob ()
            assert-type (&enum:nth (Boxed :One 42) 2) Number
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'lit-overflow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export lit-overflow ()
            assert-type (&enum:nth (Boxed :One 42) 2305843009213693952) Number
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'lit-neg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export lit-neg ()
            assert-type (&enum:nth (Boxed :One 42) -1) Number
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'dyn-nth $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export dyn-nth (idx)
            assert-type (&enum:nth (Boxed :One 42) idx) Number
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-enum-bounds.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);

  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "valid-tag")
      .expect("valid-tag")
      .call(&mut store, ())
      .expect("valid tag call"),
    1.0
  );
  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "valid-payload")
      .expect("valid-payload")
      .call(&mut store, ())
      .expect("valid payload call"),
    42.0
  );
  for trap_fn in ["lit-oob", "lit-overflow", "lit-neg"] {
    assert!(
      instance
        .get_typed_func::<(), f64>(&mut store, trap_fn)
        .expect(trap_fn)
        .call(&mut store, ())
        .is_err(),
      "{trap_fn} must trap on invalid literal &enum:nth index"
    );
  }
  let dyn_nth = instance.get_typed_func::<f64, f64>(&mut store, "dyn-nth").expect("dyn-nth");
  assert_eq!(dyn_nth.call(&mut store, 1.0).expect("dyn idx 1"), 42.0);
  for bad in [-1.0, 0.5, 2.0, f64::NAN, f64::INFINITY] {
    assert!(dyn_nth.call(&mut store, bad).is_err(), "dyn-nth({bad}) must trap");
  }
}

#[test]
fn compact_struct_scalar_equality_ignores_matching_heap_string_addresses() {
  let dir = TestDirectory::create("wasm-compact-eq-scalar");
  let snapshot = r#"
{} (:package |compact-eq-scalar)
  :about "|Compact scalar struct equality without heap probes test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-eq-scalar.main/main!) (:mode :native) (:reload-fn 'compact-eq-scalar.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-eq-scalar.main
    %{} 'FileEntry
      :defs $ {}
        'IdBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct IdBox (:id UInt32)
          :examples $ []
          :schema $ quote StructDef
        'NameBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NameBox (:name String)
          :examples $ []
          :schema $ quote StructDef
        '*s1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s1 |
          :examples $ []
          :schema $ quote $ Ref String
        '*s2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s2 |
          :examples $ []
          :schema $ quote $ Ref String
        'init-strings $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export init-strings ()
            do
              reset! *s1 (&str:concat |he |llo)
              reset! *s2 (&str:concat |he |llo)
              , true
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'get-s1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export get-s1 () @*s1
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'get-s2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export get-s2 () @*s2
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'check-eq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-eq (a b)
            let
                b1 $ IdBox :id a
                b2 $ IdBox :id b
                m $ &{} b1 10 b2 20
              and
                not= b1 b2
                = b1 (IdBox :id a)
                = (&map:get m b1) (Some 10)
                = (&map:get m b2) (Some 20)
                = (NameBox :name @*s1) (NameBox :name @*s2)
          :examples $ []
          :schema $ quote $ Fn UInt32 UInt32 (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-eq-scalar.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);

  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "init-strings")
      .expect("init-strings")
      .call(&mut store, ())
      .expect("init-strings succeeds"),
    1.0
  );
  let addr1 = instance
    .get_typed_func::<(), f64>(&mut store, "get-s1")
    .expect("get-s1")
    .call(&mut store, ())
    .expect("get-s1 succeeds");
  let addr2 = instance
    .get_typed_func::<(), f64>(&mut store, "get-s2")
    .expect("get-s2")
    .call(&mut store, ())
    .expect("get-s2 succeeds");
  assert_ne!(
    addr1, addr2,
    "distinct allocations of \"hello\" must occupy distinct heap addresses"
  );
  assert!(addr1 > 0.0 && addr2 > 0.0, "heap addresses must be positive");

  let check_eq = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "check-eq")
    .expect("check-eq");
  assert_eq!(check_eq.call(&mut store, (addr1, addr2)).expect("check-eq executes"), 1.0);
}

#[test]
fn runtime_struct_store_field_unboxes_boxed_json_bool() {
  let dir = TestDirectory::create("wasm-rt-struct-bool-store");
  let snapshot = r#"
{} (:package |compact-rt-bool-store)
  :about "|Runtime __rt_struct_store_field boxed JSON Bool unboxing test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-rt-bool-store.main/main!) (:mode :native) (:reload-fn 'compact-rt-bool-store.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-rt-bool-store.main
    %{} 'FileEntry
      :defs $ {}
        'BoolFlag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct BoolFlag (:flag Bool)
          :examples $ []
          :schema $ quote StructDef
        'NumFlag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NumFlag (:flag Number)
          :examples $ []
          :schema $ quote StructDef
        'verify-rt-bool-store $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-rt-bool-store ()
            let
                xs $ assert-type (json-parse "|[false,true]") (List Bool)
                b-true $ BoolFlag :flag true
                b-false $ BoolFlag :flag false
                nf $ NumFlag :flag 7
                from-json-false $ &struct:assoc-at b-true 0 :flag (&list:nth xs 0)
                from-json-true $ &struct:assoc-at b-false 0 :flag (&list:nth xs 1)
                from-plain-false $ &struct:assoc-at b-true 0 :flag false
                from-plain-true $ &struct:assoc-at b-false 0 :flag true
                nf-updated $ &struct:assoc-at nf 0 :flag 9
              and
                not (:flag from-json-false)
                :flag from-json-true
                not (:flag from-plain-false)
                :flag from-plain-true
                = (:flag nf-updated) 9
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (verify-rt-bool-store) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-rt-bool-store.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let parsed = parse_wasm_module(&bytes);

  let export_idx = *parsed.exports.get("verify-rt-bool-store").expect("verify-rt-bool-store export");
  let target_idx = *parsed.functions[&export_idx]
    .calls
    .iter()
    .find(|callee| {
      parsed
        .functions
        .get(callee)
        .is_some_and(|f| f.params.is_empty() && f.results.as_slice() == [ValType::F64])
    })
    .expect("export wrapper must target verify-rt-bool-store");
  assert!(
    parsed.functions[&target_idx].calls.iter().any(|callee| {
      parsed
        .functions
        .get(callee)
        .is_some_and(|f| f.params.as_slice() == [ValType::I32, ValType::I32, ValType::I32, ValType::F64] && f.results.is_empty())
    }),
    "verify-rt-bool-store must exercise __rt_struct_store_field when BoolFlag and NumFlag disagree on :flag kind"
  );

  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let verify = instance
    .get_typed_func::<(), f64>(&mut store, "verify-rt-bool-store")
    .expect("verify-rt-bool-store export");
  assert_eq!(verify.call(&mut store, ()).expect("verify-rt-bool-store executes"), 1.0);
}

#[test]
fn selective_linking_elides_unused_number_format_and_preserves_formatting_semantics() {
  const NUMBER_TABLE_BASE: i32 = 8192;

  let arith_dir = TestDirectory::create("wasm-selective-arith");
  let arith_snapshot = r#"
{} (:package |selective-arith)
  :about "|Selective linking pure arithmetic test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'selective-arith.main/add-nums) (:mode :native) (:reload-fn 'selective-arith.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'selective-arith.main
    %{} 'FileEntry
      :defs $ {}
        'add-nums $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export add-nums (a b) (+ a b)
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns selective-arith.main
"#;
  let arith_wasm = compile_wasm_with_boundary(&arith_dir, arith_snapshot, "native");
  let arith_bytes = fs::read(&arith_wasm).expect("arith wasm bytes should read");
  assert!(
    arith_bytes.len() < 1024,
    "pure arithmetic module must elide unused imports, types, and data under 1 KB, got {} bytes",
    arith_bytes.len()
  );
  let arith_parsed = parse_wasm_module(&arith_bytes);
  assert_eq!(
    arith_parsed.num_func_imports, 0,
    "pure arithmetic module must prune all unused host imports"
  );
  assert!(
    arith_parsed.type_count <= 5,
    "pure arithmetic module must deduplicate types and omit canonical HOF signatures, got {}",
    arith_parsed.type_count
  );
  assert_eq!(
    arith_parsed.data_bytes, 0,
    "pure arithmetic module must elide static string/tag/descriptor/Ryū data, got {} bytes",
    arith_parsed.data_bytes
  );
  assert!(
    arith_parsed.data_offsets.is_empty(),
    "pure arithmetic module must emit no active data segments, got {:?}",
    arith_parsed.data_offsets
  );
  assert!(
    arith_parsed.functions.len() < 16,
    "pure arithmetic should prune unreachable runtime functions, kept {}",
    arith_parsed.functions.len()
  );
  let (mut arith_store, arith_instance) = instantiate_core_with_limit(&arith_wasm, EIGHT_MIB);
  let add_nums = arith_instance
    .get_typed_func::<(f64, f64), f64>(&mut arith_store, "add-nums")
    .expect("add-nums export");
  assert_eq!(add_nums.call(&mut arith_store, (20.0, 22.0)).expect("add-nums executes"), 42.0);

  let fmt_dir = TestDirectory::create("wasm-selective-fmt");
  let fmt_snapshot = r#"
{} (:package |selective-fmt)
  :about "|Selective linking direct and indirect float formatting test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'selective-fmt.main/fmt-num) (:mode :native) (:reload-fn 'selective-fmt.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'selective-fmt.main
    %{} 'FileEntry
      :defs $ {}
        'fmt-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export fmt-num () (str (+ 1.25 2.5))
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'log-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export log-num ()
            do
              println (+ 0.5 0.25)
              , 1
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns selective-fmt.main
"#;
  let fmt_wasm = compile_wasm_with_boundary(&fmt_dir, fmt_snapshot, "native");
  let fmt_bytes = fs::read(&fmt_wasm).expect("fmt wasm bytes should read");
  let fmt_parsed = parse_wasm_module(&fmt_bytes);
  assert!(
    fmt_parsed.num_func_imports < 11,
    "fmt module must prune uncalled host imports below 11, got {}",
    fmt_parsed.num_func_imports
  );
  assert!(
    fmt_parsed.data_offsets.contains(&NUMBER_TABLE_BASE),
    "float formatting must retain Ryū lookup table at {NUMBER_TABLE_BASE}, got offsets {:?}",
    fmt_parsed.data_offsets
  );
  let (mut fmt_store, fmt_instance) = instantiate_core_with_limit(&fmt_wasm, EIGHT_MIB);
  let fmt_num = fmt_instance
    .get_typed_func::<(), f64>(&mut fmt_store, "fmt-num")
    .expect("fmt-num export");
  let str_ptr = fmt_num.call(&mut fmt_store, ()).expect("fmt-num executes") as usize;
  let memory = fmt_instance.get_memory(&mut fmt_store, "memory").expect("memory export");
  let mem = memory.data(&fmt_store);
  let len = f64::from_le_bytes(mem[str_ptr..str_ptr + 8].try_into().unwrap()) as usize;
  let text = std::str::from_utf8(&mem[str_ptr + 8..str_ptr + 8 + len]).expect("utf8 float text");
  assert_eq!(text, "3.75");
  let log_num = fmt_instance
    .get_typed_func::<(), f64>(&mut fmt_store, "log-num")
    .expect("log-num export");
  assert_eq!(log_num.call(&mut fmt_store, ()).expect("log-num executes"), 1.0);

  let print_dir = TestDirectory::create("wasm-selective-print-only");
  let print_snapshot = r#"
{}
  :about "|Selective linking print-only float test"
  :package |selective-print
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'selective-print.main/main!) (:mode :native) (:reload-fn 'selective-print.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'selective-print.main
    %{} 'FileEntry
      :defs $ {}
        'print-float $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export print-float () (println (+ 0.5 0.25)) (+ 0.5 0.25)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (print-float)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns selective-print.main (:require)
"#;
  let print_wasm = compile_wasm_with_boundary(&print_dir, print_snapshot, "native");
  let print_bytes = fs::read(&print_wasm).expect("print-only wasm bytes should read");
  let print_parsed = parse_wasm_module(&print_bytes);
  assert!(
    print_parsed.num_func_imports < 11,
    "print-only module must prune uncalled host imports below 11, got {}",
    print_parsed.num_func_imports
  );
  assert!(
    print_parsed.data_offsets.contains(&NUMBER_TABLE_BASE),
    "print-only float fixture without `str` must retain Ryū table at {NUMBER_TABLE_BASE}, got {:?}",
    print_parsed.data_offsets
  );
  let (mut print_store, print_instance) = instantiate_core_with_limit(&print_wasm, EIGHT_MIB);
  let print_float = print_instance
    .get_typed_func::<(), f64>(&mut print_store, "print-float")
    .expect("print-float export");
  assert_eq!(print_float.call(&mut print_store, ()).expect("print-float executes"), 0.75);
}

#[test]
fn native_wasi_export_name_preserves_qualification_when_uncalled_same_named_fn_is_pruned() {
  let dir = TestDirectory::create("wasi-native-export-qualification");
  let snapshot_path = dir.0.join("calcit.cirru");
  let snapshot = r#"
{}
  :about "|Preserve qualified WASI native export name when uncalled collision is pruned"
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.lib $ %{} 'FileEntry
      :defs $ {}
        'score $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn score () 7
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.lib
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'score $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export score () 42
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'unused-caller $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unused-caller () (lib/score)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (score)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require
            app.lib :as lib
"#;
  fs::write(&snapshot_path, snapshot).expect("test snapshot should write");
  let out_dir = dir.0.join("out");
  fs::create_dir(&out_dir).expect("wasi output directory should create");
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["wasi", "--boundary", "native", "--emit-path"])
    .arg(&out_dir)
    .output()
    .expect("WASI (native) compilation command should run");
  assert!(
    compile.status.success(),
    "WASI (native) compilation failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );
  let bytes = fs::read(out_dir.join("program.wasm")).expect("wasi wasm bytes should read");
  let parsed = parse_wasm_module(&bytes);
  assert!(
    parsed.exports.contains_key("app.main/score"),
    "expected qualified export `app.main/score`, got {:?}",
    parsed.exports.keys().collect::<Vec<_>>()
  );
  assert!(
    !parsed.exports.contains_key("score"),
    "short export `score` must not shadow qualified `app.main/score`"
  );
}

#[test]
fn recursive_and_nested_generic_deftype_aliases_do_not_overflow_reachability() {
  let dir = TestDirectory::create("wasm-cyclic-nested-deftype");
  let snapshot = r#"
{}
  :about "|Verify cyclic and nested generic deftype aliases terminate in reachability walk"
  :package |aliascheck
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'aliascheck.main/main!) (:mode :native) (:reload-fn 'aliascheck.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'aliascheck.main $ %{} 'FileEntry
      :defs $ {}
        'BoxItem $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct BoxItem (:value Number)
          :examples $ []
        'Nodes $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Nodes (List Nodes)
          :examples $ []
        'Wrap $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Wrap (:generics T) (List T)
          :examples $ []
        'check-cyclic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-cyclic (xs) 1
          :examples $ []
          :schema $ quote $ Fn Nodes (-> Number)
        'check-nested $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-nested (xs) (count xs)
          :examples $ []
          :schema $ quote $ Fn (Wrap (Wrap BoxItem)) (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                a $ check-cyclic ([] )
                b $ check-nested ([] )
              + a b
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns aliascheck.main (:require)
"#;
  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _parsed = parse_wasm_module(&bytes);
}

#[test]
fn map_equality_checks_key_presence_and_trim_rejects_non_ascii_patterns() {
  let dir = TestDirectory::create("wasm-map-eq-and-trim");
  let snapshot = r#"
{}
  :about "|Verify map key presence in equality and strict ASCII trim pattern validation"
  :package |maptrim
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'maptrim.main/main!) (:mode :native) (:reload-fn 'maptrim.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'maptrim.main
    %{} 'FileEntry
      :defs $ {}
        'empty-tag-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ &{} :a 1
          :examples $ []
        'check-maps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-maps ()
            and
              not $ = (&{} 1 0) (&{} 2 0)
              not $ = (&{} :a false) (&{} :b false)
              = (&{} 1 0) (&{} 1 0)
              not $ = (&{} :a 0 :b 1) (&{} :a 0 :c 1)
              = (&{} :a 0 :b 1) (&{} :b 1 :a 0)
              = (dissoc (&{} :a 1) :a) (dissoc empty-tag-num :a)
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'check-trim $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-trim ()
            and
              = (trim |__1__ |_) |1
              = (trim "|  éxé  ") |éxé
              = (trim |éxé |é) |x
              = (trim "|😀x😀" "|😀") |x
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'trim-multi-utf8 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export trim-multi-utf8 () (trim |éxé |éa)
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'trim-empty $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export trim-empty () (trim |éxé |)
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'trim-multi $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export trim-multi () (trim |éxé |ab)
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (and (check-maps) (check-trim)) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns maptrim.main (:require)
"#;
  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _parsed = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);

  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "check-maps")
      .expect("check-maps")
      .call(&mut store, ())
      .expect("check-maps executes"),
    1.0
  );
  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "check-trim")
      .expect("check-trim")
      .call(&mut store, ())
      .expect("check-trim executes"),
    1.0
  );
  for trap_fn in ["trim-multi-utf8", "trim-empty", "trim-multi"] {
    assert!(
      instance
        .get_typed_func::<(), f64>(&mut store, trap_fn)
        .expect(trap_fn)
        .call(&mut store, ())
        .is_err(),
      "{trap_fn} must trap when trim pattern is not a single Unicode scalar"
    );
  }
}

#[test]
fn option_niche_zero_alloc_writes_direct_consumers_and_escaping_identity() {
  let dir = TestDirectory::create("wasm-option-niche");
  let snapshot = r#"
{} (:package |opt-niche)
  :about "|OptionHeapPtrI32 niche optimization test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'opt-niche.main/main!) (:mode :native) (:reload-fn 'opt-niche.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'opt-niche.main
    %{} 'FileEntry
      :defs $ {}
        'NicheNode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NicheNode (:label String) (:next (Option opt-niche.main/NicheNode))
          :examples $ []
          :schema $ quote StructDef
        'NonNicheBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NonNicheBox (:nested (Option (Option String))) (:num (Option Number)) (:flag (Option Bool)) (:tag-opt (Option Tag)) (:dyn-opt (Option Dynamic))
          :examples $ []
          :schema $ quote StructDef
        '*persisted-niche $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *persisted-niche $ NicheNode :label |init :next None
          :examples $ []
          :schema $ quote $ Ref NicheNode
        '*churn-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *churn-str |seed
          :examples $ []
          :schema $ quote $ Ref String
        'measure-niche-node-bytes $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export measure-niche-node-bytes ()
            let
                a $ NicheNode :label |a :next None
                b $ NicheNode :label |b :next (Some a)
              , b
          :examples $ []
          :schema $ quote $ Fn (-> NicheNode)
        'verify-zero-alloc-write-and-direct-consumers $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-zero-alloc-write-and-direct-consumers ()
            let
                tail $ NicheNode :label |tail :next None
                mid $ NicheNode :label |mid :next (Some tail)
                copied $ NicheNode :label |copied :next (:next mid)
                fallback $ NicheNode :label |fallback :next None
                m-none $ match (:next tail)
                  (:none) true
                  (:some _) false
                m-some $ match (:next copied)
                  (:some n) (= (:label n) |tail)
                  (:none) false
                p-some $ (:next mid) .some?
                p-none $ (:next tail) .none?
                u-label $ :label ((:next mid) .unwrap)
                uo-none $ :label ((:next tail) .unwrap-or fallback)
                uo-some $ :label ((:next copied) .unwrap-or fallback)
              and m-none m-some p-some p-none (= u-label |tail) (= uo-none |fallback) (= uo-some |tail)
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'verify-escaping-identical-none $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-escaping-identical-none ()
            let
                node $ NicheNode :label |leaf :next None
                a $ :next node
                b $ :next node
                boxed $ NonNicheBox :nested (Some None) :num (Some 42) :flag (Some true) :tag-opt (Some :ok) :dyn-opt (Some (assert-type |dyn Dynamic))
                boxed-none $ NonNicheBox :nested None :num None :flag None :tag-opt None :dyn-opt None
              and
                not (identical? a b)
                not (identical? (:next node) (:next node))
                = a b
                = a None
                = (NicheNode :label |leaf :next None) node
                = (&hash (NicheNode :label |leaf :next None)) (&hash node)
                = (:num boxed) (Some 42)
                (:nested boxed) .some?
                (:nested boxed-none) .none?
                (:dyn-opt boxed-none) .none?
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'commit-and-verify-niche $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export commit-and-verify-niche ()
            let
                tail $ NicheNode :label (&str:concat |ta |il) :next None
                head $ NicheNode :label (&str:concat |he |ad) :next (Some tail)
              reset! *persisted-niche head
              , true
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'read-persisted-niche $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-persisted-niche ()
            let
                cur @*persisted-niche
              and
                = (:label cur) |head
                match (:next cur)
                  (:some tail) (and (= (:label tail) |tail) ((:next tail) .none?))
                  (:none) false
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'pump-niche-churn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-niche-churn (n)
            reset! *churn-str (&str:pad-right (&str:concat |churn- (to-string n)) 32768 |Z)
            &str:count @*churn-str
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (and (verify-zero-alloc-write-and-direct-consumers) (verify-escaping-identical-none)) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns opt-niche.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset");

  arena_reset.call(&mut store, ()).expect("arena reset");
  let before = get_i32_global(&mut store, &instance, "__heap_ptr");
  let _ = instance
    .get_typed_func::<(), f64>(&mut store, "measure-niche-node-bytes")
    .expect("measure-niche-node-bytes")
    .call(&mut store, ())
    .expect("measure-niche-node-bytes runs");
  let after_two = get_i32_global(&mut store, &instance, "__heap_ptr");
  // Two NicheNode allocations: each 8 GC + 8 header + 4 label + 4 next = 24 B => 48 B,
  // plus 16 B for __rt_turn_end's scratch_roots_head tracking node = 64 B (zero Option heap allocs!).
  assert_eq!(
    after_two - before,
    64,
    "two NicheNodes + 16B return root node must total 64 bytes without Option heap wrappers"
  );
  arena_reset.call(&mut store, ()).expect("arena reset");

  let before_zero = get_i32_global(&mut store, &instance, "__heap_ptr");
  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "verify-zero-alloc-write-and-direct-consumers")
      .expect("verify-zero-alloc-write-and-direct-consumers")
      .call(&mut store, ())
      .expect("verify-zero-alloc-write-and-direct-consumers runs"),
    1.0
  );
  let after_zero = get_i32_global(&mut store, &instance, "__heap_ptr");
  // Result is Bool scalar (`keep_scratch_result` rewinds __heap_ptr on exit), so check runtime semantics;
  // also check the internal pre-rewind usage when exporting NicheNode directly above (`after_two - before == 48`).
  assert_eq!(after_zero, before_zero);

  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "verify-escaping-identical-none")
      .expect("verify-escaping-identical-none")
      .call(&mut store, ())
      .expect("verify-escaping-identical-none runs"),
    1.0
  );

  // Persistent promotion & semi-space compaction with OptionHeapPtrI32.
  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "commit-and-verify-niche")
      .expect("commit-and-verify-niche")
      .call(&mut store, ())
      .expect("commit-and-verify-niche runs"),
    1.0
  );
  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let pump = instance
    .get_typed_func::<f64, f64>(&mut store, "pump-niche-churn")
    .expect("pump-niche-churn");
  let mut compacted = false;
  for i in 1..=25 {
    pump.call(&mut store, i as f64).expect("pump churn");
    if get_i32_global(&mut store, &instance, "__persistent_ptr") >= space_b_base {
      compacted = true;
      break;
    }
  }
  assert!(compacted, "expected persistent heap compaction into semi-space B");
  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "read-persisted-niche")
      .expect("read-persisted-niche")
      .call(&mut store, ())
      .expect("read-persisted-niche runs"),
    1.0
  );
}

#[test]
fn compact_enum_footprint_scalar_address_safety_and_codecs() {
  let dir = TestDirectory::create("wasm-compact-enum");
  let snapshot = r#"
{} (:package |compact-enum)
  :about "|Compact EnumLayouts footprint, scalar safety, defref compaction, and codecs test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-enum.main/main!) (:mode :native) (:reload-fn 'compact-enum.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-enum.main
    %{} 'FileEntry
      :defs $ {}
        'PackedMsg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum PackedMsg (:Empty) (:Pair Bool Int32 (Option String) UInt8) (:PairAlt Bool Int32 (Option String) UInt8)
          :examples $ []
          :schema $ quote EnumDef
        '*s1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s1 |
          :examples $ []
          :schema $ quote $ Ref String
        '*s2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s2 |
          :examples $ []
          :schema $ quote $ Ref String
        '*pers-msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *pers-msg $ %:: PackedMsg :Empty
          :examples $ []
          :schema $ quote $ Ref PackedMsg
        '*churn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *churn |seed
          :examples $ []
          :schema $ quote $ Ref String
        'alloc-empty-msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-empty-msg () (%:: PackedMsg :Empty)
          :examples $ []
          :schema $ quote $ Fn (-> PackedMsg)
        'alloc-pair-msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-pair-msg ()
            %:: PackedMsg :Pair true (assert-type 42 Int32) (Some |ok) (assert-type 7 UInt8)
          :examples $ []
          :schema $ quote $ Fn (-> PackedMsg)
        'init-strings $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export init-strings ()
            do
              reset! *s1 (&str:concat |he |llo)
              reset! *s2 (&str:concat |he |llo)
              , true
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'get-s1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export get-s1 () @*s1
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'get-s2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export get-s2 () @*s2
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'check-scalar-addr-and-enum-ops $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-scalar-addr-and-enum-ops (a b)
            let
                p1 $ %:: PackedMsg :Pair true a None (assert-type 5 UInt8)
                p2 $ %:: PackedMsg :Pair true b None (assert-type 5 UInt8)
                p3 $ %:: PackedMsg :Pair true a (Some @*s1) (assert-type 5 UInt8)
                p4 $ %:: PackedMsg :Pair true a (Some @*s2) (assert-type 5 UInt8)
                swapped $ &enum:assoc p3 0 :PairAlt
                updated $ &enum:assoc p3 3 (Some |hi)
                m $ &{} p1 10 p2 20
                edn-text $ format-cirru-edn p3
                parsed-ok $ match (try-parse-cirru-edn-as edn-text PackedMsg)
                  (:ok v) (= v p3)
                  (:err _) false
                match-ok $ match p3
                  (:Pair flag num opt small)
                    and flag (= num a) (opt .some?) (= (opt .unwrap) |hello) (= small (assert-type 5 UInt8))
                  _ false
                direct-niche-ok $ (assert-type (&enum:nth p3 3) (Option String)) .some?
              and
                not= p1 p2
                = p1 (%:: PackedMsg :Pair true a None (assert-type 5 UInt8))
                = p3 p4
                = (&hash p3) (&hash p4)
                = (&map:get m p1) (Some 10)
                = (&map:get m p2) (Some 20)
                = (&enum:count p3) 5
                = (assert-type (&enum:nth p3 0) Tag) :Pair
                = (assert-type (&enum:nth swapped 0) Tag) :PairAlt
                = (assert-type (&enum:nth swapped 3) (Option String)) (Some |hello)
                = (assert-type (&enum:nth updated 3) (Option String)) (Some |hi)
                not (identical? (&enum:nth p1 3) (&enum:nth p1 3))
                , parsed-ok
                , match-ok
                , direct-niche-ok
          :examples $ []
          :schema $ quote $ Fn Int32 Int32 (-> Bool)
        'commit-compact-enum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export commit-compact-enum ()
            do
              reset! *pers-msg (%:: PackedMsg :Pair false (assert-type 99 Int32) (Some (&str:concat |wo |rld)) (assert-type 3 UInt8))
              , true
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'read-compact-enum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-compact-enum ()
            match @*pers-msg
              (:Pair flag num opt small)
                and (not flag) (= num 99) (= opt (Some |world)) (= small 3)
              _ false
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'pump-churn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-churn (n)
            reset! *churn (&str:pad-right (&str:concat |churn- (to-string n)) 32768 |Z)
            &str:count @*churn
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (if (check-scalar-addr-and-enum-ops (assert-type 1 Int32) (assert-type 2 Int32)) 1 0)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-enum.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let bytes = fs::read(&wasm_path).expect("wasm bytes should read");
  let _ = parse_wasm_module(&bytes);
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset");

  let measure = |store: &mut wasmtime::Store<wasmtime::StoreLimits>, name: &str| -> i32 {
    arena_reset.call(&mut *store, ()).expect("arena reset");
    let before = get_i32_global(store, &instance, "__heap_ptr");
    let _ = instance
      .get_typed_func::<(), f64>(&mut *store, name)
      .expect(name)
      .call(&mut *store, ())
      .expect("alloc succeeds");
    let after = get_i32_global(store, &instance, "__heap_ptr");
    arena_reset.call(&mut *store, ()).expect("arena reset");
    after - before
  };

  // Module uses defref, so returning a Scratch heap enum adds a 16 B scratch_roots_head node in __rt_turn_end.
  assert_eq!(
    measure(&mut store, "alloc-empty-msg"),
    32,
    ":Empty enum (8 GC + 8 header = 16 B) + 16 B return root node must be 32 B"
  );
  // PackedMsg :Pair: 8 GC + 8 header + 4 Int32 + 4 OptionHeapPtr + 1 Bool + 1 UInt8 = 26 -> 32 B (+ 16 B root = 48 B; legacy 56 + 16 = 72 B).
  assert_eq!(
    measure(&mut store, "alloc-pair-msg"),
    48,
    "compact :Pair (32 B) + 16 B return root node must pack to 48 B (vs 72 B legacy)"
  );

  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "init-strings")
      .expect("init-strings")
      .call(&mut store, ())
      .expect("init-strings runs"),
    1.0
  );
  let addr1 = instance
    .get_typed_func::<(), f64>(&mut store, "get-s1")
    .expect("get-s1")
    .call(&mut store, ())
    .expect("get-s1 runs");
  let addr2 = instance
    .get_typed_func::<(), f64>(&mut store, "get-s2")
    .expect("get-s2")
    .call(&mut store, ())
    .expect("get-s2 runs");
  assert_ne!(addr1, addr2);

  assert_eq!(
    instance
      .get_typed_func::<(f64, f64), f64>(&mut store, "check-scalar-addr-and-enum-ops")
      .expect("check-scalar-addr-and-enum-ops")
      .call(&mut store, (addr1, addr2))
      .expect("check-scalar-addr-and-enum-ops runs"),
    1.0
  );

  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "commit-compact-enum")
      .expect("commit-compact-enum")
      .call(&mut store, ())
      .expect("commit-compact-enum runs"),
    1.0
  );
  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let pump = instance.get_typed_func::<f64, f64>(&mut store, "pump-churn").expect("pump-churn");
  let mut compacted = false;
  for i in 1..=25 {
    pump.call(&mut store, i as f64).expect("pump churn");
    if get_i32_global(&mut store, &instance, "__persistent_ptr") >= space_b_base {
      compacted = true;
      break;
    }
  }
  assert!(compacted, "expected persistent heap compaction into semi-space B");
  assert_eq!(
    instance
      .get_typed_func::<(), f64>(&mut store, "read-compact-enum")
      .expect("read-compact-enum")
      .call(&mut store, ())
      .expect("read-compact-enum runs"),
    1.0
  );
}

#[test]
fn component_abi_cross_layout_enum_equality_and_hash_parity() {
  let dir = TestDirectory::create("wasm-component-cross-layout");
  let snapshot = r#"
{} (:package |cross-layout)
  :about "|Component ABI cross-layout Enum equality and hash parity test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'cross-layout.main/main!) (:mode :native) (:reload-fn 'cross-layout.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'cross-layout.main
    %{} 'FileEntry
      :defs $ {}
        'Action $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Action (:Noop) (:Apply UInt16 (Option String) Bool)
          :examples $ []
          :schema $ quote EnumDef
        'verify-lifted-parity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-lifted-parity (lifted expected)
            let
                compact-some $ %:: Action :Apply (assert-type 9 UInt16) (Some |hi) true
                compact-none $ %:: Action :Apply (assert-type 9 UInt16) None false
                empty-a $ %:: Action :Noop
              and
                if (= expected 0)
                  and (= lifted compact-some) (= (&hash lifted) (&hash compact-some))
                  if (= expected 1)
                    and (= lifted compact-none) (= (&hash lifted) (&hash compact-none))
                    if (= expected 2)
                      and (= lifted empty-a) (= (&hash lifted) (&hash empty-a))
                      , false
                not= compact-some compact-none
                not= (&hash compact-some) (&hash compact-none)
          :examples $ []
          :schema $ quote $ Fn Action UInt8 (-> Bool)
        'make-compact-action $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-compact-action (with-str)
            if with-str
              %:: Action :Apply (assert-type 42 UInt16) (Some |ok) true
              %:: Action :Apply (assert-type 42 UInt16) None false
          :examples $ []
          :schema $ quote $ Fn Bool (-> Action)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns cross-layout.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "component");
  let bytes = fs::read(&wasm_path).expect("component core wasm should read");
  let _ = parse_wasm_module(&bytes);

  let engine = Engine::default();
  let module = Module::new(&engine, &bytes).expect("component core module loads");
  let mut store = Store::new(&engine, ());
  let instance = Instance::new(&mut store, &module, &[]).expect("instantiate component module");
  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  let realloc = instance
    .get_typed_func::<(i32, i32, i32, i32), i32>(&mut store, "cabi_realloc")
    .expect("cabi_realloc");

  // Alphabetical variants: 0 = :Apply(UInt16, Option<String>, Bool), 1 = :Noop
  // Flat params for verify-lifted-parity(Action, UInt8) -> i32:
  // (discriminant:i32, u16_val:i32, opt_disc:i32, str_ptr:i32, str_len:i32, bool_val:i32, expected:i32)
  let verify_lifted = instance
    .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32), i32>(&mut store, "verify-lifted-parity")
    .expect("verify-lifted-parity");

  let hi_bytes = b"hi";
  let hi_ptr = realloc.call(&mut store, (0, 0, 1, hi_bytes.len() as i32)).expect("realloc hi");
  memory.write(&mut store, hi_ptr as usize, hi_bytes).expect("write hi");

  // Lifted :Apply(9, Some("hi"), true) has layout_id == 0, compared against WASM-constructed compact layout_id >= 1.
  assert_eq!(
    verify_lifted
      .call(&mut store, (0, 9, 1, hi_ptr, 2, 1, 0))
      .expect("call :Apply some"),
    1
  );
  // Lifted :Apply(9, None, false)
  assert_eq!(verify_lifted.call(&mut store, (0, 9, 0, 0, 0, 0, 1)).expect("call :Apply none"), 1);
  // Lifted :Noop
  assert_eq!(verify_lifted.call(&mut store, (1, 0, 0, 0, 0, 0, 2)).expect("call :Noop"), 1);
  // Mismatched expected variants must be rejected.
  assert_eq!(
    verify_lifted
      .call(&mut store, (1, 0, 0, 0, 0, 0, 0))
      .expect("call :Noop with :Apply some expected"),
    0
  );

  // Lower compact Action (layout_id >= 1) back out through Component ABI export.
  let make_compact = instance
    .get_typed_func::<i32, i32>(&mut store, "make-compact-action")
    .expect("make-compact-action");
  let post_compact = instance
    .get_typed_func::<i32, ()>(&mut store, "cabi_post_make-compact-action")
    .expect("cabi_post_make-compact-action");

  let ret_some = make_compact.call(&mut store, 1).expect("make-compact-action(true)");
  // Canonical layout for Action: disc@0 (u8), payload@4: u16@4, opt<string>@8 (disc@8, ptr@12, len@16), bool@20 -> total 24 B.
  let mut buf = [0u8; 24];
  memory.read(&store, ret_some as usize, &mut buf).expect("read ret_some");
  assert_eq!(buf[0], 0);
  assert_eq!(u16::from_le_bytes(buf[4..6].try_into().unwrap()), 42);
  assert_eq!(buf[8], 1);
  let out_ptr = u32::from_le_bytes(buf[12..16].try_into().unwrap()) as usize;
  let out_len = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
  let mut out_str = vec![0u8; out_len];
  memory.read(&store, out_ptr, &mut out_str).expect("read lowered string");
  assert_eq!(out_str, b"ok");
  assert_eq!(buf[20], 1);
  post_compact.call(&mut store, ret_some).expect("post some");

  let ret_none = make_compact.call(&mut store, 0).expect("make-compact-action(false)");
  memory.read(&store, ret_none as usize, &mut buf).expect("read ret_none");
  assert_eq!(buf[0], 0);
  assert_eq!(u16::from_le_bytes(buf[4..6].try_into().unwrap()), 42);
  assert_eq!(buf[8], 0);
  assert_eq!(buf[20], 0);
  post_compact.call(&mut store, ret_none).expect("post none");
}

#[test]
fn compact_enum_assoc_preserves_overflow_and_encode_map_as_option_string() {
  let dir = TestDirectory::create("wasm-enum-assoc-encode");
  let snapshot = r#"
{} (:package |assoc-encode)
  :about "|Enum assoc non-narrowing fallback and encode-map-as Option<String> test"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'assoc-encode.main/main!) (:mode :native) (:reload-fn 'assoc-encode.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'assoc-encode.main
    %{} 'FileEntry
      :defs $ {}
        'Packet $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Packet (:Frame UInt8 (Option String))
          :examples $ []
          :schema $ quote EnumDef
        'verify! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify! ()
            let
                p $ %:: Packet :Frame (assert-type 5 UInt8) (Some |abcdefgh)
                fitted $ &enum:assoc p 1 9
                legacy-fitted $ :: :Frame 9 (Some |abcdefgh)
                overflowed $ &enum:assoc p 1 256
                legacy-overflow $ :: :Frame 256 (Some |abcdefgh)
                str-replaced $ &enum:assoc p 2 |raw-str
                legacy-str-replaced $ :: :Frame 5 |raw-str
                encoded $ encode-map-as p Packet
                roundtrip $ decode-map-as encoded Packet
              and
                &= fitted legacy-fitted
                = (&hash fitted) (&hash legacy-fitted)
                &= (:: :v (&enum:nth fitted 1)) (:: :v 9)
                &= overflowed legacy-overflow
                = (&hash overflowed) (&hash legacy-overflow)
                &= (:: :v (&enum:nth overflowed 1)) (:: :v 256)
                &= str-replaced legacy-str-replaced
                = (&hash str-replaced) (&hash legacy-str-replaced)
                = roundtrip p
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns assoc-encode.main
"#;

  let wasm_path = compile_wasm_with_boundary(&dir, snapshot, "native");
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let verify = instance.get_typed_func::<(), f64>(&mut store, "verify!").expect("verify! export");
  assert_eq!(verify.call(&mut store, ()).expect("verify! runs"), 1.0);
}

#[test]
fn option_niche_branch_keeps_defref_reset_state_branch_local() {
  let bad_dir = TestDirectory::create("wasm-opt-branch-reset-bad");
  let bad_snapshot = r#"
{} (:package |opt-reset-bad)
  :about "|Option branch must not leak then-arm defatom reset into else arm"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'opt-reset-bad.main/main!) (:mode :native) (:reload-fn 'opt-reset-bad.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'opt-reset-bad.main
    %{} 'FileEntry
      :defs $ {}
        'OptHolder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptHolder (:slot (Option (Fn Number (-> Number))))
          :examples $ []
          :schema $ quote StructDef
        'inc1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inc1 (x) (+ x 1)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        '*cb $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *cb $ fn (x) (hint-fn $ Fn Number (-> Number)) x
          :examples $ []
          :schema $ quote $ Ref (Fn Number (-> Number))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                flag false
                h $ %{} OptHolder
                  :slot $ if flag
                    let ((_ (reset! *cb inc1))) (Some inc1)
                    Some @*cb
              , 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns opt-reset-bad.main
"#;
  let bad_snapshot_path = bad_dir.0.join("calcit.cirru");
  fs::write(&bad_snapshot_path, bad_snapshot).expect("bad snapshot writes");
  let bad_out = bad_dir.0.join("out");
  fs::create_dir(&bad_out).expect("out dir creates");
  let bad_compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&bad_snapshot_path)
    .args(["wasm", "--boundary", "native", "--emit-path"])
    .arg(&bad_out)
    .output()
    .expect("compile runs");
  assert!(
    !bad_compile.status.success(),
    "else-arm reading uninitialized closure defref after then-arm reset must fail"
  );
  let stderr = String::from_utf8_lossy(&bad_compile.stderr);
  assert!(
    stderr.contains("E_WASM_CLOSURE_SPECIALIZATION"),
    "expected E_WASM_CLOSURE_SPECIALIZATION, got:\n{stderr}"
  );

  let good_dir = TestDirectory::create("wasm-opt-branch-reset-good");
  let good_snapshot = r#"
{} (:package |opt-reset-good)
  :about "|Option branch resetting defatom in both arms permits read after conditional"
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'opt-reset-good.main/main!) (:mode :native) (:reload-fn 'opt-reset-good.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'opt-reset-good.main
    %{} 'FileEntry
      :defs $ {}
        'OptHolder $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OptHolder (:slot (Option (Fn Number (-> Number))))
          :examples $ []
          :schema $ quote StructDef
        'inc1 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inc1 (x) (+ x 1)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        '*cb $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *cb $ fn (x) (hint-fn $ Fn Number (-> Number)) x
          :examples $ []
          :schema $ quote $ Ref (Fn Number (-> Number))
        'run! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export run! (flag)
            let
                _h $ %{} OptHolder
                  :slot $ if flag
                    let ((_ (reset! *cb inc1))) (Some inc1)
                    let ((_ (reset! *cb inc1))) None
                f @*cb
              f 5
          :examples $ []
          :schema $ quote $ Fn Bool (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns opt-reset-good.main
"#;
  let wasm_path = compile_wasm_with_boundary(&good_dir, good_snapshot, "native");
  let (mut store, instance) = instantiate_core_with_limit(&wasm_path, EIGHT_MIB);
  let run_fn = instance.get_typed_func::<f64, f64>(&mut store, "run!").expect("run! export");
  assert_eq!(run_fn.call(&mut store, 1.0).expect("flag=true"), 6.0);
  assert_eq!(run_fn.call(&mut store, 0.0).expect("flag=false"), 6.0);
}

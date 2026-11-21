#![cfg(feature = "wasm")]

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use calcit_bindgen::COMPONENT_FILE;
use tokio::sync::Notify;
use wasmtime::component::{
  Component, Destination, Linker as ComponentLinker, StreamProducer, StreamReader, StreamResult, Val as ComponentVal, VecBuffer,
};
use wasmtime::{
  Config, Engine, ExternType, Instance, Linker, Module, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder, Val as CoreVal,
};

const HEAP_MAGIC: i32 = 0xCA1C_17A9u32 as i32;
const SEMI_SPACE_BYTES: i32 = 1024 * 1024;
const EIGHT_MIB: usize = 8 * 1024 * 1024;

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
    fs::create_dir(&path).expect("temporary output directory should create");
    Self(path)
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn compile_core_wasm(dir: &TestDirectory, source: &str) -> PathBuf {
  let snapshot_path = dir.0.join("calcit.cirru");
  fs::write(&snapshot_path, source).expect("test snapshot should write");
  let out_dir = dir.0.join("out");
  fs::create_dir(&out_dir).expect("wasm output directory should create");
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["wasm", "--emit-path"])
    .arg(&out_dir)
    .output()
    .expect("Core WASM compilation command should run");
  assert!(
    compile.status.success(),
    "Core WASM compilation failed\nstdout:\n{}\nstderr:\n{}",
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

fn read_wasm_str(store: &mut Store<StoreLimits>, instance: &Instance, ptr_f64: f64) -> String {
  let string_tag = get_i32_global(store, instance, "__string_tag");
  let memory = instance.get_memory(&mut *store, "memory").expect("memory export must exist");
  let data = memory.data(&*store);
  assert!(
    ptr_f64.is_finite() && ptr_f64 >= 8.0 && ptr_f64.fract() == 0.0,
    "expected integer heap pointer, got {ptr_f64}"
  );
  let ptr = ptr_f64 as usize;
  assert!(ptr + 8 <= data.len(), "string pointer out of linear memory: {ptr}");
  let magic = i32::from_le_bytes(data[ptr - 8..ptr - 4].try_into().unwrap());
  assert_eq!(magic, HEAP_MAGIC, "invalid HEAP_MAGIC at ptr {ptr}: {magic:#x}");
  let tag = i32::from_le_bytes(data[ptr - 4..ptr].try_into().unwrap());
  assert_eq!(tag, string_tag, "expected string tag {string_tag}, got {tag} at ptr {ptr}");
  let byte_len_f64 = f64::from_le_bytes(data[ptr..ptr + 8].try_into().unwrap());
  assert!(
    byte_len_f64.is_finite() && byte_len_f64 >= 0.0 && byte_len_f64.fract() == 0.0,
    "invalid string byte length: {byte_len_f64}"
  );
  let byte_len = byte_len_f64 as usize;
  let bytes = &data[ptr + 8..ptr + 8 + byte_len];
  std::str::from_utf8(bytes)
    .expect("WASM string payload should be valid UTF-8")
    .to_owned()
}

fn read_header_magic(store: &mut Store<StoreLimits>, instance: &Instance, ptr_f64: f64) -> i32 {
  let memory = instance.get_memory(&mut *store, "memory").expect("memory export must exist");
  let data = memory.data(&*store);
  let ptr = ptr_f64 as usize;
  i32::from_le_bytes(data[ptr - 8..ptr - 4].try_into().unwrap())
}

#[test]
fn core_wasm_repeated_exports_reset_arena_within_8mib_limit() {
  let dir = TestDirectory::create("wasm-arena-repeated");
  let snapshot = r#"
{}
  :about "|Test snapshot for Core WASM request-scoped arena reset."
  :package |arena-repeated
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'arena-repeated.main/main!) (:mode :native) (:reload-fn 'arena-repeated.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'arena-repeated.main
    %{} 'FileEntry
      :defs $ {}
        'Box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Box (:label String) (:score Number)
          :examples $ []
          :schema $ quote StructDef
        'Msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Msg (:Empty) (:Text String) (:Tagged Number String)
          :examples $ []
          :schema $ quote EnumDef
        'alloc-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-string (n)
            &str:concat |item- $ &str:pad-right (to-string n) 128 |a
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'alloc-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-list (n)
            let
                s $ to-string n
                xs $ [] (&str:concat |L0- s) (&str:concat |L1- s) (&str:concat |L2- s)
              &str:concat (&list:nth xs 1) (&str:concat |# (to-string (&list:count xs)))
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'alloc-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-map (n)
            let
                s $ to-string n
                m $ &{} |k0 (&str:concat |M0- s) |k1 (&str:concat |M1- s)
                m2 $ &map:assoc m |k2 (&str:concat |M2- s)
              match (.get m2 |k2)
                (:some v) v
                (:none) |missing
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'alloc-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-struct (n)
            let
                s $ to-string n
                b $ Box :label (&str:concat |S- s) :score n
              &str:concat (:label b) (&str:concat |: (to-string (:score b)))
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'alloc-enum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alloc-enum (n)
            let
                s $ to-string n
                m $ Msg :Tagged n (&str:concat |E- s)
              match m
                (:Empty) |empty
                (:Text txt) txt
                (:Tagged code txt) (&str:concat txt (&str:concat |/ (to-string code)))
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'score-step $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export score-step (n)
            let
                s $ &str:pad-right (&str:concat |step- (to-string n)) 256 |S
                xs $ [] s (&str:concat |tail- (to-string n))
              &+ n (&str:count (&list:nth xs 0))
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
        :code $ quote $ ns arena-repeated.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  assert!(
    instance.get_global(&mut store, "__persistent_ptr").is_none(),
    "stateless module without defref should not export __persistent_ptr"
  );
  let memory = instance.get_memory(&mut store, "memory").expect("memory export must exist");
  let initial_pages = memory.size(&store);
  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  let alloc_string = instance
    .get_typed_func::<f64, f64>(&mut store, "alloc-string")
    .expect("alloc-string export");
  let alloc_list = instance
    .get_typed_func::<f64, f64>(&mut store, "alloc-list")
    .expect("alloc-list export");
  let alloc_map = instance
    .get_typed_func::<f64, f64>(&mut store, "alloc-map")
    .expect("alloc-map export");
  let alloc_struct = instance
    .get_typed_func::<f64, f64>(&mut store, "alloc-struct")
    .expect("alloc-struct export");
  let alloc_enum = instance
    .get_typed_func::<f64, f64>(&mut store, "alloc-enum")
    .expect("alloc-enum export");
  let score_step = instance
    .get_typed_func::<f64, f64>(&mut store, "score-step")
    .expect("score-step export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");

  // Scalar returns auto-rewind __heap_ptr without explicit __arena_reset().
  for n in 0..10u64 {
    assert_eq!(score_step.call(&mut store, n as f64).expect("score-step"), n as f64 + 256.0);
    assert_eq!(
      get_i32_global(&mut store, &instance, "__heap_ptr"),
      arena_base,
      "scalar return must auto-rewind __heap_ptr at iteration {n}"
    );
  }

  // Newly allocated Scratch heap results remain live across consecutive calls until __arena_reset().
  let ptr0 = alloc_string.call(&mut store, 0.0).expect("alloc-string 0 should succeed");
  let heap_after_0 = get_i32_global(&mut store, &instance, "__heap_ptr");
  assert!(heap_after_0 > arena_base, "Scratch String return must keep __heap_ptr alive");
  let ptr1 = alloc_string.call(&mut store, 1.0).expect("alloc-string 1 should succeed");
  let heap_after_1 = get_i32_global(&mut store, &instance, "__heap_ptr");
  assert!(heap_after_1 > heap_after_0, "second Scratch return must advance past the first");
  assert_ne!(ptr1, ptr0, "distinct live Scratch returns must not alias");
  let s0 = read_wasm_str(&mut store, &instance, ptr0);
  assert!(s0.starts_with("item-0") && s0.len() == 5 + 128, "first return overwritten: `{s0}`");
  let s1 = read_wasm_str(&mut store, &instance, ptr1);
  assert!(s1.starts_with("item-1") && s1.len() == 5 + 128, "second return corrupted: `{s1}`");
  arena_reset.call(&mut store, ()).expect("__arena_reset should succeed");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  for i in 0..5_500u64 {
    let idx = i as f64;
    let ptr = match i % 5 {
      0 => {
        let p = alloc_string.call(&mut store, idx).expect("alloc-string should succeed");
        let s = read_wasm_str(&mut store, &instance, p);
        assert!(s.starts_with(&format!("item-{i}")), "unexpected alloc-string result `{s}`");
        assert_eq!(s.len(), 5 + 128);
        p
      }
      1 => {
        let p = alloc_list.call(&mut store, idx).expect("alloc-list should succeed");
        assert_eq!(read_wasm_str(&mut store, &instance, p), format!("L1-{i}#3"));
        p
      }
      2 => {
        let p = alloc_map.call(&mut store, idx).expect("alloc-map should succeed");
        assert_eq!(read_wasm_str(&mut store, &instance, p), format!("M2-{i}"));
        p
      }
      3 => {
        let p = alloc_struct.call(&mut store, idx).expect("alloc-struct should succeed");
        assert_eq!(read_wasm_str(&mut store, &instance, p), format!("S-{i}:{i}"));
        p
      }
      _ => {
        let p = alloc_enum.call(&mut store, idx).expect("alloc-enum should succeed");
        assert_eq!(read_wasm_str(&mut store, &instance, p), format!("E-{i}/{i}"));
        p
      }
    };
    assert!(
      ptr >= f64::from(arena_base),
      "scratch result pointer {ptr} below arena_base {arena_base}"
    );
    assert!(
      get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base,
      "Scratch result must stay live until host calls __arena_reset at iteration {i}"
    );
    arena_reset.call(&mut store, ()).expect("__arena_reset should succeed");
    assert_eq!(
      get_i32_global(&mut store, &instance, "__heap_ptr"),
      arena_base,
      "heap watermark did not reset at iteration {i}"
    );
    assert_eq!(
      memory.size(&store),
      initial_pages,
      "memory pages grew unexpectedly at iteration {i}"
    );
  }
}

#[test]
fn core_wasm_defref_promotes_all_heap_types_across_arena_reset() {
  let dir = TestDirectory::create("wasm-defref-promote");
  let snapshot = r#"
{}
  :about "|Test snapshot for Core WASM defref promotion across all heap types."
  :package |defref-promote
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'defref-promote.main/main!) (:mode :native) (:reload-fn 'defref-promote.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'defref-promote.main
    %{} 'FileEntry
      :defs $ {}
        'Item $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Item (:name String) (:count Number)
          :examples $ []
          :schema $ quote StructDef
        'Msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Msg (:Empty) (:Text String) (:Tagged Number String)
          :examples $ []
          :schema $ quote EnumDef
        'Envelope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Envelope (:title String) (:buckets (Map String (List defref-promote.main/Msg)))
          :examples $ []
          :schema $ quote StructDef
        '*r-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-str |init-str
          :examples $ []
          :schema $ quote $ Ref String
        '*r-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-list ([] |seed)
          :examples $ []
          :schema $ quote $ Ref (List String)
        '*r-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-map ({} (|k0 |v0))
          :examples $ []
          :schema $ quote $ Ref (Map String String)
        '*r-set $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-set (#{} |s0)
          :examples $ []
          :schema $ quote $ Ref (Set String)
        '*r-struct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-struct (Item :name |init :count 0)
          :examples $ []
          :schema $ quote $ Ref defref-promote.main/Item
        '*r-enum $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-enum (Msg :Empty)
          :examples $ []
          :schema $ quote $ Ref defref-promote.main/Msg
        '*r-nested $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *r-nested (Envelope :title |init :buckets ({}))
          :examples $ []
          :schema $ quote $ Ref defref-promote.main/Envelope
        'populate-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export populate-all (n)
            let
                s $ to-string n
              reset! *r-str $ &str:concat |reset- s
              swap! *r-str (fn (cur) (&str:concat cur |-swapped))
              reset! *r-list $ [] (&str:concat |alpha- s) (&str:concat |beta- s)
              reset! *r-map $ &map:assoc (&{} |k0 (&str:concat |v0- s)) |k1 (&str:concat |v1- s)
              swap! *r-map (fn (m) (&map:assoc m |k2 (&str:concat |v2- s)))
              reset! *r-set $ &include (#{} (&str:concat |s1- s)) (&str:concat |s2- s)
              reset! *r-struct $ Item :name (&str:concat |widget- s) :count (&+ n 10)
              reset! *r-enum $ Msg :Tagged (&+ n 1) (&str:concat |ok- s)
              let
                  buckets $ {}
                    |primary $ [] (Msg :Text (&str:concat |msgA- s)) (Msg :Tagged n (&str:concat |msgB- s))
                    |secondary $ [] (Msg :Empty)
                reset! *r-nested $ Envelope :title (&str:concat |env- s) :buckets buckets
              , 1
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'thrash-arena $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export thrash-arena (rounds)
            let
                loop-fn $ fn (k acc)
                  hint-fn $ Fn Number String (-> Number)
                  if (&<= k 0) (&str:count acc)
                    let
                        noise $ &str:pad-right (&str:concat |noise- (to-string k)) 512 |Z
                      recur (&- k 1) (&str:slice noise 0 32)
              loop-fn rounds |start
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'verify-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-all ()
            let
                str-v @*r-str
                list-v $ &str:concat (&list:nth @*r-list 0) (&str:concat |+ (&list:nth @*r-list 1))
                m0 $ .unwrap-or (.get @*r-map |k0) |missing
                m2 $ .unwrap-or (.get @*r-map |k2) |missing
                map-v $ &str:concat m0 (&str:concat |+ m2)
                set-v $ if (&set:includes? @*r-set |s2-7) |set-ok |set-bad
                struct-v $ &str:concat (:name @*r-struct) (&str:concat |: (to-string (:count @*r-struct)))
                enum-v $ match @*r-enum
                  (:Empty) |empty
                  (:Text t) t
                  (:Tagged c t) (&str:concat t (&str:concat |/ (to-string c)))
                nested-list $ match (.get (:buckets @*r-nested) |primary)
                  (:some xs) xs
                  (:none) ([])
                nested-second $ &list:nth nested-list 1
                nested-second-s $ match nested-second
                  (:Empty) |empty
                  (:Text t) t
                  (:Tagged c t) (&str:concat t (&str:concat |/ (to-string c)))
                nested-v $ &str:concat (:title @*r-nested) (&str:concat |: nested-second-s)
              &str:concat str-v
                &str:concat |; $ &str:concat list-v
                  &str:concat |; $ &str:concat map-v
                    &str:concat |; $ &str:concat set-v
                      &str:concat |; $ &str:concat struct-v
                        &str:concat |; $ &str:concat enum-v (&str:concat |; nested-v)
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns defref-promote.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let populate = instance
    .get_typed_func::<f64, f64>(&mut store, "populate-all")
    .expect("populate-all export");
  let thrash = instance
    .get_typed_func::<f64, f64>(&mut store, "thrash-arena")
    .expect("thrash-arena export");
  let verify = instance
    .get_typed_func::<(), f64>(&mut store, "verify-all")
    .expect("verify-all export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");
  let str_new = instance
    .get_typed_func::<(i32, i32), f64>(&mut store, "__str_new")
    .expect("__str_new export");

  assert_eq!(populate.call(&mut store, 7.0).expect("populate should succeed"), 1.0);
  // Overwrite Scratch Arena heavily and trigger both turn reset and explicit host `__arena_reset`.
  assert_eq!(thrash.call(&mut store, 400.0).expect("thrash should succeed"), 32.0);
  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  memory
    .write(&mut store, (arena_base + 16) as usize, b"host-scratch-noise")
    .expect("host write to scratch arena should succeed");
  let _ = str_new.call(&mut store, (arena_base + 16, 18)).expect("__str_new should succeed");
  assert!(get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base);
  arena_reset.call(&mut store, ()).expect("__arena_reset should succeed");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  let summary_ptr = verify.call(&mut store, ()).expect("verify-all should succeed");
  let summary = read_wasm_str(&mut store, &instance, summary_ptr);
  assert_eq!(
    summary,
    "reset-7-swapped;alpha-7+beta-7;v0-7+v2-7;set-ok;widget-7:17;ok-7/8;env-7:msgB-7/7"
  );
}

#[test]
fn core_wasm_persistent_heap_compacts_across_semispaces_under_8mib_limit() {
  let dir = TestDirectory::create("wasm-persistent-compact");
  let snapshot = r#"
{}
  :about "|Test snapshot for Persistent Heap semi-space compaction."
  :package |persistent-compact
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'persistent-compact.main/main!) (:mode :native) (:reload-fn 'persistent-compact.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'persistent-compact.main
    %{} 'FileEntry
      :defs $ {}
        '*p-primary $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *p-primary |init
          :examples $ []
          :schema $ quote $ Ref String
        '*p-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *p-map ({})
          :examples $ []
          :schema $ quote $ Ref (Map String String)
        '*p-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *p-list ([])
          :examples $ []
          :schema $ quote $ Ref (List String)
        'init-seeds $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export init-seeds ()
            reset! *p-map $ {} (|k0 |seed-0) (|k1 |seed-1) (|k2 |seed-2)
            reset! *p-list $ [] |seed-a |seed-b
            , 1
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'step-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export step-state (n)
            let
                s $ to-string n
                big $ &str:pad-right (&str:concat |step- s) 512 |Q
              reset! *p-primary big
              swap! *p-map (fn (m) (&map:assoc m |latest big))
              reset! *p-list $ [] (&str:concat |L- s) big
              &str:count @*p-primary
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'read-state-summary $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-state-summary ()
            let
                k0 $ .unwrap-or (.get @*p-map |k0) |missing
                k1 $ .unwrap-or (.get @*p-map |k1) |missing
                k2 $ .unwrap-or (.get @*p-map |k2) |missing
                latest $ .unwrap-or (.get @*p-map |latest) |missing
                l0 $ &list:nth @*p-list 0
              &str:concat k0
                &str:concat |: $ &str:concat k1
                  &str:concat |: $ &str:concat k2
                    &str:concat |: $ &str:concat l0
                      &str:concat |: $ &str:slice latest 0 12
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns persistent-compact.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  let initial_pages = memory.size(&store);
  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let init_seeds = instance
    .get_typed_func::<(), f64>(&mut store, "init-seeds")
    .expect("init-seeds export");
  assert_eq!(init_seeds.call(&mut store, ()).expect("init-seeds should succeed"), 1.0);
  let step_state = instance
    .get_typed_func::<f64, f64>(&mut store, "step-state")
    .expect("step-state export");
  let read_summary = instance
    .get_typed_func::<(), f64>(&mut store, "read-state-summary")
    .expect("read-state-summary export");

  let mut prev_space = get_i32_global(&mut store, &instance, "__persistent_ptr") >= space_b_base;
  let mut space_flips = 0u32;

  for i in 1..=5_500u64 {
    let count = step_state.call(&mut store, i as f64).expect("step-state should succeed");
    assert_eq!(count, 512.0);
    let pers_ptr = get_i32_global(&mut store, &instance, "__persistent_ptr");
    assert!(
      pers_ptr >= space_a_base && pers_ptr <= arena_base,
      "__persistent_ptr={pers_ptr} escaped persistent region [{space_a_base}..{arena_base}]"
    );
    let cur_space = pers_ptr >= space_b_base;
    if cur_space != prev_space {
      space_flips += 1;
      prev_space = cur_space;
    }
    assert_eq!(
      get_i32_global(&mut store, &instance, "__heap_ptr"),
      arena_base,
      "__heap_ptr failed to reset at iteration {i}"
    );
    assert_eq!(memory.size(&store), initial_pages, "memory grew at iteration {i}");
  }

  assert!(
    space_flips >= 2,
    "expected at least 2 semi-space compaction flips, observed {space_flips}"
  );
  let summary_ptr = read_summary.call(&mut store, ()).expect("read-state-summary should succeed");
  let summary = read_wasm_str(&mut store, &instance, summary_ptr);
  assert_eq!(summary, "seed-0:seed-1:seed-2:L-5500:step-5500QQQ");
}

#[test]
fn core_wasm_same_turn_old_ref_and_compaction_return_value_survive() {
  let dir = TestDirectory::create("wasm-same-turn-old-ref");
  let snapshot = r#"
{}
  :about "|Test snapshot for same-turn old deref survival and compaction return relocation."
  :package |same-turn-old
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'same-turn-old.main/main!) (:mode :native) (:reload-fn 'same-turn-old.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'same-turn-old.main
    %{} 'FileEntry
      :defs $ {}
        '*state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *state |seed-initial
          :examples $ []
          :schema $ quote $ Ref String
        'swap-and-check-same-turn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export swap-and-check-same-turn (next-label)
            let
                old @*state
              reset! *state next-label
              &str:concat old (&str:concat |-> @*state)
          :examples $ []
          :schema $ quote $ Fn String (-> String)
        'bump-large $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export bump-large (n size)
            let
                tag $ &str:concat |v- (to-string n)
              reset! *state $ &str:pad-right tag size |P
              &str:count @*state
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'reset-and-return-new $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export reset-and-return-new (n size)
            let
                tag $ &str:concat |new- (to-string n)
              reset! *state $ &str:pad-right tag size |N
              , @*state
          :examples $ []
          :schema $ quote $ Fn Number Number (-> String)
        'reset-and-return-old $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export reset-and-return-old (n size)
            let
                old @*state
                tag $ &str:concat |next- (to-string n)
              reset! *state $ &str:pad-right tag size |M
              , old
          :examples $ []
          :schema $ quote $ Fn Number Number (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns same-turn-old.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let same_turn = instance
    .get_typed_func::<f64, f64>(&mut store, "swap-and-check-same-turn")
    .expect("swap-and-check-same-turn export");
  let str_new = instance
    .get_typed_func::<(i32, i32), f64>(&mut store, "__str_new")
    .expect("__str_new export");
  let bump_large = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "bump-large")
    .expect("bump-large export");
  let return_new = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "reset-and-return-new")
    .expect("reset-and-return-new export");
  let return_old = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "reset-and-return-old")
    .expect("reset-and-return-old export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");

  // (a) Same-turn `old` vs `@*state` after `reset!`
  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  memory
    .write(&mut store, (arena_base + 16) as usize, b"turn-two")
    .expect("write input arg");
  let arg_ptr = str_new.call(&mut store, (arena_base + 16, 8)).expect("__str_new");
  let pair_ptr = same_turn.call(&mut store, arg_ptr).expect("same_turn call");
  assert_eq!(read_wasm_str(&mut store, &instance, pair_ptr), "seed-initial->turn-two");
  assert!(get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base);
  arena_reset.call(&mut store, ()).expect("__arena_reset after same_turn");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  // (b1) Pump persistent heap near 512 KiB compaction threshold, then reset + return `@*state`.
  for i in 1..=25u64 {
    bump_large
      .call(&mut store, (i as f64, 20_480.0))
      .expect("bump-large should succeed");
  }
  let before_flip_a = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert!(before_flip_a >= space_a_base && before_flip_a < space_b_base);
  let mut flipped = false;
  for k in 26..=35u64 {
    let ret = return_new
      .call(&mut store, (k as f64, 24_576.0))
      .expect("reset-and-return-new should succeed");
    let ret_text = read_wasm_str(&mut store, &instance, ret);
    assert_eq!(ret_text.len(), 24_576);
    assert!(ret_text.starts_with(&format!("new-{k}N")));
    let ptr_now = get_i32_global(&mut store, &instance, "__persistent_ptr");
    if ptr_now >= space_b_base {
      assert!(
        ret >= f64::from(space_b_base) && ret < f64::from(arena_base),
        "returned @*state pointer {ret} must be relocated into active semi-space B [{space_b_base}..{arena_base})"
      );
      flipped = true;
      break;
    }
  }
  assert!(flipped, "expected return_new turn to trigger semi-space A -> B compaction");

  // (b2) Pump persistent heap in semi-space B near 512 KiB, then capture `old` and return `old` on compaction turn!
  for i in 1..=24u64 {
    bump_large
      .call(&mut store, (100.0 + i as f64, 20_480.0))
      .expect("bump-large in space B should succeed");
  }
  let initial_tag = format!("v-{}", 100 + 24);
  let mut expected_old = format!("{initial_tag}{}", "P".repeat(20_480usize - initial_tag.len()));
  let mut old_survived_compaction = false;
  for k in 25..=35u64 {
    let ret_old = return_old
      .call(&mut store, (200.0 + k as f64, 24_576.0))
      .expect("reset-and-return-old should succeed");
    let old_text = read_wasm_str(&mut store, &instance, ret_old);
    assert_eq!(old_text, expected_old, "returned `old` value corrupted at k={k}");
    let ptr_now = get_i32_global(&mut store, &instance, "__persistent_ptr");
    if ptr_now < space_b_base {
      assert!(
        ret_old >= f64::from(space_a_base) && ret_old < f64::from(space_b_base),
        "returned `old` pointer {ret_old} must be relocated into new active semi-space A [{space_a_base}..{space_b_base})"
      );
      old_survived_compaction = true;
      break;
    }
    let next_tag = format!("next-{}", 200 + k);
    expected_old = format!("{next_tag}{}", "M".repeat(24_576usize - next_tag.len()));
  }
  assert!(
    old_survived_compaction,
    "expected return_old turn to trigger semi-space B -> A compaction and relocate `old`"
  );
}

#[test]
fn core_wasm_mid_turn_persistent_overflow_compacts_cleanly_at_turn_end() {
  let dir = TestDirectory::create("wasm-mid-turn-overflow");
  let snapshot = r#"
{}
  :about "|Test snapshot for mid-turn persistent overflow spilling to Scratch Arena and compacting at turn end."
  :package |mid-turn-overflow
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'mid-turn-overflow.main/main!) (:mode :native) (:reload-fn 'mid-turn-overflow.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'mid-turn-overflow.main
    %{} 'FileEntry
      :defs $ {}
        '*overflow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *overflow |init
          :examples $ []
          :schema $ quote $ Ref String
        'flood-in-one-turn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export flood-in-one-turn (steps)
            let
                loop-fn $ fn (k)
                  hint-fn $ Fn Number (-> Number)
                  if (&> k steps) (&str:count @*overflow)
                    let
                        payload $ &str:pad-right (&str:concat |step- (to-string k)) 1024 |X
                      reset! *overflow payload
                      recur (&+ k 1)
              loop-fn 1
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'read-overflow $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-overflow () @*overflow
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns mid-turn-overflow.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;
  assert_eq!(get_i32_global(&mut store, &instance, "__persistent_ptr"), space_a_base);

  let flood = instance
    .get_typed_func::<f64, f64>(&mut store, "flood-in-one-turn")
    .expect("flood-in-one-turn export");
  let read_overflow = instance
    .get_typed_func::<(), f64>(&mut store, "read-overflow")
    .expect("read-overflow export");

  // 1,500 * 1,024 bytes (~1.5 MiB) > 1 MiB semi-space inside a single export turn.
  let final_len = flood.call(&mut store, 1_500.0).expect("mid-turn overflow should compact cleanly");
  assert_eq!(final_len, 1024.0);

  let pers_after = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert!(
    pers_after > space_b_base && pers_after - space_b_base < 4096,
    "expected turn-end compaction into semi-space B leaving only the live ~1 KiB string, got pers_after={pers_after} (space_b_base={space_b_base})"
  );
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  let live_ptr = read_overflow.call(&mut store, ()).expect("read-overflow should succeed");
  let live_str = read_wasm_str(&mut store, &instance, live_ptr);
  assert_eq!(live_str.len(), 1024);
  assert!(live_str.starts_with("step-1500X"));
}

#[test]
fn core_wasm_static_defref_initializers_and_stale_magic_type_of() {
  let dir = TestDirectory::create("wasm-static-defref-and-magic");
  let snapshot = r#"
{}
  :about "|Test snapshot for static defref initializers and stale-magic safety in type-of."
  :package |static-init-magic
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'static-init-magic.main/main!) (:mode :native) (:reload-fn 'static-init-magic.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'static-init-magic.main
    %{} 'FileEntry
      :defs $ {}
        '*s-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-str |static-hello
          :examples $ []
          :schema $ quote $ Ref String
        '*s-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-tag :ready
          :examples $ []
          :schema $ quote $ Ref Tag
        '*s-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-list ([])
          :examples $ []
          :schema $ quote $ Ref (List Number)
        '*s-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-map ({})
          :examples $ []
          :schema $ quote $ Ref (Map String Number)
        '*s-set $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-set (#{})
          :examples $ []
          :schema $ quote $ Ref (Set Number)
        '*s-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-num 42.5
          :examples $ []
          :schema $ quote $ Ref Number
        '*s-bool $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *s-bool true
          :examples $ []
          :schema $ quote $ Ref Bool
        '*dyn-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *dyn-str |initial-dyn
          :examples $ []
          :schema $ quote $ Ref String
        '*pump $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *pump |pump-init
          :examples $ []
          :schema $ quote $ Ref String
        'verify-static-inits $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export verify-static-inits ()
            let
                s-ok $ &= (&str:count @*s-str) 12
                tag-ok $ &= @*s-tag :ready
                list-ok $ &= (&list:count @*s-list) 0
                map-ok $ &= (&map:count @*s-map) 0
                set-ok $ &= (&set:count @*s-set) 0
                num-ok $ &= @*s-num 42.5
                bool-ok $ if @*s-bool true false
              if
                and s-ok tag-ok list-ok map-ok set-ok num-ok bool-ok
                , @*s-str |bad
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'set-dyn-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export set-dyn-str (n)
            reset! *dyn-str $ &str:concat |dyn- (to-string n)
            , @*dyn-str
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'get-dyn-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export get-dyn-str () , @*dyn-str
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'make-scratch-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export make-scratch-str (n)
            &str:concat |scratch- (to-string n)
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'pump-step $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-step (n)
            reset! *pump $ &str:pad-right (&str:concat |pump- (to-string n)) 32768 |Z
            &str:count @*pump
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'inspect-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export inspect-tag (v)
            if (&= (type-of v) :string) 1
              if (&= (type-of v) :number) 2 0
          :examples $ []
          :schema $ quote $ Fn String (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns static-init-magic.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let verify_static = instance
    .get_typed_func::<(), f64>(&mut store, "verify-static-inits")
    .expect("verify-static-inits export");
  let set_dyn_str = instance
    .get_typed_func::<f64, f64>(&mut store, "set-dyn-str")
    .expect("set-dyn-str export");
  let get_dyn_str = instance
    .get_typed_func::<(), f64>(&mut store, "get-dyn-str")
    .expect("get-dyn-str export");
  let make_scratch = instance
    .get_typed_func::<f64, f64>(&mut store, "make-scratch-str")
    .expect("make-scratch-str export");
  let pump_step = instance
    .get_typed_func::<f64, f64>(&mut store, "pump-step")
    .expect("pump-step export");
  let inspect_tag = instance
    .get_typed_func::<f64, f64>(&mut store, "inspect-tag")
    .expect("inspect-tag export");

  // (a) Static initializers work before any `reset!`.
  let static_str_ptr = verify_static.call(&mut store, ()).expect("verify-static-inits");
  assert!(static_str_ptr < f64::from(space_a_base));
  assert_eq!(read_wasm_str(&mut store, &instance, static_str_ptr), "static-hello");
  assert_eq!(inspect_tag.call(&mut store, static_str_ptr).expect("inspect static"), 1.0);

  // (b) Stale magic in inactive semi-space & reset Scratch Arena.
  let old_semi_a_ptr = set_dyn_str.call(&mut store, 77.0).expect("set-dyn-str");
  assert!(old_semi_a_ptr >= f64::from(space_a_base) && old_semi_a_ptr < f64::from(space_b_base));
  assert_eq!(inspect_tag.call(&mut store, old_semi_a_ptr).expect("inspect active semi A"), 1.0);

  // Trigger compaction from semi-space A -> semi-space B (32 KiB * 18 > 512 KiB).
  let mut compacted = false;
  for i in 1..=20 {
    pump_step.call(&mut store, i as f64).expect("pump-step");
    if get_i32_global(&mut store, &instance, "__persistent_ptr") >= space_b_base {
      compacted = true;
      break;
    }
  }
  assert!(compacted, "expected persistent compaction into semi-space B");

  // Compaction zeroes the alternate semi-space, and even if stale HEAP_MAGIC + string tag bytes exist
  // in that inactive semi-space, inspect-tag must classify old_semi_a_ptr as :number (2.0).
  assert_eq!(read_header_magic(&mut store, &instance, old_semi_a_ptr), 0);
  let string_tag = get_i32_global(&mut store, &instance, "__string_tag");
  let old_header_addr = (old_semi_a_ptr as usize) - 8;
  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  memory
    .write(&mut store, old_header_addr, &HEAP_MAGIC.to_le_bytes())
    .expect("write synthetic stale HEAP_MAGIC");
  memory
    .write(&mut store, old_header_addr + 4, &string_tag.to_le_bytes())
    .expect("write synthetic stale string tag");
  assert_eq!(read_header_magic(&mut store, &instance, old_semi_a_ptr), HEAP_MAGIC);
  assert_eq!(inspect_tag.call(&mut store, old_semi_a_ptr).expect("inspect stale semi A"), 2.0);

  let new_semi_b_ptr = get_dyn_str.call(&mut store, ()).expect("get-dyn-str");
  assert!(new_semi_b_ptr >= f64::from(space_b_base) && new_semi_b_ptr < f64::from(arena_base));
  assert_eq!(read_wasm_str(&mut store, &instance, new_semi_b_ptr), "dyn-77");
  assert_eq!(inspect_tag.call(&mut store, new_semi_b_ptr).expect("inspect active semi B"), 1.0);

  // Make a Scratch Arena string; it remains live in [arena_base, __heap_ptr) until __arena_reset().
  let stale_scratch_ptr = make_scratch.call(&mut store, 99.0).expect("make-scratch-str");
  assert!(get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base);
  assert_eq!(inspect_tag.call(&mut store, stale_scratch_ptr).expect("inspect live scratch"), 1.0);
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");
  arena_reset.call(&mut store, ()).expect("__arena_reset should succeed");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
  assert_eq!(read_header_magic(&mut store, &instance, stale_scratch_ptr), HEAP_MAGIC);
  assert_eq!(inspect_tag.call(&mut store, stale_scratch_ptr).expect("inspect stale scratch"), 2.0);
}

#[tokio::test]
async fn component_wasm_repeated_exports_and_defref_stay_within_8mib_limit() {
  let dir = TestDirectory::create("component-wasm-memory-arena");
  let snapshot = r#"
{}
  :about "|Test snapshot for Component WASM memory reuse and defref promotion under 8 MiB StoreLimits."
  :package |component-arena
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'component-arena.main/main!) (:mode :native) (:reload-fn 'component-arena.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'component-arena.main
    %{} 'FileEntry
      :defs $ {}
        '*comp-log $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *comp-log |seed
          :examples $ []
          :schema $ quote $ Ref String
        'record-and-read $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export record-and-read (msg)
            let
                padded $ &str:pad-right (&str:concat |C- msg) 320 |R
              reset! *comp-log padded
              &str:slice @*comp-log 0 12
          :examples $ []
          :schema $ quote $ Fn String (-> String)
        'peek-log $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export peek-log ()
            let
                scratch $ &str:pad-right @*comp-log 320 |Z
              &str:slice scratch 0 12
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'append-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export append-count (msg)
            let
                scratch $ &str:pad-right (&str:concat |W- msg) 6144 |W
                padded $ &str:pad-right (&str:concat |S- msg) 320 |K
              reset! *comp-log padded
              &+ (&str:count @*comp-log) (&- (&str:count scratch) 6144)
          :examples $ []
          :schema $ quote $ Fn String (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns component-arena.main
"#;
  let snapshot_path = dir.0.join("calcit.cirru");
  fs::write(&snapshot_path, snapshot).expect("Component arena snapshot should write");
  let out_dir = dir.0.join("out");
  fs::create_dir(&out_dir).expect("Component output directory should create");

  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["wasm", "--boundary", "component", "--emit-path"])
    .arg(&out_dir)
    .output()
    .expect("Component arena fixture should compile");
  assert!(
    compile.status.success(),
    "Component arena compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let contract_out = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["ffi", "export", "--boundary", "component"])
    .output()
    .expect("Component arena contract should export");
  assert!(
    contract_out.status.success(),
    "Component arena contract export failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&contract_out.stdout),
    String::from_utf8_lossy(&contract_out.stderr)
  );
  let contract_path = out_dir.join("interface.cirru");
  fs::write(&contract_path, contract_out.stdout).expect("Component contract should write");
  let contract = calcit_bindgen::load_contract(&contract_path).expect("Component contract should load");
  let generated = out_dir.join("generated");
  calcit_bindgen::generate_contract_directory(&contract, Some(&out_dir.join("program.wasm")), &generated, &[])
    .expect("Component arena fixture should package");

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_more_async_builtins(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime Component engine should create");
  let component = Component::from_file(&engine, generated.join(COMPONENT_FILE)).expect("Component should load");
  let linker = ComponentLinker::new(&engine);
  let limits = StoreLimitsBuilder::new().memory_size(EIGHT_MIB).build();
  let mut store = Store::new(&engine, limits);
  store.limiter(|state| state);

  let instance = linker
    .instantiate_async(&mut store, &component)
    .await
    .expect("Component should instantiate within 8 MiB limit");
  let record_and_read = instance.get_func(&mut store, "record-and-read").expect("record-and-read export");
  let peek_log = instance.get_func(&mut store, "peek-log").expect("peek-log export");
  let append_count = instance.get_func(&mut store, "append-count").expect("append-count export");

  // 4,800 invocations alternating String (post-return), read-only peek-log, and Number (no post-return) exports.
  for i in 0..1_600u64 {
    let mut str_result = [ComponentVal::Bool(false)];
    record_and_read
      .call_async(&mut store, &[ComponentVal::String(format!("m{i:04}"))], &mut str_result)
      .await
      .unwrap_or_else(|err| panic!("record-and-read failed at iteration {i}: {err}"));
    assert_eq!(
      str_result,
      [ComponentVal::String(format!("C-m{i:04}RRRRR"))],
      "unexpected record-and-read result at iteration {i}"
    );

    let mut peek_result = [ComponentVal::Bool(false)];
    peek_log
      .call_async(&mut store, &[], &mut peek_result)
      .await
      .unwrap_or_else(|err| panic!("peek-log failed at iteration {i}: {err}"));
    assert_eq!(
      peek_result,
      [ComponentVal::String(format!("C-m{i:04}RRRRR"))],
      "peek-log must read current *comp-log after record-and-read at iteration {i}"
    );

    let mut num_result = [ComponentVal::Bool(false)];
    append_count
      .call_async(&mut store, &[ComponentVal::String(format!("s{i:04}"))], &mut num_result)
      .await
      .unwrap_or_else(|err| panic!("append-count failed at iteration {i}: {err}"));
    assert_eq!(
      num_result,
      [ComponentVal::Float64(320.0)],
      "unexpected append-count result at iteration {i}"
    );
  }

  // Also instantiate raw Component core module to assert __heap_ptr == __arena_base and flat memory.size() after turns.
  let (mut raw_store, raw_instance) = instantiate_core_with_limit(&out_dir.join("program.wasm"), EIGHT_MIB);
  let raw_memory = raw_instance.get_memory(&mut raw_store, "memory").expect("raw memory export");
  let raw_initial_pages = raw_memory.size(&raw_store);
  let raw_arena_base = get_i32_global(&mut raw_store, &raw_instance, "__arena_base");
  let raw_cabi_realloc = raw_instance
    .get_typed_func::<(i32, i32, i32, i32), i32>(&mut raw_store, "cabi_realloc")
    .expect("cabi_realloc export");
  let raw_record = raw_instance
    .get_typed_func::<(i32, i32), i32>(&mut raw_store, "record-and-read")
    .expect("raw record-and-read");
  let raw_record_post = raw_instance
    .get_typed_func::<i32, ()>(&mut raw_store, "cabi_post_record-and-read")
    .expect("raw cabi_post_record-and-read");
  let raw_peek = raw_instance
    .get_typed_func::<(), i32>(&mut raw_store, "peek-log")
    .expect("raw peek-log");
  let raw_peek_post = raw_instance
    .get_typed_func::<i32, ()>(&mut raw_store, "cabi_post_peek-log")
    .expect("raw cabi_post_peek-log");
  let raw_append = raw_instance
    .get_typed_func::<(i32, i32), f64>(&mut raw_store, "append-count")
    .expect("raw append-count");

  for i in 0..100u64 {
    let msg = format!("m{i:04}");
    let ptr0 = raw_cabi_realloc
      .call(&mut raw_store, (0, 0, 1, msg.len() as i32))
      .expect("alloc msg0");
    raw_memory.write(&mut raw_store, ptr0 as usize, msg.as_bytes()).expect("write msg0");
    let ret0 = raw_record.call(&mut raw_store, (ptr0, msg.len() as i32)).expect("raw record");
    raw_record_post.call(&mut raw_store, ret0).expect("raw record post-return");
    assert_eq!(get_i32_global(&mut raw_store, &raw_instance, "__heap_ptr"), raw_arena_base);
    assert_eq!(raw_memory.size(&raw_store), raw_initial_pages);

    let ret_peek = raw_peek.call(&mut raw_store, ()).expect("raw peek");
    raw_peek_post.call(&mut raw_store, ret_peek).expect("raw peek post-return");
    assert_eq!(get_i32_global(&mut raw_store, &raw_instance, "__heap_ptr"), raw_arena_base);
    assert_eq!(raw_memory.size(&raw_store), raw_initial_pages);

    let ptr1 = raw_cabi_realloc
      .call(&mut raw_store, (0, 0, 1, msg.len() as i32))
      .expect("alloc msg1");
    raw_memory.write(&mut raw_store, ptr1 as usize, msg.as_bytes()).expect("write msg1");
    let count = raw_append.call(&mut raw_store, (ptr1, msg.len() as i32)).expect("raw append");
    assert_eq!(count, 320.0);
    assert_eq!(get_i32_global(&mut raw_store, &raw_instance, "__heap_ptr"), raw_arena_base);
    assert_eq!(raw_memory.size(&raw_store), raw_initial_pages);
  }
}

#[test]
fn core_wasm_proven_scalar_reset_and_compaction_preserve_number() {
  let dir = TestDirectory::create("wasm-proven-scalar-reset");
  let snapshot = r#"
{}
  :about "|Test snapshot for proven-scalar defref reset bypass and compaction skip."
  :package |scalar-reset
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'scalar-reset.main/main!) (:mode :native) (:reload-fn 'scalar-reset.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'scalar-reset.main
    %{} 'FileEntry
      :defs $ {}
        '*scalar-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *scalar-num 0
          :examples $ []
          :schema $ quote $ Ref Number
        '*box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *box |init
          :examples $ []
          :schema $ quote $ Ref String
        'write-scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export write-scalar (v)
            reset! *scalar-num v
            , @*scalar-num
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'read-scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-scalar () , @*scalar-num
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'pump-box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-box (k size)
            reset! *box $ &str:pad-right (&str:concat |b- (to-string k)) size |B
            &str:count @*box
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'read-box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-box () , @*box
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns scalar-reset.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let write_scalar = instance
    .get_typed_func::<f64, f64>(&mut store, "write-scalar")
    .expect("write-scalar export");
  let read_scalar = instance
    .get_typed_func::<(), f64>(&mut store, "read-scalar")
    .expect("read-scalar export");
  let pump_box = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "pump-box")
    .expect("pump-box export");
  let read_box = instance.get_typed_func::<(), f64>(&mut store, "read-box").expect("read-box export");

  // Capture an active semi-space A pointer numeric value, then store that exact integer in *scalar-num.
  assert_eq!(pump_box.call(&mut store, (1.0, 256.0)).expect("pump-box 1"), 256.0);
  let live_ptr_num = read_box.call(&mut store, ()).expect("read-box ptr");
  assert!(live_ptr_num >= f64::from(space_a_base) && live_ptr_num < f64::from(space_b_base));
  let pers_before = get_i32_global(&mut store, &instance, "__persistent_ptr");

  assert_eq!(
    write_scalar.call(&mut store, live_ptr_num).expect("write-scalar ptr_num"),
    live_ptr_num
  );
  assert_eq!(
    get_i32_global(&mut store, &instance, "__persistent_ptr"),
    pers_before,
    "scalar reset! must bypass __rt_commit_defref"
  );
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  // Trigger compaction A -> B; *scalar-num must NOT be rewritten to a space_b address.
  let mut compacted = false;
  for k in 2..=32u64 {
    pump_box.call(&mut store, (k as f64, 24_576.0)).expect("pump-box loop");
    if get_i32_global(&mut store, &instance, "__persistent_ptr") >= space_b_base {
      compacted = true;
      break;
    }
  }
  assert!(compacted, "expected *box writes to compact semi-space A -> B");
  assert_eq!(
    read_scalar.call(&mut store, ()).expect("read-scalar after compaction"),
    live_ptr_num,
    "proven-scalar defref must retain its exact numeric value across compaction"
  );

  // Writing another scalar does not perturb __persistent_ptr or __heap_ptr.
  let pers_b = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert_eq!(write_scalar.call(&mut store, 123.5).expect("write-scalar 123.5"), 123.5);
  assert_eq!(read_scalar.call(&mut store, ()).expect("read-scalar 123.5"), 123.5);
  assert_eq!(get_i32_global(&mut store, &instance, "__persistent_ptr"), pers_b);
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
}

#[test]
fn core_wasm_shared_scratch_dag_promotes_once_without_corrupting_scratch() {
  let dir = TestDirectory::create("wasm-shared-scratch-dag");
  let snapshot = r#"
{}
  :about "|Test snapshot for shared Scratch DAG promotion deduplication without header mutation."
  :package |shared-dag
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'shared-dag.main/main!) (:mode :native) (:reload-fn 'shared-dag.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'shared-dag.main
    %{} 'FileEntry
      :defs $ {}
        '*dag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *dag $ [] |init
          :examples $ []
          :schema $ quote $ Ref (List String)
        'commit-shared-dag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export commit-shared-dag (pad)
            let
                shared $ &str:pad-right |shared-dag- pad |D
                dag $ [] shared shared shared shared
              reset! *dag dag
              if (&= (type-of shared) :string) , shared (raise |corrupted-scratch-magic)
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'dag-elem-ptr $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export dag-elem-ptr (idx) (&list:nth @*dag idx)
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns shared-dag.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let commit_shared = instance
    .get_typed_func::<f64, f64>(&mut store, "commit-shared-dag")
    .expect("commit-shared-dag export");
  let dag_elem_ptr = instance
    .get_typed_func::<f64, f64>(&mut store, "dag-elem-ptr")
    .expect("dag-elem-ptr export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");

  let scratch_shared = commit_shared.call(&mut store, 32_768.0).expect("commit-shared-dag should succeed");
  assert!(scratch_shared >= f64::from(arena_base));
  assert_eq!(read_header_magic(&mut store, &instance, scratch_shared), HEAP_MAGIC);
  let s = read_wasm_str(&mut store, &instance, scratch_shared);
  assert_eq!(s.len(), 32_768);
  assert!(s.starts_with("shared-dag-D"));

  let pers_after = get_i32_global(&mut store, &instance, "__persistent_ptr");
  let pers_used = pers_after - space_a_base;
  assert!(
    (32_768..40_960).contains(&pers_used),
    "4x shared 32 KiB Scratch string should promote only once, got pers_used={pers_used}"
  );

  arena_reset.call(&mut store, ()).expect("__arena_reset");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);

  let elem0 = dag_elem_ptr.call(&mut store, 0.0).expect("elem 0");
  let elem1 = dag_elem_ptr.call(&mut store, 1.0).expect("elem 1");
  let elem3 = dag_elem_ptr.call(&mut store, 3.0).expect("elem 3");
  assert!(elem0 >= f64::from(space_a_base) && elem0 < f64::from(space_b_base));
  assert_eq!(
    elem0, elem1,
    "shared DAG elements 0 and 1 must deduplicate to the same persistent copy"
  );
  assert_eq!(
    elem0, elem3,
    "shared DAG elements 0 and 3 must deduplicate to the same persistent copy"
  );
  let persisted = read_wasm_str(&mut store, &instance, elem0);
  assert_eq!(persisted.len(), 32_768);
  assert!(persisted.starts_with("shared-dag-D"));
}

#[test]
fn core_wasm_unchanged_large_live_defref_skips_redundant_compaction() {
  let dir = TestDirectory::create("wasm-relative-compact-threshold");
  let snapshot = r#"
{}
  :about "|Test snapshot for relative compaction threshold and no-alloc commit skip."
  :package |rel-compact
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'rel-compact.main/main!) (:mode :native) (:reload-fn 'rel-compact.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'rel-compact.main
    %{} 'FileEntry
      :defs $ {}
        '*big $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *big |init
          :examples $ []
          :schema $ quote $ Ref String
        '*alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *alias |init
          :examples $ []
          :schema $ quote $ Ref String
        'set-big $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export set-big (size)
            reset! *big $ &str:pad-right |BIG- size |B
            &str:count @*big
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'alias-big $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export alias-big ()
            reset! *alias @*big
            &str:count @*alias
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'set-alias-small $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export set-alias-small (k size)
            reset! *alias $ &str:pad-right (&str:concat |S- (to-string k)) size |S
            &str:count @*alias
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns rel-compact.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let set_big = instance.get_typed_func::<f64, f64>(&mut store, "set-big").expect("set-big export");
  let alias_big = instance
    .get_typed_func::<(), f64>(&mut store, "alias-big")
    .expect("alias-big export");
  let set_alias_small = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "set-alias-small")
    .expect("set-alias-small export");

  // Exceed initial 512 KiB threshold so turn end compacts the 600 KiB live string into semi-space B.
  assert_eq!(set_big.call(&mut store, 600_000.0).expect("set-big"), 600_000.0);
  let pers_b = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert!(pers_b >= space_b_base && pers_b < arena_base);

  // Committing an already-persistent ref (@*big) does not advance pers.ptr and must NOT ping-pong back to A.
  for _ in 0..5 {
    assert_eq!(alias_big.call(&mut store, ()).expect("alias-big"), 600_000.0);
    assert_eq!(get_i32_global(&mut store, &instance, "__persistent_ptr"), pers_b);
  }

  // Small allocations well below half the remaining semi-space (~212 KiB) must NOT re-trigger compaction.
  for k in 1..=4u64 {
    assert_eq!(
      set_alias_small.call(&mut store, (k as f64, 16_384.0)).expect("set-alias-small"),
      16_384.0
    );
    let pers_now = get_i32_global(&mut store, &instance, "__persistent_ptr");
    assert!(
      pers_now >= space_b_base && pers_now < arena_base,
      "small commit {k} must stay in semi-space B without redundant compaction (pers_now={pers_now})"
    );
  }

  // Pushing past next_compact_ptr (~212 KiB above pers_b) triggers compaction cleanly back to semi-space A.
  assert_eq!(
    set_alias_small
      .call(&mut store, (5.0, 260_000.0))
      .expect("set-alias-small threshold"),
    260_000.0
  );
  let pers_a = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert!(
    pers_a >= space_a_base && pers_a < space_b_base,
    "expected threshold-crossing commit to compact cleanly back into semi-space A, got pers_a={pers_a}"
  );
}

#[test]
fn core_wasm_compaction_trap_leaves_old_roots_valid_and_recoverable() {
  let dir = TestDirectory::create("wasm-compact-trap-atomic");
  let snapshot = r#"
{}
  :about "|Test snapshot for atomic compaction staging on semi-space overflow trap."
  :package |compact-trap
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'compact-trap.main/main!) (:mode :native) (:reload-fn 'compact-trap.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'compact-trap.main
    %{} 'FileEntry
      :defs $ {}
        '*a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *a |seed-a
          :examples $ []
          :schema $ quote $ Ref String
        '*b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *b |seed-b
          :examples $ []
          :schema $ quote $ Ref String
        'set-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export set-a (size)
            reset! *a $ &str:pad-right |A-LIVE- size |A
            &str:count @*a
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'set-b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export set-b (size)
            reset! *b $ &str:pad-right |B-OVERFLOW- size |B
            &str:count @*b
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'recover-and-read-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export recover-and-read-a ()
            reset! *b |recovered
            , @*a
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'read-b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-b () , @*b
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns compact-trap.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;
  let space_a_base = arena_base - 2 * SEMI_SPACE_BYTES;

  let set_a = instance.get_typed_func::<f64, f64>(&mut store, "set-a").expect("set-a export");
  let set_b = instance.get_typed_func::<f64, f64>(&mut store, "set-b").expect("set-b export");
  let recover_a = instance
    .get_typed_func::<(), f64>(&mut store, "recover-and-read-a")
    .expect("recover-and-read-a export");
  let read_b = instance.get_typed_func::<(), f64>(&mut store, "read-b").expect("read-b export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");

  assert_eq!(set_a.call(&mut store, 600_000.0).expect("set-a"), 600_000.0);
  let pers_before = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert!(pers_before >= space_b_base && pers_before < arena_base);

  // Promoting *b (600 KiB) alongside *a (600 KiB) exceeds 1 MiB semi-space during staged compaction and must trap.
  let trap_err = set_b
    .call(&mut store, 600_000.0)
    .expect_err("600K + 600K live defrefs must trap during compaction staging");
  assert!(
    format!("{trap_err:?}").contains("unreachable"),
    "expected wasm unreachable trap on semi-space OOM, got {trap_err:?}"
  );
  assert_eq!(
    get_i32_global(&mut store, &instance, "__persistent_ptr"),
    pers_before,
    "failed staged compaction must not publish partial persistent metadata"
  );

  // Shrinking *b back to static string and completing turn-end compaction recovers cleanly with *a intact.
  let a_ptr = recover_a.call(&mut store, ()).expect("recover-and-read-a must succeed");
  assert!(a_ptr >= f64::from(space_a_base) && a_ptr < f64::from(space_b_base));
  let a_text = read_wasm_str(&mut store, &instance, a_ptr);
  assert_eq!(a_text.len(), 600_000);
  assert!(a_text.starts_with("A-LIVE-AAA"));
  let b_ptr = read_b.call(&mut store, ()).expect("read-b");
  assert_eq!(read_wasm_str(&mut store, &instance, b_ptr), "recovered");
  arena_reset.call(&mut store, ()).expect("__arena_reset");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
}

#[test]
fn core_wasm_compaction_relocates_old_space_children_of_scratch_return() {
  let dir = TestDirectory::create("wasm-scratch-child-reloc");
  let snapshot = r#"
{}
  :about "|Test snapshot for relocating old-space children referenced by a Scratch return on compaction."
  :package |scratch-child-reloc
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'scratch-child-reloc.main/main!) (:mode :native) (:reload-fn 'scratch-child-reloc.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'scratch-child-reloc.main
    %{} 'FileEntry
      :defs $ {}
        '*state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *state |seed
          :examples $ []
          :schema $ quote $ Ref String
        'pump-state $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-state (n size)
            reset! *state $ &str:pad-right (&str:concat |p- (to-string n)) size |P
            &str:count @*state
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'wrap-old-and-trigger-compact $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export wrap-old-and-trigger-compact (k size)
            let
                old @*state
                tag $ &str:concat |new- (to-string k)
              reset! *state $ &str:pad-right tag size |N
              [] old |scratch-tail
          :examples $ []
          :schema $ quote $ Fn Number Number (-> (List String))
        'inspect-wrapped $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export inspect-wrapped (wrapped)
            &str:concat (&list:nth wrapped 0) (&str:concat |:: (&list:nth wrapped 1))
          :examples $ []
          :schema $ quote $ Fn (List String) (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns scratch-child-reloc.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;

  let pump_state = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "pump-state")
    .expect("pump-state export");
  let wrap_old = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "wrap-old-and-trigger-compact")
    .expect("wrap-old-and-trigger-compact export");
  let inspect_wrapped = instance
    .get_typed_func::<f64, f64>(&mut store, "inspect-wrapped")
    .expect("inspect-wrapped export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");

  for i in 1..=24u64 {
    pump_state
      .call(&mut store, (i as f64, 20_480.0))
      .expect("pump-state should succeed");
  }
  let initial_tag = format!("p-{}", 24);
  let mut expected_old = format!("{initial_tag}{}", "P".repeat(20_480usize - initial_tag.len()));

  let mut compacted = false;
  for k in 25..=35u64 {
    let wrapped_ptr = wrap_old
      .call(&mut store, (k as f64, 24_576.0))
      .expect("wrap-old-and-trigger-compact should succeed");
    assert!(wrapped_ptr >= f64::from(arena_base), "wrapped List must live in Scratch Arena");
    assert!(get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base);
    let combined_ptr = inspect_wrapped
      .call(&mut store, wrapped_ptr)
      .expect("inspect-wrapped should read relocated old child inside Scratch list");
    assert_eq!(
      read_wasm_str(&mut store, &instance, combined_ptr),
      format!("{expected_old}::scratch-tail"),
      "Scratch return child in old semi-space corrupted at k={k}"
    );
    let ptr_now = get_i32_global(&mut store, &instance, "__persistent_ptr");
    arena_reset.call(&mut store, ()).expect("__arena_reset");
    assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
    if ptr_now >= space_b_base {
      compacted = true;
      break;
    }
    let next_tag = format!("new-{k}");
    expected_old = format!("{next_tag}{}", "N".repeat(24_576usize - next_tag.len()));
  }
  assert!(compacted, "expected wrap-old turn to trigger semi-space A -> B compaction");
}

#[test]
fn core_wasm_trapped_export_recovers_arena_reset_and_compaction() {
  let dir = TestDirectory::create("wasm-trap-turn-recovery");
  let snapshot = r#"
{}
  :about "|Test snapshot for post-trap Core export active_turns recovery via turn_enter and __arena_reset."
  :package |trap-recover
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'trap-recover.main/main!) (:mode :native) (:reload-fn 'trap-recover.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'trap-recover.main
    %{} 'FileEntry
      :defs $ {}
        '*log $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defref *log |seed
          :examples $ []
          :schema $ quote $ Ref String
        'pump-log $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export pump-log (n size)
            reset! *log $ &str:pad-right (&str:concat |l- (to-string n)) size |L
            &str:count @*log
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'write-and-trap $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export write-and-trap (n size)
            let
                payload $ &str:pad-right (&str:concat |trapped- (to-string n)) size |T
              reset! *log payload
              if (&> size 0) (raise |boom) 0
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'scalar-probe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export scalar-probe (delta)
            &+ delta (&str:count @*log)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'read-log $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export read-log () , @*log
          :examples $ []
          :schema $ quote $ Fn (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns trap-recover.main
"#;
  let wasm = compile_core_wasm(&dir, snapshot);
  let (mut store, instance) = instantiate_core_with_limit(&wasm, EIGHT_MIB);

  let arena_base = get_i32_global(&mut store, &instance, "__arena_base");
  let space_b_base = arena_base - SEMI_SPACE_BYTES;

  let pump_log = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "pump-log")
    .expect("pump-log export");
  let write_and_trap = instance
    .get_typed_func::<(f64, f64), f64>(&mut store, "write-and-trap")
    .expect("write-and-trap export");
  let scalar_probe = instance
    .get_typed_func::<f64, f64>(&mut store, "scalar-probe")
    .expect("scalar-probe export");
  let read_log = instance.get_typed_func::<(), f64>(&mut store, "read-log").expect("read-log export");
  let arena_reset = instance
    .get_typed_func::<(), ()>(&mut store, "__arena_reset")
    .expect("__arena_reset export");

  for n in 1..=24u64 {
    pump_log.call(&mut store, (n as f64, 20_480.0)).expect("pump-log should succeed");
  }
  assert!(get_i32_global(&mut store, &instance, "__persistent_ptr") < space_b_base);

  // Trap mid-export after crossing the 512 KiB threshold and leaving Scratch & active_turns dirty.
  let err1 = write_and_trap
    .call(&mut store, (99.0, 40_960.0))
    .expect_err("write-and-trap should trap");
  assert!(format!("{err1:?}").contains("unreachable"));
  assert!(get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base);
  assert!(get_i32_global(&mut store, &instance, "__persistent_ptr") < space_b_base);

  // Next scalar export recovers via __rt_turn_enter: runs pending compaction AND rewinds Scratch Arena.
  assert_eq!(scalar_probe.call(&mut store, 1.0).expect("scalar-probe"), 40_961.0);
  let pers_after = get_i32_global(&mut store, &instance, "__persistent_ptr");
  assert!(
    pers_after >= space_b_base && pers_after < arena_base,
    "expected turn_enter recovery to compact pending defref data into semi-space B, got {pers_after}"
  );
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
  let log_ptr = read_log.call(&mut store, ()).expect("read-log");
  let log_str = read_wasm_str(&mut store, &instance, log_ptr);
  assert_eq!(log_str.len(), 40_960);
  assert!(log_str.starts_with("trapped-99T"));

  // Second trap followed directly by host __arena_reset recovers cleanly as well.
  let err2 = write_and_trap
    .call(&mut store, (100.0, 2048.0))
    .expect_err("second write-and-trap should trap");
  assert!(format!("{err2:?}").contains("unreachable"));
  assert!(get_i32_global(&mut store, &instance, "__heap_ptr") > arena_base);
  arena_reset.call(&mut store, ()).expect("__arena_reset after trap");
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
  assert_eq!(scalar_probe.call(&mut store, 2.0).expect("scalar-probe 2"), 2050.0);
  assert_eq!(get_i32_global(&mut store, &instance, "__heap_ptr"), arena_base);
}

struct GatedChunks {
  first: Option<Vec<u8>>,
  rest: VecDeque<Vec<u8>>,
  entered_wait: Arc<Notify>,
  released: Arc<AtomicBool>,
  waker: Arc<Mutex<Option<Waker>>>,
}

impl StreamProducer<StoreLimits> for GatedChunks {
  type Item = u8;
  type Buffer = VecBuffer<u8>;

  fn poll_produce<'a>(
    mut self: Pin<&mut Self>,
    context: &mut Context<'_>,
    _store: StoreContextMut<'a, StoreLimits>,
    mut destination: Destination<'a, Self::Item, Self::Buffer>,
    finish: bool,
  ) -> Poll<wasmtime::Result<StreamResult>> {
    if finish {
      return Poll::Ready(Ok(StreamResult::Cancelled));
    }
    if let Some(chunk) = self.first.take() {
      destination.set_buffer(chunk.into());
      return Poll::Ready(Ok(StreamResult::Completed));
    }
    if !self.released.load(Ordering::Acquire) {
      *self.waker.lock().expect("waker mutex") = Some(context.waker().clone());
      self.entered_wait.notify_one();
      return Poll::Pending;
    }
    let chunk = self.rest.pop_front().expect("gated stream should have remaining chunk");
    destination.set_buffer(chunk.into());
    Poll::Ready(Ok(if self.rest.is_empty() {
      StreamResult::Dropped
    } else {
      StreamResult::Completed
    }))
  }
}

#[tokio::test]
async fn component_wasm_reclaims_scratch_while_stream_remains_pending() {
  let dir = TestDirectory::create("component-wasm-pending-stream-arena");
  let snapshot = r#"
{}
  :about "|Test snapshot verifying Component Scratch Arena rewinds to turn_mark across calls while a stream WAITs."
  :package |pending-stream-arena
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'pending-stream-arena.main/main!) (:mode :native) (:reload-fn 'pending-stream-arena.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'pending-stream-arena.main
    %{} 'FileEntry
      :defs $ {}
        'consume $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export consume (stream) (consume-readable-byte-stream stream 6 3 on-chunk)
          :examples $ []
          :schema $ quote $ Fn ReadableByteStream
            -> $ Result Unit StreamConsumeError
            :async
        'on-chunk $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn on-chunk (chunk)
            or
              &= chunk $ &buffer 1
              or
                &= chunk $ &buffer 2 3
                &= chunk $ &buffer 4 5 6
          :examples $ []
          :schema $ quote $ Fn Buffer (-> Bool)
        'churn-scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export churn-scalar (n)
            let
                s $ &str:pad-right (&str:concat |SC- (to-string n)) 16384 |S
              &str:count s
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'churn-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export churn-string (n)
            let
                s $ &str:pad-right (&str:concat |ST- (to-string n)) 16384 |T
              &str:slice s 0 10
          :examples $ []
          :schema $ quote $ Fn Number (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns pending-stream-arena.main
"#;
  let snapshot_path = dir.0.join("calcit.cirru");
  fs::write(&snapshot_path, snapshot).expect("pending stream snapshot should write");
  let out_dir = dir.0.join("out");
  fs::create_dir(&out_dir).expect("pending stream out dir should create");

  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["wasm", "--boundary", "component", "--emit-path"])
    .arg(&out_dir)
    .output()
    .expect("pending stream Component should compile");
  assert!(
    compile.status.success(),
    "pending stream compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );
  let contract_out = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none"])
    .arg(&snapshot_path)
    .args(["ffi", "export", "--boundary", "component"])
    .output()
    .expect("pending stream contract should export");
  assert!(
    contract_out.status.success(),
    "pending stream contract failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&contract_out.stdout),
    String::from_utf8_lossy(&contract_out.stderr)
  );
  let contract_path = out_dir.join("interface.cirru");
  fs::write(&contract_path, contract_out.stdout).expect("contract file should write");
  let contract = calcit_bindgen::load_contract(&contract_path).expect("contract should load");
  let generated = out_dir.join("generated");
  calcit_bindgen::generate_contract_directory(&contract, Some(&out_dir.join("program.wasm")), &generated, &[])
    .expect("pending stream Component should package");

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_more_async_builtins(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime engine should create");
  let component = Component::from_file(&engine, generated.join(COMPONENT_FILE)).expect("Component should load");
  let linker = ComponentLinker::new(&engine);
  let limits = StoreLimitsBuilder::new().memory_size(EIGHT_MIB).build();
  let mut store = Store::new(&engine, limits);
  store.limiter(|state| state);

  let instance = linker
    .instantiate_async(&mut store, &component)
    .await
    .expect("Component should instantiate within 8 MiB");
  let consume = instance.get_func(&mut store, "consume").expect("consume export");
  let churn_scalar = instance.get_func(&mut store, "churn-scalar").expect("churn-scalar export");
  let churn_string = instance.get_func(&mut store, "churn-string").expect("churn-string export");

  let entered_wait = Arc::new(Notify::new());
  let released = Arc::new(AtomicBool::new(false));
  let saved_waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));

  let reader = StreamReader::new(
    &mut store,
    GatedChunks {
      first: Some(vec![1]),
      rest: VecDeque::from([vec![2, 3], vec![4, 5, 6]]),
      entered_wait: Arc::clone(&entered_wait),
      released: Arc::clone(&released),
      waker: Arc::clone(&saved_waker),
    },
  )
  .expect("gated stream reader should create");
  let stream = reader
    .try_into_stream_any(&mut store)
    .expect("stream should erase to dynamic value");

  let mut consume_results = [ComponentVal::Bool(false)];
  tokio::time::timeout(
    Duration::from_secs(15),
    store.run_concurrent(async |accessor| {
      let consume_params = [ComponentVal::Stream(stream)];
      let consume_fut = consume.call_concurrent(accessor, &consume_params, &mut consume_results);
      let churn_fut = async {
        entered_wait.notified().await;
        // Let the guest stream task finish returning WAIT and releasing active_turns.
        tokio::task::yield_now().await;
        // 400 * (16 KiB scalar + 16 KiB string) ~= 12.8 MiB > 8 MiB StoreLimits; succeeds iff turn_mark rewinds while consume holds live state!
        for i in 0..400u64 {
          let mut scalar_res = [ComponentVal::Bool(false)];
          churn_scalar
            .call_concurrent(accessor, &[ComponentVal::Float64(i as f64)], &mut scalar_res)
            .await
            .unwrap_or_else(|err| panic!("churn-scalar failed at {i}: {err}"));
          assert_eq!(scalar_res, [ComponentVal::Float64(16_384.0)]);

          let mut str_res = [ComponentVal::Bool(false)];
          churn_string
            .call_concurrent(accessor, &[ComponentVal::Float64(i as f64)], &mut str_res)
            .await
            .unwrap_or_else(|err| panic!("churn-string failed at {i}: {err}"));
          assert_eq!(str_res, [ComponentVal::String(format!("ST-{i}TTTTTTT")[..10].to_owned())]);
        }
        released.store(true, Ordering::Release);
        if let Some(waker) = saved_waker.lock().expect("waker lock").take() {
          waker.wake();
        }
      };
      let (consume_outcome, ()) = tokio::join!(consume_fut, churn_fut);
      consume_outcome.expect("gated stream call should complete cleanly");
    }),
  )
  .await
  .expect("pending stream concurrency test timed out")
  .expect("store.run_concurrent should succeed");

  assert!(
    matches!(&consume_results[0], ComponentVal::Result(Ok(None))),
    "expected consume to return ok(()), got {consume_results:?}"
  );
  store.assert_concurrent_state_empty();
}

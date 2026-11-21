use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use calcit_bindgen::COMPONENT_FILE;
use wasmtime::component::{Component, Linker as ComponentLinker, Val as ComponentVal};
use wasmtime::{Config, Engine, ExternType, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val as CoreVal};

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

  // (a) Same-turn `old` vs `@*state` after `reset!`
  let memory = instance.get_memory(&mut store, "memory").expect("memory export");
  memory
    .write(&mut store, (arena_base + 16) as usize, b"turn-two")
    .expect("write input arg");
  let arg_ptr = str_new.call(&mut store, (arena_base + 16, 8)).expect("__str_new");
  let pair_ptr = same_turn.call(&mut store, arg_ptr).expect("same_turn call");
  assert_eq!(read_wasm_str(&mut store, &instance, pair_ptr), "seed-initial->turn-two");

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
  let mut old_survived_compaction = false;
  for k in 25..=35u64 {
    // Capture expected `old` prefix from the previous `bump_large` / `reset-and-return-old` step.
    let ret_old = return_old
      .call(&mut store, (200.0 + k as f64, 24_576.0))
      .expect("reset-and-return-old should succeed");
    let old_text = read_wasm_str(&mut store, &instance, ret_old);
    assert!(
      old_text.starts_with("v-") || old_text.starts_with("next-"),
      "returned `old` value corrupted: prefix `{}`",
      &old_text[..old_text.len().min(24)]
    );
    let ptr_now = get_i32_global(&mut store, &instance, "__persistent_ptr");
    if ptr_now < space_b_base {
      assert!(
        ret_old >= f64::from(space_a_base) && ret_old < f64::from(space_b_base),
        "returned `old` pointer {ret_old} must be relocated into new active semi-space A [{space_a_base}..{space_b_base})"
      );
      old_survived_compaction = true;
      break;
    }
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

  // Make a Scratch Arena string; after return, __heap_ptr == __arena_base while HEAP_MAGIC remains in linear memory.
  let stale_scratch_ptr = make_scratch.call(&mut store, 99.0).expect("make-scratch-str");
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
        'append-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export append-count (msg)
            let
                padded $ &str:pad-right (&str:concat |S- msg) 320 |K
              reset! *comp-log padded
              &str:count @*comp-log
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
  let append_count = instance.get_func(&mut store, "append-count").expect("append-count export");

  // 3,200 invocations alternating String (post-return) and Number (no post-return) exports.
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
}

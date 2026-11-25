use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory {
  path: PathBuf,
}

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-any-box-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("create temp dir");
    Self { path }
  }

  fn snapshot(&self) -> PathBuf {
    self.path.join("calcit.cirru")
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.path);
  }
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .env("CALCIT_PROOF_AUDIT", "0")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("run calcit binary")
}

fn combined_output(output: &Output) -> String {
  format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  )
}

fn base_snapshot(extra_defs: &str) -> String {
  format!(
    r#"
{{}}
  :about "|AnyBox CLI test snapshot"
  :package |any-test
  :entries $ {{}} $ :default
    {{}} (:description |) (:init-fn 'any-test.main/main!) (:mode :native) (:reload-fn 'any-test.main/reload!)
      :feature-policy $ {{}}
      :modules $ []
      :type-slots $ {{}}
  :files $ {{}} $ 'any-test.main
    %{{}} 'FileEntry
      :defs $ {{}}
        'Counter $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Counter (:n Number)
          :examples $ []
          :schema $ quote StructDef
        'CounterAlias $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ deftype CounterAlias any-test.main/Counter
          :examples $ []
        'OtherCounter $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OtherCounter (:n Number)
          :examples $ []
          :schema $ quote StructDef
        'NumListAlias $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ deftype NumListAlias $ List Number
          :examples $ []
        'TokenUnion $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defunion TokenUnion Number String
          :examples $ []
          :schema $ quote UnionDef
        'Showable $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Showable (.render $ Fn Self (-> String))
          :examples $ []
          :schema $ quote Trait
{extra_defs}
        'main! $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'reload! $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{{}} 'NsEntry (:doc |)
        :code $ quote $ ns any-test.main
"#
  )
}

#[test]
fn any_box_rejects_open_and_non_monomorphic_types_with_e_any_open_type() {
  let cases = [
    ("dynamic-param", "(defn bad (x) (any:box x))", "quote $ Fn Dynamic (-> Any)"),
    (
      "explicit-dynamic",
      "(defn bad () (any:box (assert-type 1 Dynamic)))",
      "quote $ Fn (-> Any)",
    ),
    (
      "nested-dynamic-list",
      "(defn bad () (any:box (assert-type ([] 1) (List Dynamic))))",
      "quote $ Fn (-> Any)",
    ),
    (
      "unbound-type-var",
      "(defn bad (x) (any:box x))",
      "quote $ Fn T (-> Any) (:generics T)",
    ),
    ("dyn-fn", "(defn bad () (any:box (fn (x) x)))", "quote $ Fn (-> Any)"),
    ("any-struct-family", "(defn bad (x) (any:box x))", "quote $ Fn AnyStruct (-> Any)"),
    ("anonymous-enum", "(defn bad () (any:box (:: :ok 1)))", "quote $ Fn (-> Any)"),
    (
      "trait-bounded-generic",
      "(defn bad (x) (any:box x))",
      "quote $ Fn T (-> Any) (:generics T) (:where $ T any-test.main/Showable)",
    ),
    ("empty-list-never", "(defn bad () (any:box ([])))", "quote $ Fn (-> Any)"),
    ("empty-none-never", "(defn bad () (any:box (Option :none)))", "quote $ Fn (-> Any)"),
  ];

  for (label, code, schema) in cases {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = format!(
      "        'bad $ %{{}} 'CodeEntry (:doc |)\n          :code $ quote {code}\n          :examples $ []\n          :schema $ {schema}\n"
    );
    fs::write(&snapshot, base_snapshot(&extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    let text = combined_output(&out);
    assert!(!out.status.success(), "{label} should fail:\n{text}");
    assert!(text.contains("E_ANY_OPEN_TYPE"), "{label} expected E_ANY_OPEN_TYPE:\n{text}");
  }
}

#[test]
fn any_downcast_rejects_open_targets_and_non_any_receivers() {
  let open_targets = [
    ("dynamic-target", "Dynamic"),
    ("nested-dynamic-target", "(List Dynamic)"),
    ("never-target", "Never"),
    ("dyn-fn-target", "Fn"),
    ("any-struct-target", "AnyStruct"),
    ("bare-trait-target", "any-test.main/Showable"),
    ("unresolved-slot-target", "*MissingSlot"),
  ];
  for (label, target) in open_targets {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = format!(
      "        'bad $ %{{}} 'CodeEntry (:doc |)\n          :code $ quote $ defn bad (b) (any:downcast b {target})\n          :examples $ []\n          :schema $ quote $ Fn Any (-> Dynamic)\n"
    );
    fs::write(&snapshot, base_snapshot(&extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    let text = combined_output(&out);
    assert!(!out.status.success(), "{label} should fail:\n{text}");
    assert!(text.contains("E_ANY_OPEN_TYPE"), "{label} expected E_ANY_OPEN_TYPE:\n{text}");
  }

  for (label, receiver_expr) in [
    ("number-receiver", "42"),
    ("dynamic-receiver", "(assert-type 42 Dynamic)"),
    ("option-any-receiver", "(assert-type (Option :some (any:box 42)) (Option Any))"),
  ] {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = format!(
      "        'bad $ %{{}} 'CodeEntry (:doc |)\n          :code $ quote $ defn bad () (any:downcast {receiver_expr} Number)\n          :examples $ []\n          :schema $ quote $ Fn (-> (Option Number))\n"
    );
    fs::write(&snapshot, base_snapshot(&extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    let text = combined_output(&out);
    assert!(!out.status.success(), "{label} should fail:\n{text}");
    assert!(
      text.contains("E_ANY_DOWNCAST_RECEIVER"),
      "{label} expected E_ANY_DOWNCAST_RECEIVER:\n{text}"
    );
  }
}

#[test]
fn any_prevents_implicit_widening_and_rejects_type_args_or_static_eq() {
  // 1. Concrete value does not widen implicitly to Any parameter
  {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = r#"
        'take-any $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn take-any (x) 0
          :examples $ []
          :schema $ quote $ Fn Any (-> Number)
        'call-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn call-bad () (take-any 42)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
"#;
    fs::write(&snapshot, base_snapshot(extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    assert!(
      !out.status.success(),
      "concrete Number must not widen to Any:\n{}",
      combined_output(&out)
    );
  }

  // 2. Any does not implement static Eq / Compare, while Debug passes
  {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = r#"
        'need-eq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn need-eq (x) x
          :examples $ []
          :schema $ quote $ Fn T (-> T) (:generics T) (:where $ T Eq)
        'bad-eq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-eq (b) (need-eq b)
          :examples $ []
          :schema $ quote $ Fn Any (-> Any)
"#;
    fs::write(&snapshot, base_snapshot(extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    assert!(!out.status.success(), "Any must not satisfy static Eq:\n{}", combined_output(&out));
  }
  {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = r#"
        'need-compare $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn need-compare (x) x
          :examples $ []
          :schema $ quote $ Fn T (-> T) (:generics T) (:where $ T Compare)
        'bad-compare $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-compare (b) (need-compare b)
          :examples $ []
          :schema $ quote $ Fn Any (-> Any)
"#;
    fs::write(&snapshot, base_snapshot(extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    assert!(
      !out.status.success(),
      "Any must not satisfy static Compare:\n{}",
      combined_output(&out)
    );
  }
  {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let mut text = base_snapshot("");
    text = text.replace("defn main! () 0", "defn main! () (do (assert-traits (any:box 42) Eq) 0)");
    fs::write(&snapshot, &text).expect("write snapshot");
    let check = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    assert!(
      check.status.success(),
      "runtime assert-traits check-only should succeed:\n{}",
      combined_output(&check)
    );
    let out = run_calcit(&snapshot, &[]);
    let out_text = combined_output(&out);
    assert!(
      !out.status.success() && out_text.contains("does not nominally implement"),
      "runtime assert-traits Eq on Any must fail with trait message:\n{out_text}"
    );
  }
  {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = r#"
        'LoopAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype LoopAlias (:generics T) (LoopAlias T)
          :examples $ []
        'bad-cycle $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-cycle (b) (any:downcast b (LoopAlias Number))
          :examples $ []
          :schema $ quote $ Fn Any (-> Dynamic)
"#;
    fs::write(&snapshot, base_snapshot(extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    let text = combined_output(&out);
    assert!(!out.status.success(), "cyclic deftype alias must fail:\n{text}");
    assert!(text.contains("E_ANY_OPEN_TYPE"), "{text}");
  }

  // 3. Applied (Any Number) and wrong generic deftype / struct arity are rejected
  {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = r#"
        'bad-arity $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-arity (b) (any:downcast b (Any Number))
          :examples $ []
          :schema $ quote $ Fn Any (-> Dynamic)
"#;
    fs::write(&snapshot, base_snapshot(extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    let text = combined_output(&out);
    assert!(!out.status.success(), "(Any Number) should fail:\n{text}");
    assert!(text.contains("E_TYPE_EXPR_SYNTAX"), "{text}");
  }
  for (label, target_expr, expected_code) in [
    ("alias-too-many-args", "(MyOpt Number String)", "E_TYPE_ALIAS_ARITY"),
    ("alias-missing-arg-bare", "MyOpt", "E_TYPE_ALIAS_ARITY"),
    ("alias-empty-app", "(MyOpt)", "E_TYPE_EXPR_SYNTAX"),
    ("non-generic-struct-with-arg", "(Counter Number)", "E_TYPE_EXPR_SYNTAX"),
  ] {
    let dir = TestDirectory::create();
    let snapshot = dir.snapshot();
    let extra = format!(
      r#"
        'MyOpt $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ deftype MyOpt (:generics T) (Option T)
          :examples $ []
        'bad-target-arity $ %{{}} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-target-arity (b) (any:downcast b {target_expr})
          :examples $ []
          :schema $ quote $ Fn Any (-> Dynamic)
"#
    );
    fs::write(&snapshot, base_snapshot(&extra)).expect("write snapshot");
    let out = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
    let text = combined_output(&out);
    assert!(!out.status.success(), "{label} should fail:\n{text}");
    assert!(text.contains(expected_code), "{label} expected {expected_code}:\n{text}");
  }
}

#[test]
fn any_box_handles_structs_aliases_and_unions_on_native_and_js() {
  let dir = TestDirectory::create();
  let snapshot = dir.snapshot();
  let extra = r#"
        'Id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Id (:generics T) T
          :examples $ []
        'run-checks! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn run-checks! ()
            let
                c $ Counter :n 7
                b-c $ any:box c
                _ $ assert-traits b-c Debug
                xs $ assert-type ([] 1 2) NumListAlias
                b-xs $ any:box xs
                u $ assert-type |leaf TokenUnion
                b-u $ any:box u
                b-id $ any:box 42
                buf1 $ any:box (&buffer 1 2)
                buf2 $ any:box (&buffer 1 3)
                buf1-eq $ any:box (&buffer 1 2)
                buf-set $ #{} buf1 buf2 buf1-eq
              match (any:downcast b-c CounterAlias)
                (:some got) $ do
                  assert= c got
                  assert= 7 (:n got)
                (:none) (raise |expected-counter-hit)
              match (any:downcast b-c OtherCounter)
                (:some _) (raise |expected-other-counter-miss)
                (:none) &unit
              match (any:downcast b-xs (List Number))
                (:some got) (assert= ([] 1 2) got)
                (:none) (raise |expected-alias-list-hit)
              match (any:downcast b-id (Id (Id Number)))
                (:some got) (assert= 42 got)
                (:none) (raise |expected-nested-id-hit)
              match (any:downcast b-id Number)
                (:some got) (assert= 42 got)
                (:none) (raise |expected-nested-id-as-number-hit)
              assert= 2 (count buf-set)
              assert= true (&= buf1 buf1-eq)
              assert= false (&= buf1 buf2)
              assert= true (not= (&compare buf1 buf2) 0)
              match (any:downcast b-u TokenUnion)
                (:some got)
                  match got
                    (String s) (assert= |leaf s)
                    (Number _) (raise |wrong-union-branch)
                (:none) (raise |expected-union-hit)
              match (any:downcast b-u String)
                (:some _) (raise |expected-union-member-miss)
                (:none) &unit
              println |ANY-OK
              , 0
          :examples $ []
          :schema $ quote $ Fn (-> Number) (:features :io)
"#;
  let mut text = base_snapshot(extra);
  text = text.replace(
    "'main! $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ defn main! () 0\n          :examples $ []\n          :schema $ quote $ Fn (-> Number)",
    "'main! $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ defn main! () (run-checks!)\n          :examples $ []\n          :schema $ quote $ Fn (-> Number) (:features :io)",
  );
  fs::write(&snapshot, &text).expect("write snapshot");

  let check = run_calcit(&snapshot, &["--check-only", "--all-defs"]);
  assert!(check.status.success(), "check-only failed:\n{}", combined_output(&check));

  let native = run_calcit(&snapshot, &[]);
  let native_out = combined_output(&native);
  assert!(native.status.success(), "native failed:\n{native_out}");
  assert!(native_out.contains("ANY-OK"), "{native_out}");

  let js_emit_dir = dir.path.join("js-out");
  let js = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .env("CALCIT_PROOF_AUDIT", "0")
    .args(["--tips-level", "none", "--emit-path"])
    .arg(&js_emit_dir)
    .arg(&snapshot)
    .arg("js")
    .output()
    .expect("emit js");
  let js_out = combined_output(&js);
  assert!(js.status.success(), "js codegen failed:\n{js_out}");

  let manifest_node_modules = Path::new(env!("CARGO_MANIFEST_DIR")).join("node_modules");
  #[cfg(unix)]
  std::os::unix::fs::symlink(&manifest_node_modules, dir.path.join("node_modules")).expect("symlink node_modules");
  let node_run = Command::new("node")
    .current_dir(&dir.path)
    .args([
      "--input-type=module",
      "-e",
      "import * as app from './js-out/any-test.main.mjs'; app.main_$x_();",
    ])
    .output()
    .expect("run emitted js");
  let node_out = combined_output(&node_run);
  assert!(node_run.status.success(), "node execution failed:\n{node_out}");
  assert!(node_out.contains("ANY-OK"), "{node_out}");
}

#[cfg(feature = "wasm")]
#[test]
fn any_box_handles_structs_aliases_and_unions_on_wasm() {
  use std::collections::HashSet;
  use wasmtime::{Engine, ExternType, Linker, Module, Store};

  let dir = TestDirectory::create();
  let snapshot = dir.snapshot();
  let extra = r#"
        'NumAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype NumAlias Number
          :examples $ []
        'TagAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype TagAlias Tag
          :examples $ []
        'TokenUnionAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype TokenUnionAlias any-test.main/TokenUnion
          :examples $ []
        'ScalarUnion $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion ScalarUnion Number Tag
          :examples $ []
          :schema $ quote UnionDef
        'Id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Id (:generics T) T
          :examples $ []
        'check-counter! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-counter! ()
            let
                c $ Counter :n 7
                b-c $ any:box c
              match (any:downcast b-c CounterAlias)
                (:some got) (if (and (&= c got) (&= 7 (:n got))) 1 0)
                (:none) 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'check-other! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-other! ()
            let
                c $ Counter :n 7
                b-c $ any:box c
              match (any:downcast b-c OtherCounter)
                (:some _) 0
                (:none) 1
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'check-list! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-list! ()
            let
                xs $ assert-type ([] 1 2) NumListAlias
                b-xs $ any:box xs
              match (any:downcast b-xs (List Number))
                (:some got) (if (&= ([] 1 2) got) 1 0)
                (:none) 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'check-union! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-union! ()
            let
                u $ assert-type |leaf TokenUnion
                b-u $ any:box u
                union-hit $ match (any:downcast b-u TokenUnionAlias)
                  (:some got)
                    match got
                      (String s) (&= |leaf s)
                      (Number _) false
                  (:none) false
                union-miss $ match (any:downcast b-u String)
                  (:some _) false
                  (:none) true
              if (and union-hit union-miss) 1 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'check-num-tag! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defwasm-export check-num-tag! ()
            let
                b-num $ any:box (assert-type 262152 NumAlias)
                b-tag $ any:box (assert-type :ok TagAlias)
                num-ok $ match (any:downcast b-num Number)
                  (:some n) (&= n 262152)
                  (:none) false
                tag-ok $ match (any:downcast b-tag Tag)
                  (:some t) (&= t :ok)
                  (:none) false
                json-bools $ assert-type (json-parse "|[false,true]") (List Bool)
                json-bool-ok $ match (first json-bools)
                  (:some jb)
                    let
                        b-json-false $ any:box jb
                        b-lit-false $ any:box false
                      and (&= b-json-false b-lit-false) (&= (&hash b-json-false) (&hash b-lit-false))
                  (:none) false
                scalar-u $ assert-type 262152 ScalarUnion
                b-su $ any:box scalar-u
                scalar-u-ok $ and
                  &= b-su (any:box (assert-type 262152 ScalarUnion))
                  not (&= b-su (any:box 262152))
                  match (any:downcast b-su ScalarUnion)
                    (:some got)
                      match got
                        (Number _) true
                        (Tag _) false
                    (:none) false
                nested-id-ok $ match (any:downcast (any:box 42) (Id (Id Number)))
                  (:some n) (&= n 42)
                  (:none) false
              if (and num-ok tag-ok json-bool-ok scalar-u-ok nested-id-ok) 1 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
"#;
  fs::write(&snapshot, base_snapshot(extra)).expect("write snapshot");

  let wasm_out = dir.path.join("wasm-out");
  fs::create_dir(&wasm_out).expect("create wasm out dir");
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .env("CALCIT_PROOF_AUDIT", "0")
    .args(["--tips-level", "none", "--emit-path"])
    .arg(&wasm_out)
    .arg(&snapshot)
    .arg("wasm")
    .output()
    .expect("compile wasm");
  assert!(compile.status.success(), "wasm compile failed:\n{}", combined_output(&compile));

  let engine = Engine::default();
  let module = Module::from_file(&engine, wasm_out.join("program.wasm")).expect("load wasm");
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
          for (idx, ty) in result_tys.iter().enumerate() {
            results[idx] = match ty {
              wasmtime::ValType::I32 => wasmtime::Val::I32(0),
              wasmtime::ValType::I64 => wasmtime::Val::I64(0),
              wasmtime::ValType::F32 => wasmtime::Val::F32(0),
              wasmtime::ValType::F64 => wasmtime::Val::F64(0),
              _ => wasmtime::Val::I32(0),
            };
          }
          Ok(())
        })
        .expect("stub import");
    }
  }
  let mut store = Store::new(&engine, ());
  let instance = linker.instantiate(&mut store, &module).expect("instantiate wasm");
  for name in ["check-counter!", "check-other!", "check-list!", "check-union!", "check-num-tag!"] {
    let func = instance
      .get_typed_func::<(), f64>(&mut store, name)
      .unwrap_or_else(|e| panic!("missing export `{name}`: {e}"));
    assert_eq!(
      func.call(&mut store, ()).unwrap_or_else(|e| panic!("call `{name}`: {e}")),
      1.0,
      "{name}"
    );
  }
}

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
    text = text.replace("defn main! () 0", "defn main! () (assert-traits (any:box 42) Eq)");
    fs::write(&snapshot, &text).expect("write snapshot");
    let out = run_calcit(&snapshot, &[]);
    assert!(
      !out.status.success(),
      "runtime assert-traits Eq on Any must fail:\n{}",
      combined_output(&out)
    );
  }

  // 3. Applied (Any Number) is rejected by type expression arity check
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
}

#[test]
fn any_box_handles_structs_aliases_and_unions_on_native_and_js() {
  let dir = TestDirectory::create();
  let snapshot = dir.snapshot();
  let extra = r#"
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
              match (any:downcast b-c CounterAlias)
                (:some got) (assert= c got)
                (:none) (raise |expected-counter-hit)
              match (any:downcast b-c OtherCounter)
                (:some _) (raise |expected-other-counter-miss)
                (:none) &unit
              match (any:downcast b-xs (List Number))
                (:some got) (assert= ([] 1 2) got)
                (:none) (raise |expected-alias-list-hit)
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

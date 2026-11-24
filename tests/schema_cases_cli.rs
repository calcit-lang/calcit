use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn calcit_bin() -> &'static str {
  env!("CARGO_BIN_EXE_calcit")
}

fn temp_dir(name: &str) -> PathBuf {
  static COUNTER: AtomicU64 = AtomicU64::new(0);
  let id = COUNTER.fetch_add(1, Ordering::Relaxed);
  let dir = std::env::temp_dir().join(format!("calcit-schema-cases-{name}-{}-{id}", std::process::id()));
  let _ = fs::remove_dir_all(&dir);
  fs::create_dir_all(&dir).expect("create temp dir");
  dir
}

fn write_snapshot(dir: &Path, content: &str) -> PathBuf {
  let path = dir.join("calcit.cirru");
  fs::write(&path, content).expect("write calcit.cirru");
  path
}

fn run_calcit(dir: &Path, args: &[&str]) -> std::process::Output {
  Command::new(calcit_bin())
    .current_dir(dir)
    .args(args)
    .output()
    .expect("spawn calcit")
}

fn format_output(out: &std::process::Output) -> String {
  format!(
    "status={:?}\nstdout:\n{}\nstderr:\n{}",
    out.status.code(),
    String::from_utf8_lossy(&out.stdout),
    String::from_utf8_lossy(&out.stderr)
  )
}

#[test]
fn user_cases_schema_dispatches_checks_infers_and_runs_across_backends() {
  let dir = temp_dir("user-cases-e2e");
  let snapshot = r#"
{} (:about "|schema :cases end-to-end test") (:package |demo)
  :entries $ {}
    :default $ {} (:init-fn |demo.main/main!) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
    :wasm $ {} (:init-fn |demo.main/wasm-main) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
  :files $ {}
    |demo.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn main! ()
              let
                  n-opt $ pick-head ([] 10 20)
                  s-opt $ pick-head |hello
                  mapped $ map-or-id ([] 1 2 3) (fn (x) (&+ x 10))
                  kept $ map-or-id |abc (fn (s) (&str:concat s |!))
                  scores $ {} (:a 1) (:b 2)
                  has-big $ any? scores (fn (pair) (> (&+ (nth pair 1) 0) 1))
                  all-pos $ every? scores (fn (pair) (> (nth pair 1) 0))
                  trimmed $ dissoc scores :b
                  shorter $ dissoc ([] |x |y) 1
                  has-two $ includes? scores 2
                  pm $ pairs-map ([] (assert-type ([] :k 9) (Tuple Tag Number)))
                assert= 15 $ match n-opt
                  (:some n) (&+ n 5)
                  (:none) 0
                assert= |h! $ match s-opt
                  (:some s) (&str:concat s |!)
                  (:none) |
                assert= ([] 11 12 13) mapped
                assert= |abc! kept
                assert= true has-big
                assert= true all-pos
                assert= ({} (:a 1)) trimmed
                assert= ([] |x) shorter
                assert= true has-two
                assert= (Option :some 9) (get pm :k)
                , &unit
          :schema $ quote $ Fn (-> Unit)
        |reload! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn reload! () &unit
          :schema $ quote $ Fn (-> Unit)
        |pick-head $ %{} :CodeEntry (:doc "|Returns first element for List or first char for String") (:examples $ [])
          :code $ quote
            defn pick-head (x)
              cond
                (list? x) (first x)
                (string? x) (first x)
                true (Option :none)
          :schema $ quote $ Fn Dynamic (-> (Option Dynamic)) (:open :reflect)
            :cases
              Fn (List T) (-> (Option T)) (:generics T)
              Fn String (-> (Option String))
        |map-or-id $ %{} :CodeEntry (:doc "|Applies callback to List items or directly to String") (:examples $ [])
          :code $ quote
            defn map-or-id (x f)
              cond
                (list? x) (map x f)
                (string? x) (f x)
                true x
          :schema $ quote $ Fn Dynamic (Fn Dynamic (-> Dynamic) (:features :pass)) (-> Dynamic) (:open :reflect)
            :cases
              Fn (List T) (Fn T (-> U) (:features :pass)) (-> (List U)) (:generics T U)
              Fn String (Fn String (-> U) (:features :pass)) (-> U) (:generics U)
        |lookup-entry $ %{} :CodeEntry (:doc "|Typed lookup with Map and List cases") (:examples $ [])
          :code $ quote
            defn lookup-entry (c k)
              get c k
          :schema $ quote $ Fn Dynamic Dynamic (-> (Option Dynamic)) (:open :reflect)
            :cases
              Fn (Map K V) K (-> (Option V)) (:generics K V)
              Fn (List T) Number (-> (Option T)) (:generics T)
        |wasm-main $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn wasm-main ()
              let
                  n-opt $ pick-head ([] 40 2)
                match n-opt
                  (:some n) (&+ n 2)
                  (:none) 0
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} :CodeEntry (:doc |) (:examples $ [])
        :code $ quote (ns demo.main)
        :schema $ quote Dynamic
"#;
  write_snapshot(&dir, snapshot);

  // 1. Strict --check-only passes (user :cases + migrated core defs + Map->Tuple parity)
  let check = run_calcit(&dir, &["--check-only", "calcit.cirru"]);
  assert!(check.status.success(), "--check-only should pass:\n{}", format_output(&check));

  // 2. Native execution passes
  let native = run_calcit(&dir, &["calcit.cirru"]);
  assert!(native.status.success(), "native execution should pass:\n{}", format_output(&native));

  // 3. query schema (human + JSON + EDN) and query peek render :cases for both user & core defs
  let user_schema_human = run_calcit(&dir, &["calcit.cirru", "query", "schema", "demo.main/pick-head"]);
  assert!(user_schema_human.status.success(), "{}", format_output(&user_schema_human));
  let user_schema_stdout = String::from_utf8_lossy(&user_schema_human.stdout);
  assert!(
    user_schema_stdout.contains("## Cases") && user_schema_stdout.contains("Fn (List T)"),
    "query schema should render ## Cases:\n{user_schema_stdout}"
  );

  let user_schema_json = run_calcit(
    &dir,
    &["calcit.cirru", "query", "schema", "demo.main/pick-head", "--format", "json"],
  );
  assert!(user_schema_json.status.success(), "{}", format_output(&user_schema_json));
  let parsed_json: serde_json::Value = serde_json::from_slice(&user_schema_json.stdout).expect("parse query schema json");
  let cases_arr = parsed_json["data"]["cases"].as_array().expect("data.cases array");
  assert_eq!(cases_arr.len(), 2, "expected 2 cases in JSON output: {parsed_json}");

  let user_peek = run_calcit(&dir, &["calcit.cirru", "query", "peek", "demo.main/pick-head"]);
  assert!(user_peek.status.success(), "{}", format_output(&user_peek));
  let user_peek_stdout = String::from_utf8_lossy(&user_peek.stdout);
  assert!(
    user_peek_stdout.contains("## Cases") && user_peek_stdout.contains("Fn String"),
    "query peek should render ## Cases:\n{user_peek_stdout}"
  );

  let core_schema = run_calcit(&dir, &["calcit.cirru", "query", "schema", "calcit.core/filter"]);
  assert!(core_schema.status.success(), "{}", format_output(&core_schema));
  let core_schema_stdout = String::from_utf8_lossy(&core_schema.stdout);
  assert!(
    core_schema_stdout.contains("## Cases") && !core_schema_stdout.contains("## Checker rules"),
    "migrated core filter should render ## Cases without duplicated ## Checker rules:\n{core_schema_stdout}"
  );

  // 4. WASM check-only succeeds: `:cases` narrows `pick-head` return to `Option Number` before WASM lowering
  let wasm_check = run_calcit(&dir, &["--entry", "wasm", "calcit.cirru", "wasm", "--check-only"]);
  assert!(
    wasm_check.status.success(),
    "wasm --check-only should pass for :cases:\n{}",
    format_output(&wasm_check)
  );

  // 5. JS codegen succeeds
  let js_emit = run_calcit(&dir, &["calcit.cirru", "js"]);
  assert!(js_emit.status.success(), "js codegen should pass:\n{}", format_output(&js_emit));

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_cases_schema_rejects_mismatched_case_args_and_falls_back_on_dynamic() {
  let dir = temp_dir("user-cases-mismatch");
  let snapshot = r#"
{} (:about "|schema :cases mismatch test") (:package |demo)
  :entries $ {}
    :default $ {} (:init-fn |demo.main/main!) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
  :files $ {}
    |demo.main $ %{} :FileEntry
      :defs $ {}
        |lookup-entry $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn lookup-entry (c k)
              get c k
          :schema $ quote $ Fn Dynamic Dynamic (-> (Option Dynamic)) (:open :reflect)
            :cases
              Fn (Map K V) K (-> (Option V)) (:generics K V)
              Fn (List T) Number (-> (Option T)) (:generics T)
        |dynamic-fallback $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn dynamic-fallback (x k)
              lookup-entry x k
          :schema $ quote $ Fn Dynamic Dynamic (-> (Option Dynamic)) (:open :reflect)
        |main! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn main! ()
              let
                  bad-map-key $ lookup-entry ({} (:a 1)) |wrong-key
                  bad-list-idx $ lookup-entry ([] 1 2) :not-a-number
                  bad-each $ each ({} (:a 1)) (fn (n) (&+ n 1))
                , &unit
          :schema $ quote $ Fn (-> Unit)
        |reload! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn reload! () &unit
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} :CodeEntry (:doc |) (:examples $ [])
        :code $ quote (ns demo.main)
        :schema $ quote Dynamic
"#;
  write_snapshot(&dir, snapshot);

  let check = run_calcit(&dir, &["--check-only", "calcit.cirru"]);
  assert!(
    !check.status.success(),
    "mismatched :cases arguments must fail --check-only:\n{}",
    format_output(&check)
  );
  let combined = format!(
    "{}\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
  assert!(
    combined.contains("demo.main/lookup-entry") && combined.contains("E_FN_ARG_TYPE_MISMATCH"),
    "expected E_FN_ARG_TYPE_MISMATCH on demo.main/lookup-entry:\n{combined}"
  );
  assert!(
    combined.contains("expects type `:tag`, but got `:string`"),
    "expected Map key mismatch on lookup-entry:\n{combined}"
  );
  assert!(
    combined.contains("expects type `:number`, but got `:tag`"),
    "expected List index mismatch on lookup-entry:\n{combined}"
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_cases_body_verification_accepts_predicate_narrowing_across_backends() {
  let dir = temp_dir("user-cases-body-valid");
  let snapshot = r#"
{} (:about "|schema :cases body verification positive test") (:package |demo)
  :entries $ {}
    :default $ {} (:init-fn |demo.main/main!) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
    :wasm $ {} (:init-fn |demo.main/wasm-main) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
  :files $ {}
    |demo.main $ %{} :FileEntry
      :defs $ {}
        |measure-value $ %{} :CodeEntry (:doc "|Measures List/String/Map/Number/Nil/Tag via predicates and type-of") (:examples $ [])
          :code $ quote
            defn measure-value (x)
              cond
                (list? x) (&list:count x)
                (string? x) (&str:count x)
                (map? x) (&map:count x)
                (number? x) (&+ x 1)
                (nil? x) 0
                (&= (type-of x) :tag) 7
                true 99
          :schema $ quote $ Fn Dynamic (-> Number) (:open :reflect)
            :cases
              Fn (List T) (-> Number) (:generics T)
              Fn String (-> Number)
              Fn (Map K V) (-> Number) (:generics K V)
              Fn Number (-> Number)
              Fn Nil (-> Number)
              Fn Tag (-> Number)
        |unwrap-or-negate $ %{} :CodeEntry (:doc "|Uses if + not predicate narrowing") (:examples $ [])
          :code $ quote
            defn unwrap-or-negate (x)
              if (not (string? x))
                if (number? x) (&+ x 10) 0
                &str:count x
          :schema $ quote $ Fn Dynamic (-> Number) (:open :reflect)
            :cases
              Fn Number (-> Number)
              Fn String (-> Number)
        |main! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn main! ()
              assert= 3 $ measure-value ([] 1 2 3)
              assert= 5 $ measure-value |hello
              assert= 2 $ measure-value ({} (:a 1) (:b 2))
              assert= 42 $ measure-value 41
              assert= 0 $ measure-value nil
              assert= 7 $ measure-value :ok
              assert= 15 $ unwrap-or-negate 5
              assert= 4 $ unwrap-or-negate |test
              , &unit
          :schema $ quote $ Fn (-> Unit)
        |wasm-main $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn wasm-main ()
              &+ (measure-value |hello) (unwrap-or-negate 5)
          :schema $ quote $ Fn (-> Number)
        |reload! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn reload! () &unit
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} :CodeEntry (:doc |) (:examples $ [])
        :code $ quote (ns demo.main)
        :schema $ quote Dynamic
"#;
  write_snapshot(&dir, snapshot);

  let check = run_calcit(&dir, &["--check-only", "calcit.cirru"]);
  assert!(check.status.success(), "--check-only should pass:\n{}", format_output(&check));

  let native = run_calcit(&dir, &["calcit.cirru"]);
  assert!(native.status.success(), "native execution should pass:\n{}", format_output(&native));

  let js_emit = run_calcit(&dir, &["calcit.cirru", "js"]);
  assert!(js_emit.status.success(), "js codegen should pass:\n{}", format_output(&js_emit));

  let wasm_check = run_calcit(&dir, &["--entry", "wasm", "calcit.cirru", "wasm", "--check-only"]);
  assert!(
    wasm_check.status.success(),
    "wasm --check-only should pass:\n{}",
    format_output(&wasm_check)
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_cases_body_verification_rejects_lying_return_type() {
  let dir = temp_dir("user-cases-lying-return");
  let snapshot = r#"
{} (:about "|schema :cases lying return type test") (:package |demo)
  :entries $ {}
    :default $ {} (:init-fn |demo.main/main!) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
  :files $ {}
    |demo.main $ %{} :FileEntry
      :defs $ {}
        |lying-len $ %{} :CodeEntry (:doc "|Claims Number for String case but returns String") (:examples $ [])
          :code $ quote
            defn lying-len (x)
              cond
                (list? x) (&list:count x)
                (string? x) x
                true 0
          :schema $ quote $ Fn Dynamic (-> Dynamic) (:open :reflect)
            :cases
              Fn (List T) (-> Number) (:generics T)
              Fn String (-> Number)
        |main! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn main! ()
              let
                  n $ lying-len |abc
                , &unit
          :schema $ quote $ Fn (-> Unit)
        |reload! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn reload! () &unit
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} :CodeEntry (:doc |) (:examples $ [])
        :code $ quote (ns demo.main)
        :schema $ quote Dynamic
"#;
  write_snapshot(&dir, snapshot);

  let check = run_calcit(&dir, &["--check-only", "calcit.cirru"]);
  assert!(
    !check.status.success(),
    "lying :cases return type must fail --check-only:\n{}",
    format_output(&check)
  );
  let combined = format!(
    "{}\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
  assert!(
    combined.contains("E_FN_RETURN_TYPE_MISMATCH") && combined.contains("demo.main/lying-len") && combined.contains(":cases"),
    "expected E_FN_RETURN_TYPE_MISMATCH on demo.main/lying-len with :cases context:\n{combined}"
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_cases_body_verification_rejects_wrong_branch_argument_usage() {
  let dir = temp_dir("user-cases-bad-branch-arg");
  let snapshot = r#"
{} (:about "|schema :cases wrong branch argument usage test") (:package |demo)
  :entries $ {}
    :default $ {} (:init-fn |demo.main/main!) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
  :files $ {}
    |demo.main $ %{} :FileEntry
      :defs $ {}
        |bad-branch-call $ %{} :CodeEntry (:doc "|Passes String to callback expecting T on List T branch") (:examples $ [])
          :code $ quote
            defn bad-branch-call (x f)
              cond
                (list? x) (f |not-t)
                (string? x) (f x)
                true (f x)
          :schema $ quote $ Fn Dynamic (Fn Dynamic (-> Dynamic) (:features :pass)) (-> Dynamic) (:open :reflect)
            :cases
              Fn (List T) (Fn T (-> U) (:features :pass)) (-> U) (:generics T U)
              Fn String (Fn String (-> U) (:features :pass)) (-> U) (:generics U)
        |main! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn main! ()
              let
                  n $ bad-branch-call ([] 1 2) (fn (v) (&+ v 1))
                , &unit
          :schema $ quote $ Fn (-> Unit)
        |reload! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn reload! () &unit
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} :CodeEntry (:doc |) (:examples $ [])
        :code $ quote (ns demo.main)
        :schema $ quote Dynamic
"#;
  write_snapshot(&dir, snapshot);

  let check = run_calcit(&dir, &["--check-only", "calcit.cirru"]);
  assert!(
    !check.status.success(),
    "wrong argument usage inside a :cases branch must fail --check-only:\n{}",
    format_output(&check)
  );
  let combined = format!(
    "{}\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
  assert!(
    combined.contains("E_CALL_ARGUMENT_UNPROVEN") && combined.contains("demo.main/bad-branch-call") && combined.contains(":cases"),
    "expected E_CALL_ARGUMENT_UNPROVEN on demo.main/bad-branch-call with :cases context:\n{combined}"
  );

  let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_cases_body_verification_rejects_lying_generic_return() {
  let dir = temp_dir("user-cases-lying-generic");
  let snapshot = r#"
{} (:about "|schema :cases lying generic return test") (:package |demo)
  :entries $ {}
    :default $ {} (:init-fn |demo.main/main!) (:reload-fn |demo.main/reload!) (:version |0.0.1) (:modules $ []) (:mode :native)
  :files $ {}
    |demo.main $ %{} :FileEntry
      :defs $ {}
        |lying-generic $ %{} :CodeEntry (:doc "|Claims -> T for List T case but returns Number") (:examples $ [])
          :code $ quote
            defn lying-generic (x)
              cond
                (list? x) 42
                (string? x) x
                true x
          :schema $ quote $ Fn Dynamic (-> Dynamic) (:open :reflect)
            :cases
              Fn (List T) (-> T) (:generics T)
              Fn String (-> String)
        |main! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn main! ()
              let
                  v $ lying-generic ([] |a |b)
                , &unit
          :schema $ quote $ Fn (-> Unit)
        |reload! $ %{} :CodeEntry (:doc |) (:examples $ [])
          :code $ quote
            defn reload! () &unit
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} :CodeEntry (:doc |) (:examples $ [])
        :code $ quote (ns demo.main)
        :schema $ quote Dynamic
"#;
  write_snapshot(&dir, snapshot);

  let check = run_calcit(&dir, &["--check-only", "calcit.cirru"]);
  assert!(
    !check.status.success(),
    "lying generic :cases return must fail --check-only:\n{}",
    format_output(&check)
  );
  let combined = format!(
    "{}\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
  assert!(
    (combined.contains("E_FN_RETURN_TYPE_MISMATCH")
      || combined.contains("E_ERASED_GENERIC_RELATION")
      || combined.contains("E_FN_RETURN_UNPROVEN"))
      && combined.contains(":cases"),
    "expected generic return failure with :cases context:\n{combined}"
  );

  let _ = fs::remove_dir_all(&dir);
}

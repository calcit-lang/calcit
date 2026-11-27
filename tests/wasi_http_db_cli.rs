use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Instant;

use calcit_bindgen::wasmtime_db::{DbConfig, HttpDbServiceRunner, InMemoryKvBackend, SqlBackend, SqliteSqlBackend, WasmtimeDbState};
use http::Method;
use serde_json::Value;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

const MEMORY_LIMIT_BYTES: usize = 8 * 1024 * 1024;

fn calcit(args: &[&str], emit_dir: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .arg("--emit-path")
    .arg(emit_dir)
    .output()
    .expect("execute calcit CLI")
}

fn calcit_raw(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .output()
    .expect("execute calcit CLI")
}

fn edit_def(snapshot: &Path, target: &str, code: &str, schema: &str) {
  let snapshot_str = snapshot.to_str().expect("UTF-8 snapshot path");
  let def_res = calcit_raw(&[
    snapshot_str,
    "edit",
    "def",
    target,
    "--overwrite",
    "--input-format",
    "cirru",
    "--code",
    code,
  ]);
  assert!(
    def_res.status.success(),
    "edit def {target}: {}",
    String::from_utf8_lossy(&def_res.stderr)
  );
  let schema_res = calcit_raw(&[snapshot_str, "edit", "schema", target, "--input-format", "cirru", "--code", schema]);
  assert!(
    schema_res.status.success(),
    "edit schema {target}: {}",
    String::from_utf8_lossy(&schema_res.stderr)
  );
}

fn rm_def(snapshot: &Path, target: &str) {
  let snapshot_str = snapshot.to_str().expect("UTF-8 snapshot path");
  let res = calcit_raw(&[snapshot_str, "edit", "rm-def", target]);
  assert!(res.status.success(), "rm-def {target}: {}", String::from_utf8_lossy(&res.stderr));
}

#[test]
fn http_db_fixture_tests_and_deterministic_component_packaging() {
  let native_tests = calcit_raw(&["tests/fixtures/wasi-http-db.cirru", "test", "--require-match"]);
  assert!(
    native_tests.status.success(),
    "wasi-http-db native unit tests failed:\nstdout: {}\nstderr: {}",
    String::from_utf8_lossy(&native_tests.stdout),
    String::from_utf8_lossy(&native_tests.stderr)
  );

  let check_dir = tempfile::tempdir().expect("check-only directory");
  let checked = calcit(
    &["tests/fixtures/wasi-http-db.cirru", "wasi", "--world", "http", "--check-only"],
    check_dir.path(),
  );
  assert!(checked.status.success(), "{}", String::from_utf8_lossy(&checked.stderr));
  assert!(!check_dir.path().join("program.wasm").exists());

  let emit_a = tempfile::tempdir().expect("emit dir A");
  let emit_b = tempfile::tempdir().expect("emit dir B");
  let out_a = calcit(&["tests/fixtures/wasi-http-db.cirru", "wasi", "--world", "http"], emit_a.path());
  assert!(out_a.status.success(), "{}", String::from_utf8_lossy(&out_a.stderr));
  let out_b = calcit(&["tests/fixtures/wasi-http-db.cirru", "wasi", "--world", "http"], emit_b.path());
  assert!(out_b.status.success(), "{}", String::from_utf8_lossy(&out_b.stderr));

  let wasm_a = fs::read(emit_a.path().join("program.wasm")).expect("read first HTTP+DB Component");
  let wasm_b = fs::read(emit_b.path().join("program.wasm")).expect("read second HTTP+DB Component");
  assert_eq!(wasm_a, wasm_b, "WASI HTTP+DB Component output must be byte-deterministic");

  let engine = Engine::new(&Config::new()).expect("create Wasmtime engine");
  let component = Component::new(&engine, &wasm_a).expect("valid WASI HTTP+DB Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:http/types@0.2.12"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:io/streams@0.2.12"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:io/error@0.2.12"), "imports: {imports:?}");
  assert!(imports.contains(&"calcit:db/sql@0.1.0"), "imports: {imports:?}");
  assert!(imports.contains(&"calcit:db/kv@0.1.0"), "imports: {imports:?}");
  let exports = component_type.exports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert_eq!(exports, vec!["wasi:http/incoming-handler@0.2.12"]);
}

#[test]
fn http_db_sql_only_component_omits_kv_import() {
  let dir = tempfile::tempdir().expect("sql-only directory");
  let snapshot = dir.path().join("sql-only.cirru");
  fs::copy("tests/fixtures/wasi-http-db.cirru", &snapshot).expect("copy HTTP+DB fixture");

  edit_def(
    &snapshot,
    "app.main/handle",
    concat!(
      "quote $ defn handle (req)\n",
      "  if (= (:path req) |/items)\n",
      "    match (query-as \"|SELECT id, name FROM items ORDER BY id\" ([]) app.main/ItemRow)\n",
      "      (:err e) (Result :ok (db-error-response e))\n",
      "      (:ok rows)\n",
      "        match (number->int32 (bump-hits!))\n",
      "          (:err reason) (Result :ok (plain-response 500 reason))\n",
      "          (:ok hits) (Result :ok (json-response 200 (json-stringify (encode-map-as (ItemsReply :hits hits :items rows) app.main/ItemsReply))))\n",
      "    Result :ok $ plain-response 404 |not-found",
    ),
    "quote $ Fn HttpRequest (-> (Result HttpResponse String)) (:features :state)",
  );
  rm_def(&snapshot, "app.main/kv-get");
  rm_def(&snapshot, "app.main/kv-set");
  rm_def(&snapshot, "app.main/kv-delete");

  let emit_dir = tempfile::tempdir().expect("sql-only emit dir");
  let built = calcit(&[snapshot.to_str().unwrap(), "wasi", "--world", "http"], emit_dir.path());
  assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
  let wasm_bytes = fs::read(emit_dir.path().join("program.wasm")).expect("read SQL-only Component");

  let engine = Engine::new(&Config::new()).expect("create Wasmtime engine");
  let component = Component::new(&engine, &wasm_bytes).expect("valid SQL-only Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"calcit:db/sql@0.1.0"), "imports: {imports:?}");
  assert!(
    !imports.contains(&"calcit:db/kv@0.1.0"),
    "SQL-only HTTP+DB Component must omit calcit:db/kv@0.1.0: {imports:?}"
  );
}

#[test]
fn http_db_validator_rejects_disallowed_worlds_unknown_symbols_async_and_shape_drift() {
  let dir = tempfile::tempdir().expect("validator error check dir");
  let out = tempfile::tempdir().expect("validator emit dir");
  let copy_entry = |name: &str| -> std::path::PathBuf {
    let dest = dir.path().join(name);
    fs::copy("tests/fixtures/wasi-http-db.cirru", &dest).expect("copy fixture");
    dest
  };

  // DB imports on `--world command` must still fail with E_WASM_CAPABILITY.
  let cmd_res = calcit(
    &[
      "tests/fixtures/wasi-http-db.cirru",
      "wasi",
      "--world",
      "command",
      "--init-fn",
      "app.main/reload!",
      "--check-only",
    ],
    out.path(),
  );
  assert!(!cmd_res.status.success());
  let cmd_err = String::from_utf8_lossy(&cmd_res.stderr);
  assert!(cmd_err.contains("E_WASM_CAPABILITY"), "{cmd_err}");

  // Isolate handle from DB helper signatures for negative DB import validation cases.
  let make_minimal_copy = |name: &str| {
    let dest = copy_entry(name);
    edit_def(
      &dest,
      "app.main/handle",
      "quote $ defn handle (_req) (Result :ok (plain-response 200 |ok))",
      "quote $ Fn HttpRequest (-> (Result HttpResponse String))",
    );
    for obsolete in [
      "app.main/db-error-response",
      "app.main/query-as",
      "app.main/tx-query-as",
      "app.main/sql-exec",
      "app.main/tx-begin",
      "app.main/tx-commit",
      "app.main/tx-exec",
      "app.main/tx-query",
      "app.main/tx-rollback",
      "app.main/kv-get",
      "app.main/kv-set",
      "app.main/kv-delete",
    ] {
      rm_def(&dest, obsolete);
    }
    dest
  };

  // Arbitrary custom import on `--world http` still fails with E_WASM_CAPABILITY.
  let custom_imp = make_minimal_copy("custom-import.cirru");
  edit_def(
    &custom_imp,
    "app.main/custom-imp",
    "quote $ defwasm-import custom-imp (x) |host |hook",
    "quote $ Fn Number (-> Number)",
  );
  let res = calcit(
    &[custom_imp.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASM_CAPABILITY"), "{err}");

  // Unknown symbol inside calcit:db/sql@0.1.0 must fail with E_WASI_HTTP_DB_IMPORT.
  let unknown_sym = make_minimal_copy("unknown-symbol.cirru");
  edit_def(
    &unknown_sym,
    "app.main/bad-db",
    "quote $ defwasm-import bad-db () |calcit:db/sql@0.1.0 |drop-all",
    "quote $ Fn (-> (Result Unit DbError))",
  );
  let res = calcit(&[unknown_sym.to_str().unwrap(), "wasi", "--world", "http"], out.path());
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_DB_IMPORT"), "{err}");
  assert!(!out.path().join("program.wasm").exists());

  // Wrong parameter schema on calcit:db/sql@0.1.0 query must fail with E_WASI_HTTP_DB_IMPORT.
  let bad_params = make_minimal_copy("bad-params.cirru");
  edit_def(
    &bad_params,
    "app.main/sql-query",
    "quote $ defwasm-import sql-query (sql params) |calcit:db/sql@0.1.0 |query",
    "quote $ Fn String Number (-> (Result DbQueryResult DbError))",
  );
  let res = calcit(
    &[bad_params.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_DB_IMPORT"), "{err}");

  // Async invocation schema on calcit:db/sql@0.1.0 query must fail with E_WASI_HTTP_DB_IMPORT.
  let async_db = make_minimal_copy("async-db.cirru");
  edit_def(
    &async_db,
    "app.main/sql-query",
    "quote $ defwasm-import sql-query (sql params) |calcit:db/sql@0.1.0 |query",
    "quote $ Fn String (List DbParam) (-> (Result DbQueryResult DbError)) (:async)",
  );
  let res = calcit(&[async_db.to_str().unwrap(), "wasi", "--world", "http", "--check-only"], out.path());
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_DB_IMPORT"), "{err}");

  // Drifted DbError enum variants must fail with E_WASI_HTTP_DB_IMPORT.
  let drifted_err = make_minimal_copy("drifted-error.cirru");
  edit_def(
    &drifted_err,
    "app.main/DbError",
    "quote $ defenum DbError (:backend String) (:capability-denied String)",
    "quote EnumDef",
  );
  let res = calcit(
    &[drifted_err.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_DB_IMPORT"), "{err}");

  // Drifted DbQueryResult struct fields must fail with E_WASI_HTTP_DB_IMPORT.
  let drifted_query = make_minimal_copy("drifted-query-result.cirru");
  edit_def(
    &drifted_query,
    "app.main/DbQueryResult",
    "quote $ defstruct DbQueryResult (:columns (List String)) (:rows String)",
    "quote StructDef",
  );
  let res = calcit(
    &[drifted_query.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_DB_IMPORT"), "{err}");

  // Unsupported Component ABI type (e.g. Map) on a recognized DB import must still report
  // E_WASI_HTTP_DB_IMPORT while preserving the underlying E_COMPONENT_ABI_* detail.
  let unsupported_db_type = make_minimal_copy("unsupported-db-type.cirru");
  edit_def(
    &unsupported_db_type,
    "app.main/kv-get",
    "quote $ defwasm-import kv-get (key) |calcit:db/kv@0.1.0 |get",
    "quote $ Fn String (-> (Result (Map String String) DbError))",
  );
  let res = calcit(
    &[unsupported_db_type.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(
    err.contains("E_WASI_HTTP_DB_IMPORT") && err.contains("E_COMPONENT_ABI_UNSUPPORTED_TYPE"),
    "{err}"
  );
}

#[tokio::test]
async fn http_db_service_end_to_end_sqlite_kv_default_deny_tx_and_memory_ceiling() {
  let emit_dir = tempfile::tempdir().expect("emit dir for HTTP+DB service");
  let built = calcit(&["tests/fixtures/wasi-http-db.cirru", "wasi", "--world", "http"], emit_dir.path());
  assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
  let component_bytes = fs::read(emit_dir.path().join("program.wasm")).expect("read compiled HTTP+DB Component");

  // 1. Default-deny host policy rejects SQL & KV capabilities cleanly with 403.
  let denied_state = WasmtimeDbState::new(
    DbConfig::default(),
    Box::new(SqliteSqlBackend::in_memory().expect("open in-memory SQLite")),
    Box::new(InMemoryKvBackend::new()),
  );
  let mut denied_runner = HttpDbServiceRunner::new(&component_bytes, denied_state, Some(MEMORY_LIMIT_BYTES))
    .await
    .expect("instantiate default-deny HttpDbServiceRunner");

  for (method, path, body) in [
    (Method::GET, "/items", Vec::new()),
    (Method::POST, "/items", br#"{"name":"alice"}"#.to_vec()),
    (Method::GET, "/kv", Vec::new()),
    (Method::POST, "/kv", vec![0x00, 0xff, 0x01]),
  ] {
    let resp = denied_runner
      .handle_request(method.clone(), path, Vec::new(), body)
      .await
      .expect("default-deny turn returns HTTP 403");
    assert_eq!(resp.status, 403, "{method} {path}: {:?}", String::from_utf8_lossy(&resp.body));
    assert!(resp.peak_memory_bytes <= MEMORY_LIMIT_BYTES);
  }
  drop(denied_runner);

  // 2. Explicit SQL read/write + prefixed KV service with DDL denied.
  let mut sqlite = SqliteSqlBackend::in_memory().expect("open seeded SQLite");
  sqlite
    .exec(
      &DbConfig::default().allow_sql_ddl(),
      "CREATE TABLE items (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL)",
      &[],
    )
    .expect("create items schema before locking down DDL");

  let granted_config = DbConfig::default().allow_sql_read().allow_sql_write().allow_kv_prefix("app:");
  let granted_state = WasmtimeDbState::new(granted_config, Box::new(sqlite), Box::new(InMemoryKvBackend::new()));
  let mut runner = HttpDbServiceRunner::new(&component_bytes, granted_state, Some(MEMORY_LIMIT_BYTES))
    .await
    .expect("instantiate granted HttpDbServiceRunner");

  let health = runner
    .handle_request(Method::GET, "/health", Vec::new(), Vec::new())
    .await
    .expect("GET /health");
  assert_eq!(health.status, 200);
  assert_eq!(String::from_utf8_lossy(&health.body), "ok:1");

  let insert_alice = runner
    .handle_request(Method::POST, "/items", Vec::new(), br#"{"name":"alice"}"#.to_vec())
    .await
    .expect("POST /items alice");
  assert_eq!(insert_alice.status, 201, "{}", String::from_utf8_lossy(&insert_alice.body));
  let insert_alice_json: Value = serde_json::from_slice(&insert_alice.body).expect("JSON InsertReply");
  assert_eq!(insert_alice_json["rows-affected"], 1);
  assert_eq!(insert_alice_json["last-insert-id"], 1);

  let insert_bob = runner
    .handle_request(Method::POST, "/items", Vec::new(), br#"{"name":"bob"}"#.to_vec())
    .await
    .expect("POST /items bob");
  assert_eq!(insert_bob.status, 201);
  let insert_bob_json: Value = serde_json::from_slice(&insert_bob.body).expect("JSON InsertReply");
  assert_eq!(insert_bob_json["rows-affected"], 1);
  assert_eq!(insert_bob_json["last-insert-id"], 2);

  let dup_alice = runner
    .handle_request(Method::POST, "/items", Vec::new(), br#"{"name":"alice"}"#.to_vec())
    .await
    .expect("duplicate POST /items");
  assert_eq!(dup_alice.status, 409, "{}", String::from_utf8_lossy(&dup_alice.body));

  let list_initial = runner
    .handle_request(Method::GET, "/items", Vec::new(), Vec::new())
    .await
    .expect("GET /items");
  assert_eq!(list_initial.status, 200, "{}", String::from_utf8_lossy(&list_initial.body));
  let list_initial_json: Value = serde_json::from_slice(&list_initial.body).expect("JSON ItemsReply");
  assert_eq!(list_initial_json["hits"], 2);
  let items = list_initial_json["items"].as_array().expect("items array");
  assert_eq!(items.len(), 2);
  assert_eq!(items[0]["id"], 1);
  assert_eq!(items[0]["name"], "alice");
  assert_eq!(items[1]["id"], 2);
  assert_eq!(items[1]["name"], "bob");

  let ddl_denied = runner
    .handle_request(Method::POST, "/ddl", Vec::new(), Vec::new())
    .await
    .expect("POST /ddl");
  assert_eq!(ddl_denied.status, 403, "{}", String::from_utf8_lossy(&ddl_denied.body));

  let tx_commit = runner
    .handle_request(Method::POST, "/tx/commit", Vec::new(), Vec::new())
    .await
    .expect("POST /tx/commit");
  assert_eq!(tx_commit.status, 200, "{}", String::from_utf8_lossy(&tx_commit.body));
  assert_eq!(tx_commit.leaked_tx_rolled_back, 0);
  let tx_commit_json: Value = serde_json::from_slice(&tx_commit.body).expect("JSON tx-commit reply");
  assert_eq!(tx_commit_json["items"].as_array().expect("items").len(), 3);

  let tx_rollback = runner
    .handle_request(Method::POST, "/tx/rollback", Vec::new(), Vec::new())
    .await
    .expect("POST /tx/rollback");
  assert_eq!(tx_rollback.status, 200);
  assert_eq!(String::from_utf8_lossy(&tx_rollback.body), "rolled-back");
  assert_eq!(tx_rollback.leaked_tx_rolled_back, 0);

  let tx_leak = runner
    .handle_request(Method::POST, "/tx/leak", Vec::new(), Vec::new())
    .await
    .expect("POST /tx/leak");
  assert_eq!(tx_leak.status, 200);
  assert_eq!(String::from_utf8_lossy(&tx_leak.body), "leaked");
  assert_eq!(tx_leak.leaked_tx_rolled_back, 1);

  let after_tx = runner
    .handle_request(Method::GET, "/items", Vec::new(), Vec::new())
    .await
    .expect("GET /items after rollback/leak");
  assert_eq!(after_tx.status, 200);
  let after_tx_json: Value = serde_json::from_slice(&after_tx.body).expect("JSON ItemsReply");
  let names = after_tx_json["items"]
    .as_array()
    .expect("items array")
    .iter()
    .map(|row| row["name"].as_str().expect("row name"))
    .collect::<Vec<_>>();
  assert_eq!(names, vec!["alice", "bob", "tx-committed"]);

  // KV binary round-trip + delete + prefix denial.
  let raw_kv = vec![0x00, 0xff, 0x80, 0x42];
  let kv_set = runner
    .handle_request(Method::POST, "/kv", Vec::new(), raw_kv.clone())
    .await
    .expect("POST /kv");
  assert_eq!(kv_set.status, 200);
  assert_eq!(String::from_utf8_lossy(&kv_set.body), "stored");

  let kv_get = runner
    .handle_request(Method::GET, "/kv", Vec::new(), Vec::new())
    .await
    .expect("GET /kv");
  assert_eq!(kv_get.status, 200);
  assert_eq!(kv_get.body, raw_kv);

  let kv_del = runner
    .handle_request(Method::DELETE, "/kv", Vec::new(), Vec::new())
    .await
    .expect("DELETE /kv");
  assert_eq!(kv_del.status, 200);
  assert_eq!(String::from_utf8_lossy(&kv_del.body), "true");

  let kv_missing = runner
    .handle_request(Method::GET, "/kv", Vec::new(), Vec::new())
    .await
    .expect("GET /kv missing");
  assert_eq!(kv_missing.status, 404);
  assert_eq!(String::from_utf8_lossy(&kv_missing.body), "missing");

  // Use non-UTF-8 bytes so the HTTP adapter produces HttpBody :bytes and exercises kv-set denial.
  let kv_denied_set = runner
    .handle_request(Method::POST, "/kv/denied", Vec::new(), vec![0xff, 2, 3])
    .await
    .expect("POST /kv/denied set");
  assert_eq!(kv_denied_set.status, 403);

  let kv_denied_get = runner
    .handle_request(Method::POST, "/kv/denied", Vec::new(), Vec::new())
    .await
    .expect("POST /kv/denied get");
  assert_eq!(kv_denied_get.status, 403);

  // Repeated GET /items loop verifies Request Arena reset under an 8 MiB linear memory ceiling.
  let turns = 60_u64;
  let started = Instant::now();
  let mut last_hits = 0_i64;
  for step in 0..turns {
    let resp = runner
      .handle_request(Method::GET, "/items", Vec::new(), Vec::new())
      .await
      .expect("repeated GET /items turn");
    assert_eq!(resp.status, 200, "turn {step} status");
    assert_eq!(resp.leaked_tx_rolled_back, 0);
    assert!(
      resp.peak_memory_bytes <= MEMORY_LIMIT_BYTES,
      "turn {step} exceeded memory ceiling: {} > {MEMORY_LIMIT_BYTES}",
      resp.peak_memory_bytes
    );
    let parsed: Value = serde_json::from_slice(&resp.body).expect("repeated JSON reply");
    assert_eq!(parsed["items"].as_array().expect("items").len(), 3);
    let hits = parsed["hits"].as_i64().expect("hits int");
    assert!(hits > last_hits, "hits should advance monotonically: {last_hits} -> {hits}");
    last_hits = hits;
  }
  let elapsed = started.elapsed();
  let throughput = turns as f64 / elapsed.as_secs_f64();
  let peak_bytes = runner.peak_memory_bytes();
  assert!(
    peak_bytes <= MEMORY_LIMIT_BYTES,
    "final peak memory exceeded ceiling: {peak_bytes} > {MEMORY_LIMIT_BYTES}"
  );
  eprintln!(
    "[wasi_http_db_cli] {turns} GET /items turns in {:.3}s ({:.1} turns/sec), peak linear memory = {} bytes ({:.2} MiB)",
    elapsed.as_secs_f64(),
    throughput,
    peak_bytes,
    peak_bytes as f64 / (1024.0 * 1024.0)
  );
}

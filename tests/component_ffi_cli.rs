use std::path::Path;
use std::process::{Command, Output};

use cirru_edn::Edn;

fn run_calcit(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(Path::new("calcit/test-wasm.cirru"))
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn run_component_fixture(path: &str, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(path)
    .args(args)
    .output()
    .expect("calcit fixture command should run")
}

fn stdout(output: &Output) -> String {
  assert!(
    output.status.success(),
    "command failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn json_as_edn(value: &serde_json::Value) -> Edn {
  match value {
    serde_json::Value::Null => Edn::Nil,
    serde_json::Value::Bool(value) => Edn::Bool(*value),
    serde_json::Value::Number(value) => Edn::Number(value.as_f64().expect("contract numbers should fit f64")),
    serde_json::Value::String(value) => Edn::str(value.as_str()),
    serde_json::Value::Array(values) => Edn::List(cirru_edn::EdnListView(values.iter().map(json_as_edn).collect())),
    serde_json::Value::Object(values) => Edn::map_from_iter(
      values
        .iter()
        .map(|(key, value)| (Edn::tag(key.replace('_', "-")), json_as_edn(value))),
    ),
  }
}

#[test]
fn component_contract_defaults_to_edn_and_matches_explicit_json() {
  let edn_output = run_calcit(&["ffi", "export", "--boundary", "component"]);
  let edn = cirru_edn::parse(&stdout(&edn_output)).expect("default component stdout should be one Cirru EDN value");
  let json_output = run_calcit(&["ffi", "export", "--boundary", "component", "--format", "json"]);
  let json: serde_json::Value =
    serde_json::from_str(&stdout(&json_output)).expect("explicit component JSON stdout should be one JSON value");

  assert_eq!(edn, json_as_edn(&json));
  assert!(
    json["interface_schema"]
      .as_str()
      .is_some_and(|schema| schema.ends_with("component-interface-ir-v4.schema.json"))
  );
  assert_eq!(json["data"]["filters"]["boundary"], "component");
  assert_eq!(json["data"]["interface"]["version"], 4);
  let import_definition = json["data"]["interface"]["definitions"]
    .as_array()
    .expect("component definitions should be an array")
    .iter()
    .find(|definition| definition["id"] == "test-wasm.main/host-string-upcase")
    .expect("Calcit sync import fixture should be exported");
  assert_eq!(import_definition["direction"], "import");
  assert_eq!(import_definition["invocation"], "sync");
  let async_definition = json["data"]["interface"]["definitions"]
    .as_array()
    .expect("component definitions should be an array")
    .iter()
    .find(|definition| definition["id"] == "test-wasm.main/wasm-ffi-async-echo")
    .expect("Calcit async boundary fixture should be exported");
  assert_eq!(async_definition["invocation"], "async");
  assert!(
    async_definition["logical_schema"]
      .as_str()
      .is_some_and(|schema| schema.contains("(:async true)"))
  );
}

#[test]
fn buffered_http_contract_is_closed_and_explicitly_bounded() {
  let output = run_component_fixture(
    "tests/fixtures/component-wasm-async-import.cirru",
    &["ffi", "export", "--boundary", "component", "--format", "json"],
  );
  let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("buffered HTTP contract should be valid JSON");
  let interface = &json["data"]["interface"];
  let definitions = interface["definitions"]
    .as_array()
    .expect("component definitions should be an array");
  let request = definitions
    .iter()
    .find(|definition| definition["id"] == "component-wasm-async-import.main/host-http-request")
    .expect("buffered HTTP request import should be exported");
  assert_eq!(request["direction"], "import");
  assert_eq!(request["invocation"], "async");
  assert_eq!(request["module"], "calcit:wasi-http/client");
  assert_eq!(request["symbol"], "request");
  assert_eq!(request["signature"]["result"]["kind"], "result");

  let declarations = interface["declarations"]
    .as_array()
    .expect("component declarations should be an array");
  let http_request = declarations
    .iter()
    .find(|declaration| declaration["id"] == "component-wasm-async-import.main/HttpRequest")
    .expect("HTTP request declaration should be exported");
  let max_response_bytes = http_request["fields"]
    .as_array()
    .expect("HTTP request fields should be an array")
    .iter()
    .find(|field| field["name"] == "max-response-bytes")
    .expect("HTTP request should carry an explicit response bound");
  assert_eq!(max_response_bytes["type"]["kind"], "uint64");

  let http_error = declarations
    .iter()
    .find(|declaration| declaration["id"] == "component-wasm-async-import.main/HttpError")
    .expect("HTTP error declaration should be exported");
  assert!(
    http_error["variants"]
      .as_array()
      .expect("HTTP error variants should be an array")
      .iter()
      .any(|variant| variant["name"] == "response-too-large"),
    "response overflow must remain a typed failure"
  );
}

#[test]
fn scoped_stream_contract_exports_the_builtin_failure_enum() {
  let output = run_component_fixture(
    "tests/fixtures/component-wasm-stream.cirru",
    &["ffi", "export", "--boundary", "component", "--format", "json"],
  );
  let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("stream contract should be valid JSON");
  assert_eq!(json["data"]["summary"]["unsupported"], 0);
  let interface = &json["data"]["interface"];
  let consume = interface["definitions"]
    .as_array()
    .expect("stream definitions should be an array")
    .iter()
    .find(|definition| definition["id"] == "component-wasm-stream.main/consume")
    .expect("stream consumer should be exported");
  assert_eq!(consume["signature"]["parameters"][0]["type"]["kind"], "readable-byte-stream");
  assert_eq!(consume["signature"]["result"]["kind"], "result");
  let error = interface["declarations"]
    .as_array()
    .expect("stream declarations should be an array")
    .iter()
    .find(|declaration| declaration["id"] == "calcit.core/StreamConsumeError")
    .expect("stream failure enum should be exported");
  assert_eq!(error["variants"][0]["name"], "total-limit");
}

#[test]
fn native_json_keeps_the_v4_envelope_without_a_boundary_field() {
  let output = run_calcit(&["ffi", "export", "--json", "--ns", "test-wasm.main"]);
  let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("native JSON stdout should stay parseable");

  assert!(
    json["interface_schema"]
      .as_str()
      .is_some_and(|schema| schema.ends_with("ffi-interface-ir-v4.schema.json"))
  );
  assert_eq!(json["data"]["interface"]["version"], 4);
  assert!(json["data"]["filters"].get("boundary").is_none());
}

#[test]
fn component_core_adapter_accepts_explicit_async_exports() {
  let output = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-async-export.cirru",
      "wasm",
      "--boundary",
      "component",
      "--check-only",
    ])
    .output()
    .expect("async Component fixture check should run");
  assert!(
    output.status.success(),
    "async Component check failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn ffi_resource_and_named_union_end_to_end_check_export_and_wasm_rejection() {
  let snapshot_text = r#"
{} (:about "|FFI resource and named union end-to-end fixture") (:package |regex)
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'regex.core/main!) (:mode :native) (:reload-fn 'regex.core/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'regex.core $ %{} 'FileEntry
      :defs $ {}
        'Regex $ %{} 'CodeEntry (:doc "|Compiled regex holding an opaque native resource handle") (:schema $ quote StructDef)
          :code $ quote
            defstruct Regex
              :source String
              :handle FfiResource
          :examples $ []
          :tests $ []
        'Pattern $ %{} 'CodeEntry (:doc "|String or compiled Regex") (:schema $ quote UnionDef)
          :code $ quote
            defunion Pattern String Regex
          :examples $ []
          :tests $ []
        'decode-handle $ %{} 'CodeEntry (:doc "|Checked runtime projection from Dynamic FFI return to FfiResource")
          :schema $ quote $ Fn Dynamic (-> (Result FfiResource String))
          :code $ quote
            defn decode-handle (raw)
              try-decode-map-as raw FfiResource
          :examples $ []
          :tests $ []
            %{} 'TestEntry (:name |rejects-non-resource-string)
              :code $ quote
                assert= true $ .err? $ decode-handle |not-a-handle
            %{} 'TestEntry (:name |rejects-non-resource-nil)
              :code $ quote
                assert= true $ .err? $ decode-handle nil
        'compile-regex $ %{} 'CodeEntry (:doc "|Compile a pattern into a Regex")
          :ffi $ {} (:backend :native) (:symbol |compile_regex) (:invoke :sync) (:transport :edn-buffer-v1)
          :schema $ quote $ Fn String (-> (Result Regex String))
          :code $ quote
            defn compile-regex (pattern)
              let
                  raw $ &call-dylib-edn |libregex |compile_regex pattern
                match (try-decode-map-as raw (Result FfiResource String))
                  (:ok inner)
                    match inner
                      (:ok handle) (Result :ok $ Regex :source pattern :handle handle)
                      (:err msg) (Result :err msg)
                  (:err err) (Result :err err)
          :examples $ []
          :tests $ []
        'is-match $ %{} 'CodeEntry (:doc "|Test a Pattern against text")
          :ffi $ {} (:backend :native) (:symbol |is_match) (:invoke :sync) (:transport :edn-buffer-v1)
          :schema $ quote $ Fn Pattern String (-> Bool)
          :code $ quote
            defn is-match (pattern text)
              match pattern
                (String s) (= s text)
                (Regex r) (= (:source r) text)
          :examples $ []
          :tests $ []
            %{} 'TestEntry (:name |matches-string-member)
              :code $ quote
                assert= true $ is-match |abc |abc
        'main! $ %{} 'CodeEntry (:doc "|Entry") (:schema $ quote $ Fn (-> Unit))
          :code $ quote
            defn main! ()
              , &unit
          :examples $ []
          :tests $ []
      :ns $ %{} 'NsEntry (:doc "|")
        :code $ quote (ns regex.core)
"#;

  let dir = tempfile::tempdir().expect("tempdir");
  let snapshot_path = dir.path().join("calcit.cirru");
  std::fs::write(&snapshot_path, snapshot_text.trim_start()).expect("write fixture snapshot");
  let snapshot_arg = snapshot_path.to_str().expect("utf8 path");

  // 1. Static type-check across all definitions succeeds with no unsafe-coerce.
  let check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", snapshot_arg, "--check-only", "--all-defs"])
    .output()
    .expect("run calcit --check-only --all-defs");
  assert!(
    check.status.success(),
    "check-only failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );

  // 2. Calcit definition `:tests` pass.
  let test_run = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", snapshot_arg, "test", "--require-match"])
    .output()
    .expect("run calcit test");
  assert!(
    test_run.status.success(),
    "calcit test failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&test_run.stdout),
    String::from_utf8_lossy(&test_run.stderr)
  );

  // 3. `calcit ffi export --json --ns regex.core` exports IR v4 with Union + FfiResource.
  let export = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      snapshot_arg,
      "ffi",
      "export",
      "--json",
      "--ns",
      "regex.core",
    ])
    .output()
    .expect("run calcit ffi export");
  assert!(
    export.status.success(),
    "ffi export failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&export.stdout),
    String::from_utf8_lossy(&export.stderr)
  );
  let json: serde_json::Value = serde_json::from_str(&stdout(&export)).expect("parse ffi export JSON");
  assert_eq!(json["data"]["interface"]["version"], 4);
  assert_eq!(json["data"]["summary"]["supported"], 2);
  assert_eq!(json["data"]["summary"]["unsupported"], 0);
  let declarations = json["data"]["interface"]["declarations"].as_array().expect("declarations array");
  assert!(declarations.iter().any(|d| d["kind"] == "union" && d["id"] == "regex.core/Pattern"));
  assert!(declarations.iter().any(|d| {
    d["kind"] == "struct"
      && d["id"] == "regex.core/Regex"
      && d["fields"]
        .as_array()
        .is_some_and(|fields| fields.iter().any(|f| f["name"] == "handle" && f["type"]["kind"] == "ffi-resource"))
  }));

  // 4. WASM target rejects FfiResource explicitly (never silently erases to f64/Dynamic).
  let wasm_check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args(["--tips-level", "none", snapshot_arg, "wasm", "--check-only"])
    .output()
    .expect("run calcit wasm --check-only");
  assert!(!wasm_check.status.success(), "WASM check must fail when snapshot uses FfiResource");
  let wasm_stderr = String::from_utf8_lossy(&wasm_check.stderr);
  assert!(
    wasm_stderr.contains("E_WASM_UNSUPPORTED_FFI_RESOURCE"),
    "expected E_WASM_UNSUPPORTED_FFI_RESOURCE, got:\n{wasm_stderr}"
  );

  // 5. `encode-map-as` rejects FfiResource at compile time (non-serializable invariant).
  let encode_check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "eval",
      "--dep",
      snapshot_arg,
      "encode-map-as nil regex.core/Regex",
    ])
    .output()
    .expect("run eval encode-map-as on Regex");
  assert!(!encode_check.status.success(), "encode-map-as must reject Regex with FfiResource");
  let encode_stderr = String::from_utf8_lossy(&encode_check.stderr);
  assert!(
    encode_stderr.contains("E_ENCODER_DERIVE")
      || encode_stderr.contains("non-serializable")
      || encode_stderr.contains("E_ENCODE_MAP_AS_TYPE_MISMATCH"),
    "expected encoder rejection, got:\n{encode_stderr}"
  );

  // 6. `parse-cirru-edn-as` rejects FfiResource at compile time (non-serializable invariant).
  let parse_check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "eval",
      "--dep",
      snapshot_arg,
      "parse-cirru-edn-as |nil FfiResource",
    ])
    .output()
    .expect("run eval parse-cirru-edn-as on FfiResource");
  assert!(
    !parse_check.status.success(),
    "parse-cirru-edn-as must reject non-serializable FfiResource"
  );
  let parse_stderr = String::from_utf8_lossy(&parse_check.stderr);
  assert!(
    parse_stderr.contains("E_DECODER_DERIVE") && parse_stderr.contains("non-serializable"),
    "expected E_DECODER_DERIVE non-serializable error, got:\n{parse_stderr}"
  );

  // 7. Direct construction of Regex with String (or unchecked Dynamic) fails statically.
  let bad_ctor = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "eval",
      "--dep",
      snapshot_arg,
      "regex.core/Regex :source |abc :handle |not-a-resource",
    ])
    .output()
    .expect("run eval bad Regex ctor");
  assert!(!bad_ctor.status.success(), "Regex constructor must reject String for :handle");
  let bad_ctor_stderr = String::from_utf8_lossy(&bad_ctor.stderr);
  assert!(
    bad_ctor_stderr.contains("E_FN_ARG_TYPE_MISMATCH")
      || bad_ctor_stderr.contains("ffi-resource")
      || bad_ctor_stderr.contains("FfiResource"),
    "expected static type mismatch for Regex :handle, got:\n{bad_ctor_stderr}"
  );
}

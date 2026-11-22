use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

struct ChildGuard(Child);

impl ChildGuard {
  fn kill_and_collect_output(&mut self) -> (String, String) {
    let _ = self.0.kill();
    let _ = self.0.wait();
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = self.0.stdout.take() {
      let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = self.0.stderr.take() {
      let _ = err.read_to_string(&mut stderr);
    }
    (stdout, stderr)
  }
}

impl Drop for ChildGuard {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

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

fn find_wasmtime_cli() -> Option<PathBuf> {
  if let Some(cli) = std::env::var_os("WASMTIME_CLI") {
    return Some(PathBuf::from(cli));
  }
  let probe = Command::new("wasmtime").arg("--version").output().ok()?;
  probe.status.success().then(|| PathBuf::from("wasmtime"))
}

fn free_loopback_addr() -> SocketAddr {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
  listener.local_addr().expect("ephemeral local addr")
}

fn connect_with_retry(child: &mut Child, addr: SocketAddr) -> TcpStream {
  let deadline = Instant::now() + Duration::from_secs(10);
  loop {
    if let Some(status) = child.try_wait().expect("poll wasmtime serve") {
      let mut stderr = String::new();
      if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
      }
      panic!("wasmtime serve exited early ({status}): {stderr}");
    }
    match TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
      Ok(stream) => return stream,
      Err(err) => {
        if Instant::now() >= deadline {
          panic!("timed out connecting to wasmtime serve at {addr}: {err}");
        }
        thread::sleep(Duration::from_millis(50));
      }
    }
  }
}

struct HttpReply {
  status: u16,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

fn decode_chunked_bytes(raw: &[u8]) -> Vec<u8> {
  let mut rest = raw;
  let mut out = Vec::new();
  loop {
    let pos = rest
      .windows(2)
      .position(|w| w == b"\r\n")
      .unwrap_or_else(|| panic!("invalid chunked header in {} bytes", raw.len()));
    let size_line = std::str::from_utf8(&rest[..pos]).expect("ASCII chunk size");
    let size_hex = size_line.split(';').next().expect("chunk size").trim();
    let size = usize::from_str_radix(size_hex, 16).unwrap_or_else(|e| panic!("bad chunk size {size_hex:?}: {e}"));
    rest = &rest[pos + 2..];
    if size == 0 {
      break;
    }
    assert!(rest.len() >= size + 2, "truncated chunk of {size} bytes (got {})", rest.len());
    out.extend_from_slice(&rest[..size]);
    assert_eq!(&rest[size..size + 2], b"\r\n", "missing CRLF after chunk");
    rest = &rest[size + 2..];
  }
  out
}

fn send_raw_http(child: &mut Child, addr: SocketAddr, raw_request: &[u8]) -> HttpReply {
  let mut stream = connect_with_retry(child, addr);
  stream.set_read_timeout(Some(Duration::from_secs(10))).expect("set read timeout");
  stream.set_write_timeout(Some(Duration::from_secs(10))).expect("set write timeout");
  // Large payloads (e.g. >4 MiB 413) may receive an early error response and close before all bytes flush.
  let _ = stream.write_all(raw_request);
  let _ = stream.flush();

  let mut response = Vec::new();
  let mut buf = [0u8; 8192];
  loop {
    match stream.read(&mut buf) {
      Ok(0) => break,
      Ok(n) => response.extend_from_slice(&buf[..n]),
      Err(err) if err.kind() == std::io::ErrorKind::ConnectionReset && !response.is_empty() => break,
      Err(err) if err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::TimedOut => {
        panic!(
          "timed out reading HTTP response (received {} bytes so far): {:?}",
          response.len(),
          String::from_utf8_lossy(&response)
        );
      }
      Err(err) => panic!(
        "read HTTP response failed after {} bytes ({err:?}): {:?}",
        response.len(),
        String::from_utf8_lossy(&response)
      ),
    }
  }

  let header_end = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or_else(|| {
    panic!(
      "invalid HTTP response ({} bytes): {:?}",
      response.len(),
      String::from_utf8_lossy(&response)
    )
  });
  let head_text = std::str::from_utf8(&response[..header_end]).expect("UTF-8 HTTP response head");
  let raw_body = &response[header_end + 4..];

  let mut lines = head_text.split("\r\n");
  let status_line = lines.next().expect("HTTP status line");
  let status = status_line
    .split_whitespace()
    .nth(1)
    .and_then(|code| code.parse::<u16>().ok())
    .unwrap_or_else(|| panic!("unexpected HTTP status line: {status_line:?}"));

  let mut headers = Vec::new();
  let mut is_chunked = false;
  let mut content_length: Option<usize> = None;
  for line in lines {
    if let Some((name, value)) = line.split_once(':') {
      let lower_name = name.trim().to_ascii_lowercase();
      let trimmed_val = value.trim().to_owned();
      if lower_name == "transfer-encoding" && trimmed_val.to_ascii_lowercase().contains("chunked") {
        is_chunked = true;
      } else if lower_name == "content-length" {
        content_length = trimmed_val.parse::<usize>().ok();
      }
      headers.push((lower_name, trimmed_val));
    }
  }

  let body = if is_chunked {
    decode_chunked_bytes(raw_body)
  } else if let Some(len) = content_length {
    raw_body[..len.min(raw_body.len())].to_vec()
  } else {
    raw_body.to_vec()
  };

  HttpReply { status, headers, body }
}

fn send_http(child: &mut Child, addr: SocketAddr, method: &str, path: &str, headers: &[(&str, &[u8])], body: &[u8]) -> HttpReply {
  let mut req = Vec::new();
  write!(req, "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n").expect("write request line");
  for (name, value) in headers {
    write!(req, "{name}: ").expect("write header name");
    req.extend_from_slice(value);
    req.extend_from_slice(b"\r\n");
  }
  if !body.is_empty() {
    write!(req, "Content-Length: {}\r\n", body.len()).expect("write content-length");
  }
  req.extend_from_slice(b"\r\n");
  req.extend_from_slice(body);
  send_raw_http(child, addr, &req)
}

#[test]
fn http_proxy_component_is_deterministic_and_imports_exact_wasi_0212_interfaces() {
  let check_dir = tempfile::tempdir().expect("check-only directory");
  let checked = calcit(
    &["tests/fixtures/wasi-http-proxy.cirru", "wasi", "--world", "http", "--check-only"],
    check_dir.path(),
  );
  assert!(checked.status.success(), "{}", String::from_utf8_lossy(&checked.stderr));
  assert!(!check_dir.path().join("program.wasm").exists());

  let emit_a = tempfile::tempdir().expect("emit dir A");
  let emit_b = tempfile::tempdir().expect("emit dir B");
  let out_a = calcit(&["tests/fixtures/wasi-http-proxy.cirru", "wasi", "--world", "http"], emit_a.path());
  assert!(out_a.status.success(), "{}", String::from_utf8_lossy(&out_a.stderr));
  let out_b = calcit(&["tests/fixtures/wasi-http-proxy.cirru", "wasi", "--world", "http"], emit_b.path());
  assert!(out_b.status.success(), "{}", String::from_utf8_lossy(&out_b.stderr));

  let wasm_a = fs::read(emit_a.path().join("program.wasm")).expect("read first proxy Component");
  let wasm_b = fs::read(emit_b.path().join("program.wasm")).expect("read second proxy Component");
  assert_eq!(wasm_a, wasm_b, "WASI HTTP proxy Component output must be byte-deterministic");

  let config = Config::new();
  let engine = Engine::new(&config).expect("create Wasmtime engine");
  let component = Component::new(&engine, &wasm_a).expect("valid WASI 0.2.12 HTTP proxy Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:io/error@0.2.12"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:io/streams@0.2.12"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:http/types@0.2.12"), "imports: {imports:?}");
  assert!(
    !imports.contains(&"wasi:cli/stdout@0.2.12") && !imports.contains(&"wasi:cli/stderr@0.2.12"),
    "pure proxy fixture without stdio logging must omit CLI stdio imports: {imports:?}"
  );
  let exports = component_type.exports(&engine).map(|(name, _)| name).collect::<Vec<_>>();
  assert_eq!(exports, vec!["wasi:http/incoming-handler@0.2.12"]);
}

#[test]
fn http_proxy_cli_and_validator_reject_invalid_worlds_schemas_and_capabilities() {
  let out = tempfile::tempdir().expect("error check directory");

  let bad_world = calcit(&["tests/fixtures/wasi-http-proxy.cirru", "wasi", "--world", "bogus"], out.path());
  assert!(!bad_world.status.success());
  let bad_world_err = String::from_utf8_lossy(&bad_world.stderr);
  assert!(bad_world_err.contains("E_WASI_WORLD"), "{bad_world_err}");
  assert!(!out.path().join("program.wasm").exists());

  let native_http = calcit(
    &[
      "tests/fixtures/wasi-http-proxy.cirru",
      "wasi",
      "--world",
      "http",
      "--boundary",
      "native",
    ],
    out.path(),
  );
  assert!(!native_http.status.success());
  let native_http_err = String::from_utf8_lossy(&native_http.stderr);
  assert!(native_http_err.contains("E_WASI_WORLD"), "{native_http_err}");
  assert!(!out.path().join("program.wasm").exists());

  let dir = tempfile::tempdir().expect("temp fixture directory");

  // Entry validation failures (arity, missing schema, wrong param/return schema)
  let copy_entry = |name: &str| {
    let p = dir.path().join(name);
    fs::copy("tests/fixtures/wasi-http-proxy.cirru", &p).expect("copy HTTP proxy fixture");
    p
  };

  // Zero-arg command fixture passes target wrong-arity error
  let zero_arg = copy_entry("zero-arg.cirru");
  edit_def(
    &zero_arg,
    "app.main/handle",
    "quote $ defn handle () (Result :ok (plain-response 200 |ok))",
    "quote $ Fn (-> (Result HttpResponse String))",
  );
  let res = calcit(&[zero_arg.to_str().unwrap(), "wasi", "--world", "http"], out.path());
  assert!(!res.status.success());
  assert!(String::from_utf8_lossy(&res.stderr).contains("E_WASI_HTTP_ENTRY"));
  assert!(!out.path().join("program.wasm").exists());

  // Missing handler schema (strict mode rejects cleared schema before HTTP proxy entry check)
  let no_schema = copy_entry("no-schema.cirru");
  let rm_schema = calcit_raw(&[no_schema.to_str().unwrap(), "edit", "schema", "app.main/handle", "--clear"]);
  assert!(rm_schema.status.success(), "{}", String::from_utf8_lossy(&rm_schema.stderr));
  let res = calcit(
    &[no_schema.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(
    err.contains("E_WHOLE_DYNAMIC_PUBLIC_SCHEMA") || err.contains("E_WASI_HTTP_ENTRY"),
    "{err}"
  );

  // Wrong parameter type
  let wrong_param = copy_entry("wrong-param.cirru");
  edit_def(
    &wrong_param,
    "app.main/handle",
    "quote $ defn handle (_raw) (Result :ok (plain-response 200 |ok))",
    "quote $ Fn String (-> (Result HttpResponse String))",
  );
  let res = calcit(
    &[wrong_param.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_ENTRY"), "{err}");

  // Wrong return type
  let wrong_ret = copy_entry("wrong-ret.cirru");
  edit_def(
    &wrong_ret,
    "app.main/handle",
    "quote $ defn handle (_req) (plain-response 200 |ok)",
    "quote $ Fn HttpRequest (-> HttpResponse)",
  );
  let res = calcit(
    &[wrong_ret.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_ENTRY"), "{err}");

  // Generic defwasm-export must fail with E_WASI_HTTP_EXPORT
  let with_export = copy_entry("with-export.cirru");
  edit_def(
    &with_export,
    "app.main/extra-export",
    "quote $ defwasm-export extra-export (x) x",
    "quote $ Fn Number (-> Number)",
  );
  let res = calcit(&[with_export.to_str().unwrap(), "wasi", "--world", "http"], out.path());
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASI_HTTP_EXPORT"), "{err}");
  assert!(!out.path().join("program.wasm").exists());

  // Custom defwasm-import must fail with E_WASM_CAPABILITY
  let with_import = copy_entry("with-import.cirru");
  edit_def(
    &with_import,
    "app.main/custom-imp",
    "quote $ defwasm-import custom-imp (x) |host |hook",
    "quote $ Fn Number (-> Number)",
  );
  let res = calcit(
    &[with_import.to_str().unwrap(), "wasi", "--world", "http", "--check-only"],
    out.path(),
  );
  assert!(!res.status.success());
  let err = String::from_utf8_lossy(&res.stderr);
  assert!(err.contains("E_WASM_CAPABILITY"), "{err}");

  // Reachable command-only capabilities must fail with E_WASI_HTTP_CAPABILITY
  let cases = [
    (
      "get-args",
      "quote $ defn handle (_req) (let ((_a (get-args))) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "get-env",
      "quote $ defn handle (_req) (let ((_e (get-env |PATH))) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "quit",
      "quote $ defn handle (_req) (do (quit! 1) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "stdin",
      "quote $ defn handle (_req) (let ((_s (read-stdin-text))) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "fs-read",
      "quote $ defn handle (_req) (let ((_r (.read-text (fs:path |workspace/a.txt)))) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "clock",
      "quote $ defn handle (_req) (let ((_t (monotonic-time-ms))) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "wait",
      "quote $ defn handle (_req) (let ((_w (wait-ms 1))) (Result :ok (plain-response 200 |ok)))",
    ),
    (
      "random",
      "quote $ defn handle (_req) (let ((_b (secure-random-bytes 4))) (Result :ok (plain-response 200 |ok)))",
    ),
  ];
  for (label, body) in cases {
    let cap_file = copy_entry(&format!("cap-{label}.cirru"));
    edit_def(
      &cap_file,
      "app.main/handle",
      body,
      "quote $ Fn HttpRequest (-> (Result HttpResponse String)) (:features :io)",
    );
    let res = calcit(&[cap_file.to_str().unwrap(), "wasi", "--world", "http"], out.path());
    assert!(!res.status.success(), "{label} must fail on --world http");
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(err.contains("E_WASI_HTTP_CAPABILITY"), "{label}: {err}");
    assert!(!out.path().join("program.wasm").exists(), "{label} must not write program.wasm");
  }
}

#[test]
fn http_proxy_supports_println_and_eprintln_logging_and_live_wasmtime_serve() {
  let dir = tempfile::tempdir().expect("stdio proxy directory");
  let snapshot = dir.path().join("stdio-proxy.cirru");
  fs::copy("tests/fixtures/wasi-http-proxy.cirru", &snapshot).expect("copy HTTP proxy fixture");
  edit_def(
    &snapshot,
    "app.main/handle",
    concat!(
      "quote $ defn handle (req)\n",
      "  do (println |http-stdout (:path req)) (eprintln |http-stderr (:path req)) (Result :ok (plain-response 200 |logged))",
    ),
    "quote $ Fn HttpRequest (-> (Result HttpResponse String)) (:features :io)",
  );

  let emit_dir = tempfile::tempdir().expect("stdio proxy emit dir");
  let built = calcit(&[snapshot.to_str().unwrap(), "wasi", "--world", "http"], emit_dir.path());
  assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));

  let wasm_path = emit_dir.path().join("program.wasm");
  let wasm_bytes = fs::read(&wasm_path).expect("read stdio proxy Component");
  let config = Config::new();
  let engine = Engine::new(&config).expect("Wasmtime engine");
  let component = Component::new(&engine, &wasm_bytes).expect("valid WASI HTTP stdio Component");
  let component_type = component.component_type();
  let imports = component_type.imports(&engine).map(|(n, _)| n).collect::<Vec<_>>();
  assert!(imports.contains(&"wasi:cli/stdout@0.2.12"), "imports: {imports:?}");
  assert!(imports.contains(&"wasi:cli/stderr@0.2.12"), "imports: {imports:?}");

  let Some(wasmtime_cli) = find_wasmtime_cli() else {
    return;
  };
  let addr = free_loopback_addr();
  let mut guard = ChildGuard(
    Command::new(wasmtime_cli)
      .args(["serve", "--addr", &addr.to_string(), "--max-instance-reuse-count", "8"])
      .arg(&wasm_path)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .expect("spawn wasmtime serve for stdio proxy"),
  );
  let reply = send_http(&mut guard.0, addr, "GET", "/ping", &[], b"");
  assert_eq!(reply.status, 200);
  assert_eq!(String::from_utf8_lossy(&reply.body), "logged");
  let (stdout, stderr) = guard.kill_and_collect_output();
  assert!(stdout.contains("http-stdout /ping"), "stdout: {stdout:?}");
  assert!(stderr.contains("http-stderr /ping"), "stderr: {stderr:?}");
}

#[test]
fn http_proxy_serves_routes_binary_errors_and_64_stress_turns_under_8mib_cap() {
  let emit_dir = tempfile::tempdir().expect("proxy emit dir");
  let built = calcit(
    &["tests/fixtures/wasi-http-proxy.cirru", "wasi", "--world", "http"],
    emit_dir.path(),
  );
  assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
  let wasm_path = emit_dir.path().join("program.wasm");

  let Some(wasmtime_cli) = find_wasmtime_cli() else {
    return;
  };

  let addr = free_loopback_addr();
  let mut guard = ChildGuard(
    Command::new(wasmtime_cli)
      .args([
        "serve",
        "--addr",
        &addr.to_string(),
        "--max-instance-reuse-count",
        "128",
        "-W",
        "max-memory-size=8388608",
      ])
      .arg(&wasm_path)
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()
      .expect("spawn wasmtime serve with 8 MiB memory limit"),
  );

  // 1. /health -> persistent *hits = 1
  let r1 = send_http(&mut guard.0, addr, "GET", "/health", &[], b"");
  assert_eq!(r1.status, 200);
  assert_eq!(String::from_utf8_lossy(&r1.body), "ok:1");

  // 2. /headers -> echoes header in both x-echo response header and body
  let r2 = send_http(&mut guard.0, addr, "GET", "/headers", &[("x-test", b"hello-header")], b"");
  assert_eq!(r2.status, 200);
  assert_eq!(String::from_utf8_lossy(&r2.body), "hello-header");
  assert!(
    r2.headers.iter().any(|(k, v)| k == "x-echo" && v == "hello-header"),
    "headers: {:?}",
    r2.headers
  );

  // 2b. Non-UTF-8 HTTP header value -> 400
  let mut bad_hdr_req = Vec::new();
  write!(
    bad_hdr_req,
    "GET /headers HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nx-test: "
  )
  .unwrap();
  bad_hdr_req.extend_from_slice(&[0xff, 0xfe]);
  bad_hdr_req.extend_from_slice(b"\r\n\r\n");
  let r_bad_hdr = send_raw_http(&mut guard.0, addr, &bad_hdr_req);
  assert_eq!(r_bad_hdr.status, 400);

  // 3. /json -> JSON decode + encode + persistent *hits = 2
  let r3 = send_http(
    &mut guard.0,
    addr,
    "POST",
    "/json",
    &[("content-type", b"application/json")],
    br#"{"name":"calcit","count":7}"#,
  );
  assert_eq!(r3.status, 200);
  let json_reply: Value = serde_json::from_slice(&r3.body).expect("JSON response payload");
  assert_eq!(json_reply["ok"], Value::Bool(true));
  assert_eq!(json_reply["greeting"], Value::String("hi-calcit#7".into()));
  assert_eq!(json_reply["hits"], serde_json::json!(2));

  // 4. /edn -> Cirru EDN decode + format + persistent *hits = 3
  let r4 = send_http(
    &mut guard.0,
    addr,
    "POST",
    "/edn",
    &[("content-type", b"application/edn")],
    b"%{} 'Payload (:name |edn) (:count 9)",
  );
  assert_eq!(r4.status, 200);
  let edn_body = String::from_utf8_lossy(&r4.body);
  assert!(edn_body.contains(":greeting |hi-edn#9"), "{edn_body:?}");
  assert!(edn_body.contains(":hits 3"), "{edn_body:?}");

  // 5. /echo-bytes -> 12 KiB non-UTF-8 binary payload byte-exact roundtrip (uses HttpBody :bytes Buffer)
  let binary_payload: Vec<u8> = (0..12288usize).map(|i| ((i * 37 + 0x80) & 0xff) as u8).collect();
  assert!(std::str::from_utf8(&binary_payload).is_err());
  let r5 = send_http(
    &mut guard.0,
    addr,
    "POST",
    "/echo-bytes",
    &[("content-type", b"application/octet-stream")],
    &binary_payload,
  );
  assert_eq!(r5.status, 200);
  assert_eq!(r5.body, binary_payload, "binary payload must round-trip without UTF-8 loss");

  // 6. /fail -> Result :err mapped to HTTP 500 with error text
  let r6 = send_http(&mut guard.0, addr, "GET", "/fail", &[], b"");
  assert_eq!(r6.status, 500);
  assert_eq!(String::from_utf8_lossy(&r6.body), "boom-from-handler");

  // 7. /missing -> HTTP 404
  let r7 = send_http(&mut guard.0, addr, "GET", "/missing", &[], b"");
  assert_eq!(r7.status, 404);
  assert_eq!(String::from_utf8_lossy(&r7.body), "not-found");

  // 8. >4 MiB POST body -> HTTP 413 Request Entity Too Large before handler crash/OOM
  let oversized = vec![b'x'; 4 * 1024 * 1024 + 512];
  let r8 = send_http(
    &mut guard.0,
    addr,
    "POST",
    "/echo-bytes",
    &[("content-type", b"application/octet-stream")],
    &oversized,
  );
  assert_eq!(r8.status, 413, "body: {:?}", String::from_utf8_lossy(&r8.body));
  assert!(
    String::from_utf8_lossy(&r8.body).contains("4 MiB"),
    "413 body: {:?}",
    String::from_utf8_lossy(&r8.body)
  );

  // 9. 64 sequential /alloc-stress turns (>256 KiB each, ~17 MiB cumulative) under 8 MiB hard cap
  //    must reset Scratch Arena per turn while retaining monotonically increasing persistent *hits.
  let mut expected_hits = 4u64;
  for step in 0..64 {
    let stress = send_http(&mut guard.0, addr, "GET", "/alloc-stress", &[], b"");
    assert_eq!(
      stress.status,
      200,
      "stress step {step} failed: {}",
      String::from_utf8_lossy(&stress.body)
    );
    let text = String::from_utf8_lossy(&stress.body);
    let prefix = format!("stress:{expected_hits}:");
    assert!(text.starts_with(&prefix), "step {step} expected {prefix}*, got {text:?}");
    expected_hits += 1;
  }

  // Final health check confirms instance reuse remained alive across all 64 stress turns.
  let final_health = send_http(&mut guard.0, addr, "GET", "/health", &[], b"");
  assert_eq!(final_health.status, 200);
  assert_eq!(String::from_utf8_lossy(&final_health.body), format!("ok:{expected_hits}"));
}

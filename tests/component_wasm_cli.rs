use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use calcit_bindgen::COMPONENT_FILE;
use calcit_bindgen::wasmtime_http::WasiHttpConfig;
use wasmtime::component::{Component, Destination, Linker, StreamProducer, StreamReader, StreamResult, Val, VecBuffer};
use wasmtime::{Config, Engine, Store, StoreContextMut};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-component-wasm-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary output directory should create");
    Self(path)
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn serve_http_response(mut stream: TcpStream) {
  let mut reader = BufReader::new(stream.try_clone().expect("HTTP stream should clone"));
  let mut request_line = String::new();
  reader.read_line(&mut request_line).expect("HTTP request line should read");
  loop {
    let mut header = String::new();
    reader.read_line(&mut header).expect("HTTP request header should read");
    if header == "\r\n" || header.is_empty() {
      break;
    }
  }
  if request_line.split_whitespace().nth(1) == Some("/redirect") {
    write!(
      stream,
      "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:1/denied\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
    .expect("HTTP redirect response should write");
    return;
  }
  let (body, content_type) = match request_line.split_whitespace().nth(1) {
    Some("/ok") => (br#"{"ok":true}"#.as_slice(), "application/json"),
    Some("/large") => (b"123456789".as_slice(), "application/octet-stream"),
    path => panic!("unexpected HTTP path: {path:?}"),
  };
  write!(
    stream,
    "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
    body.len()
  )
  .expect("HTTP response headers should write");
  stream.write_all(body).expect("HTTP response body should write");
}

#[test]
fn async_component_export_returns_once_through_canonical_task_return() {
  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-async-export.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("async Component fixture should compile");
  assert!(
    compile.status.success(),
    "async Component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let wasm = output.0.join("program.wasm");
  let script = r#"
const fs = require("fs");
const bytes = fs.readFileSync(process.argv[1]);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
const importNames = imports.map(({ module, name }) => `${module}/${name}`).sort();
if (importNames.join(",") !== "[export]$root/[task-return]echo-result,[export]$root/[task-return]load-text,[export]$root/[task-return]load-wide") {
  throw new Error(`unexpected imports: ${JSON.stringify(imports)}`);
}
let instance;
let completions = 0;
let returnedText = null;
const resultCompletions = [];
let wideCompletion = null;
let wideReturnPtr = null;
const canonical = {
  "[task-return]load-text": (ptr, len) => {
    completions += 1;
    returnedText = Buffer.from(instance.exports.memory.buffer, ptr, len).toString("utf8");
  },
  "[task-return]echo-result": (discriminant, ptr, len) => {
    resultCompletions.push([discriminant, Buffer.from(instance.exports.memory.buffer, ptr, len).toString("utf8")]);
  },
  "[task-return]load-wide": ptr => {
    const memory = new DataView(instance.exports.memory.buffer);
    wideReturnPtr = ptr;
    wideCompletion = Array.from({ length: 17 }, (_, index) => memory.getFloat64(ptr + index * 8, true));
  },
};
WebAssembly.instantiate(module, { "[export]$root": canonical }).then(result => {
  instance = result;
  const allocateText = text => {
    const input = Buffer.from(text, "utf8");
    const ptr = instance.exports.cabi_realloc(0, 0, 1, input.length);
    new Uint8Array(instance.exports.memory.buffer, ptr, input.length).set(input);
    return [ptr, input.length];
  };
  const [ptr, len] = allocateText("你好 async");
  const returned = instance.exports["[async-lift-stackful]load-text"](ptr, len);
  if (returned !== undefined) throw new Error(`async core export returned ${returned}`);
  if (completions !== 1) throw new Error(`expected one completion, got ${completions}`);
  if (returnedText !== "你好 async") throw new Error(`unexpected result: ${returnedText}`);
  const [okPtr, okLen] = allocateText("ok-value");
  const [errorPtr, errorLen] = allocateText("error-value");
  instance.exports["[async-lift-stackful]echo-result"](0, okPtr, okLen);
  instance.exports["[async-lift-stackful]echo-result"](1, errorPtr, errorLen);
  if (JSON.stringify(resultCompletions) !== JSON.stringify([[0, "ok-value"], [1, "error-value"]])) {
    throw new Error(`typed Result did not round-trip: ${JSON.stringify(resultCompletions)}`);
  }
  const expectedWide = Array.from({ length: 17 }, (_, index) => index);
  instance.exports["[async-lift-stackful]load-wide"]();
  if (JSON.stringify(wideCompletion) !== JSON.stringify(expectedWide)) {
    throw new Error(`indirect async typed Struct did not round-trip: ${JSON.stringify(wideCompletion)}`);
  }
  const reclaimedAsyncWide = instance.exports.cabi_realloc(0, 0, 8, 17 * 8);
  if (reclaimedAsyncWide !== wideReturnPtr) throw new Error("stackful async task.return did not reclaim its result area");
  instance.exports.cabi_realloc(reclaimedAsyncWide, 17 * 8, 8, 0);
  const syncWidePtr = instance.exports["load-wide-sync"]();
  const syncWideMemory = new DataView(instance.exports.memory.buffer);
  const syncWide = Array.from({ length: 17 }, (_, index) => syncWideMemory.getFloat64(syncWidePtr + index * 8, true));
  if (JSON.stringify(syncWide) !== JSON.stringify(expectedWide)) {
    throw new Error(`indirect sync typed Struct did not round-trip: ${JSON.stringify(syncWide)}`);
  }
  instance.exports["cabi_post_load-wide-sync"](syncWidePtr);
  const repeatedSyncWidePtr = instance.exports["load-wide-sync"]();
  if (repeatedSyncWidePtr !== syncWidePtr) {
    throw new Error(`sync post-return did not reuse the indirect return area: ${syncWidePtr} -> ${repeatedSyncWidePtr}`);
  }
  instance.exports["cabi_post_load-wide-sync"](repeatedSyncWidePtr);
}).catch(error => {
  console.error(error);
  process.exitCode = 1;
});
"#;
  let runtime = Command::new("node")
    .args(["-e", script])
    .arg(&wasm)
    .output()
    .expect("Node.js should validate and instantiate the async Component core module");
  assert!(
    runtime.status.success(),
    "async Component runtime failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&runtime.stdout),
    String::from_utf8_lossy(&runtime.stderr)
  );
}

#[test]
fn scoped_stream_export_bounds_reads_and_closes_every_terminal_path() {
  let output = TestDirectory::create();
  let calcit_tests = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-stream.cirru",
      "test",
      "--tag",
      "wasm",
      "--require-match",
      "--summary-only",
    ])
    .output()
    .expect("scoped stream Calcit tests should run");
  assert!(
    calcit_tests.status.success(),
    "scoped stream Calcit tests failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_tests.stdout),
    String::from_utf8_lossy(&calcit_tests.stderr)
  );
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-stream.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("scoped stream Component fixture should compile");
  assert!(
    compile.status.success(),
    "scoped stream Component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let wasm = output.0.join("program.wasm");
  let script = r#"
const fs = require("fs");
const module = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
let instance;
let nextSet = 40;
let currentContext = 0;
const reads = new Map();
const active = new Set();
const pending = new Map();
const drops = new Map();
const liveSets = new Set();
const liveStreams = new Set();
const completions = [];
let cancellations = 0;
let readCancellations = 0;

const write = (ptr, bytes) => new Uint8Array(instance.exports.memory.buffer, ptr, bytes.length).set(bytes);
const startRead = (handle, ptr, len, mode) => {
  if (active.has(handle)) throw new Error(`more than one outstanding read for ${handle}`);
  if (len < 1 || len > 3) throw new Error(`read ${handle} escaped chunk bound: ${len}`);
  const call = (reads.get(handle) || 0) + 1;
  reads.set(handle, call);
  active.add(handle);
  liveStreams.add(handle);
  if (mode === "stop") {
    write(ptr, [1, 2]);
    active.delete(handle);
    return 2 << 4;
  }
  if (handle === 8) {
    const chunks = [[1], [2, 3], [4, 5, 6], [7]];
    write(ptr, chunks[call - 1]);
    active.delete(handle);
    return chunks[call - 1].length << 4;
  }
  if (handle === 7 && call === 2) {
    write(ptr, [2, 3]);
    active.delete(handle);
    return 2 << 4;
  }
  if (handle >= 100) {
    write(ptr, [1]);
    active.delete(handle);
    return (1 << 4) | 1;
  }
  if (handle === 11) {
    active.delete(handle);
    return 0;
  }
  pending.set(handle, { ptr, len });
  return -1;
};
const complete = (handle, bytes, dropped) => {
  const read = pending.get(handle);
  if (!read) throw new Error(`missing pending read ${handle}`);
  if (bytes.length > read.len) throw new Error(`producer exceeded requested read length for ${handle}`);
  write(read.ptr, bytes);
  pending.delete(handle);
  active.delete(handle);
  return (bytes.length << 4) | (dropped ? 1 : 0);
};
const streamDrop = handle => {
  if (active.has(handle)) throw new Error(`dropped stream ${handle} with an outstanding read`);
  if (!liveStreams.delete(handle)) throw new Error(`dropped unknown stream ${handle}`);
  drops.set(handle, (drops.get(handle) || 0) + 1);
};
const canonical = {
  "[waitable-set-new]": () => {
    const handle = nextSet++;
    liveSets.add(handle);
    return handle;
  },
  "[waitable-set-wait]": () => { throw new Error("stream adapter must not block internally"); },
  "[waitable-set-drop]": handle => {
    if (!liveSets.delete(handle)) throw new Error(`dropped unknown waitable set ${handle}`);
  },
  "[waitable-join]": () => {},
  "[subtask-drop]": () => {},
  "[context-get-0]": () => currentContext,
  "[context-set-0]": value => { currentContext = value; },
  "[async-lower][subtask-cancel]": () => { throw new Error("stream cancellation must use stream-cancel-read"); },
  "[task-return]consume": (result, error) => completions.push(["consume", result, error]),
  "[task-return]stop": (result, error) => completions.push(["stop", result, error]),
  "[task-cancel]": () => { cancellations += 1; },
  "[async-lower][stream-read-0]consume": (handle, ptr, len) => startRead(handle, ptr, len, "consume"),
  "[async-lower][stream-cancel-read-0]consume": handle => {
    if (!active.has(handle)) throw new Error(`cancelled stream ${handle} without an outstanding read`);
    readCancellations += 1;
    return -1;
  },
  "[stream-drop-readable-0]consume": streamDrop,
  "[async-lower][stream-read-0]stop": (handle, ptr, len) => startRead(handle, ptr, len, "stop"),
  "[async-lower][stream-cancel-read-0]stop": () => { throw new Error("early stop must not cancel a completed read"); },
  "[stream-drop-readable-0]stop": streamDrop,
};

WebAssembly.instantiate(module, { "$root": canonical, "[export]$root": canonical }).then(result => {
  instance = result;
  const consume = instance.exports["[async-lift]consume"];
  const consumeCallback = instance.exports["[callback][async-lift]consume"];
  const stop = instance.exports["[async-lift]stop"];

  const first = consume(7);
  const firstContext = currentContext;
  if (first !== ((40 << 4) | 2)) throw new Error(`slow producer did not yield: ${first}`);
  const firstResume = consumeCallback(2, 7, complete(7, [1], false));
  if (firstResume !== first) throw new Error(`second slow read did not preserve wait set: ${firstResume}`);
  if (currentContext !== firstContext) throw new Error("stream callback replaced its context");
  if (consumeCallback(2, 7, complete(7, [4, 5, 6], true)) !== 0) throw new Error("EOF did not exit");

  if (consume(8) !== 0) throw new Error("total-limit path did not finish immediately");

  const cancelled = consume(9);
  if (cancelled !== ((42 << 4) | 2)) throw new Error(`cancel task did not yield: ${cancelled}`);
  if (consumeCallback(6, 0, 0) !== cancelled) throw new Error("blocked read cancellation did not keep waiting");
  active.delete(9);
  pending.delete(9);
  if (consumeCallback(2, 9, 2) !== 0) throw new Error("cancelled read did not exit");

  if (stop(10) !== 0) throw new Error("handler early stop did not exit");

  const expectedCompletions = [["consume", 0, 0], ["consume", 1, 0], ["stop", 0, 0]];
  if (JSON.stringify(completions) !== JSON.stringify(expectedCompletions)) {
    throw new Error(`unexpected stream completions: ${JSON.stringify(completions)}`);
  }
  if (cancellations !== 1) throw new Error(`expected one task cancellation, got ${cancellations}`);
  if (readCancellations !== 1) throw new Error(`expected one read cancellation, got ${readCancellations}`);
  for (const handle of [7, 8, 9, 10]) {
    if (drops.get(handle) !== 1) throw new Error(`stream ${handle} dropped ${drops.get(handle)} times`);
  }
  if (JSON.stringify([...reads]) !== JSON.stringify([[7, 3], [8, 4], [9, 1], [10, 1]])) {
    throw new Error(`unexpected read counts: ${JSON.stringify([...reads])}`);
  }
  if (consume(11) !== 0) throw new Error("zero-count completed read did not terminate");
  if (reads.get(11) !== 1 || drops.get(11) !== 1) throw new Error("zero-count completed read retried or leaked its stream");
  if (liveSets.size || liveStreams.size || active.size || pending.size) {
    throw new Error("terminal stream paths leaked lifecycle handles");
  }

  if (consume(100) !== 0) throw new Error("warm-up stream did not complete");
  const warmPages = instance.exports.memory.buffer.byteLength;
  const probe = instance.exports.cabi_realloc(0, 0, 4, 64);
  instance.exports.cabi_realloc(probe, 64, 4, 0);
  for (let handle = 101; handle < 111; handle += 1) {
    if (consume(handle) !== 0) throw new Error(`repeated stream ${handle} did not complete`);
    const repeatedProbe = instance.exports.cabi_realloc(0, 0, 4, 64);
    if (repeatedProbe !== probe) throw new Error(`heap high-water changed after warm-up: ${probe} -> ${repeatedProbe}`);
    instance.exports.cabi_realloc(repeatedProbe, 64, 4, 0);
    if (instance.exports.memory.buffer.byteLength !== warmPages) throw new Error("repeated streams grew linear memory after warm-up");
    if (liveSets.size || liveStreams.size || active.size || pending.size) {
      throw new Error(`repeated stream ${handle} leaked lifecycle handles`);
    }
  }
}).catch(error => {
  console.error(error);
  process.exitCode = 1;
});
"#;
  let runtime = Command::new("node")
    .args(["-e", script])
    .arg(&wasm)
    .output()
    .expect("Node.js should exercise the scoped stream core adapter");
  assert!(
    runtime.status.success(),
    "scoped stream Component runtime failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&runtime.stdout),
    String::from_utf8_lossy(&runtime.stderr)
  );
}

struct DelayedChunks {
  chunks: VecDeque<Vec<u8>>,
  waiting_for_chunk: bool,
}

impl StreamProducer<()> for DelayedChunks {
  type Item = u8;
  type Buffer = VecBuffer<u8>;

  fn poll_produce<'a>(
    mut self: Pin<&mut Self>,
    context: &mut Context<'_>,
    _store: StoreContextMut<'a, ()>,
    mut destination: Destination<'a, Self::Item, Self::Buffer>,
    finish: bool,
  ) -> Poll<wasmtime::Result<StreamResult>> {
    if finish {
      return Poll::Ready(Ok(StreamResult::Cancelled));
    }
    if !self.waiting_for_chunk {
      self.waiting_for_chunk = true;
      let waker = context.waker().clone();
      thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        waker.wake();
      });
      return Poll::Pending;
    }
    self.waiting_for_chunk = false;
    let chunk = self.chunks.pop_front().expect("delayed stream should retain a chunk after waking");
    destination.set_buffer(chunk.into());
    Poll::Ready(Ok(if self.chunks.is_empty() {
      StreamResult::Dropped
    } else {
      StreamResult::Completed
    }))
  }
}

#[tokio::test]
async fn packaged_scoped_stream_validates_three_chunks_and_repeats_after_backpressure() {
  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-stream.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("scoped stream Component fixture should compile");
  assert!(
    compile.status.success(),
    "scoped stream compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );
  let contract = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-stream.cirru",
      "ffi",
      "export",
      "--boundary",
      "component",
    ])
    .output()
    .expect("scoped stream contract should export");
  assert!(
    contract.status.success(),
    "scoped stream contract export failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&contract.stdout),
    String::from_utf8_lossy(&contract.stderr)
  );
  let contract_path = output.0.join("interface.cirru");
  fs::write(&contract_path, contract.stdout).expect("scoped stream contract should write");
  let contract = calcit_bindgen::load_contract(&contract_path).expect("scoped stream contract should load");
  let generated = output.0.join("generated");
  calcit_bindgen::generate_contract_directory(&contract, Some(&output.0.join("program.wasm")), &generated, &[])
    .expect("scoped stream Component should package");

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_more_async_builtins(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime Component engine should create");
  let component = Component::from_file(&engine, generated.join(COMPONENT_FILE)).expect("scoped stream Component should load");
  let linker = Linker::new(&engine);
  let mut store = Store::new(&engine, ());
  let instance = linker
    .instantiate_async(&mut store, &component)
    .await
    .expect("scoped stream Component should instantiate");
  let consume = instance.get_func(&mut store, "consume").expect("scoped stream export should exist");
  for invocation in 0..6 {
    let reader = StreamReader::new(
      &mut store,
      DelayedChunks {
        chunks: VecDeque::from([vec![1], vec![2, 3], vec![4, 5, 6]]),
        waiting_for_chunk: false,
      },
    )
    .expect("host byte stream should create");
    let stream = reader
      .try_into_stream_any(&mut store)
      .expect("typed host byte stream should erase to the dynamic Component value");
    let mut results = [Val::Bool(false)];
    tokio::time::timeout(
      Duration::from_secs(3),
      consume.call_async(&mut store, &[Val::Stream(stream)], &mut results),
    )
    .await
    .expect("scoped stream should resume after host backpressure")
    .expect("scoped stream call should complete");
    assert!(
      matches!(&results[0], Val::Result(Ok(None))),
      "unexpected scoped stream result on invocation {invocation}: {results:?}"
    );
    store.assert_concurrent_state_empty();
  }
}

#[tokio::test]
async fn packaged_component_invokes_sync_post_return_in_wasmtime() {
  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-async-export.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("post-return Component fixture should compile");
  assert!(
    compile.status.success(),
    "post-return Component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );
  let contract = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-async-export.cirru",
      "ffi",
      "export",
      "--boundary",
      "component",
    ])
    .output()
    .expect("post-return Component contract should export");
  assert!(
    contract.status.success(),
    "post-return contract export failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&contract.stdout),
    String::from_utf8_lossy(&contract.stderr)
  );
  let contract_path = output.0.join("interface.cirru");
  fs::write(&contract_path, contract.stdout).expect("post-return contract should write");
  let contract = calcit_bindgen::load_contract(&contract_path).expect("post-return contract should load");
  let generated = output.0.join("generated");
  calcit_bindgen::generate_contract_directory(&contract, Some(&output.0.join("program.wasm")), &generated, &[])
    .expect("post-return Component should package");

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_more_async_builtins(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime Component engine should create");
  let component = Component::from_file(&engine, generated.join(COMPONENT_FILE)).expect("post-return Component should load");
  let linker = Linker::new(&engine);
  let mut store = Store::new(&engine, ());
  let instance = linker
    .instantiate_async(&mut store, &component)
    .await
    .expect("post-return Component should instantiate");
  let load_wide = instance
    .get_func(&mut store, "load-wide-sync")
    .expect("sync wide export should exist");
  for _ in 0..3 {
    let mut result = [Val::Bool(false)];
    load_wide
      .call_async(&mut store, &[], &mut result)
      .await
      .expect("Wasmtime should lift the result and invoke post-return");
    assert!(matches!(&result[0], Val::Record(fields) if fields.len() == 17));
  }
}

#[test]
fn async_component_import_waits_for_subtask_and_drops_lifecycle_handles() {
  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-async-import.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("async Component import fixture should compile");
  assert!(
    compile.status.success(),
    "async Component import compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let wasm = output.0.join("program.wasm");
  let script = r#"
const fs = require("fs");
const bytes = fs.readFileSync(process.argv[1]);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module).map(({ module, name }) => `${module}/${name}`).sort();
const expectedImports = [
  "host/[async-lower]flag",
  "host/[async-lower]combine",
  "host/[async-lower]load",
  "calcit:wasi-http/client/[async-lower]request",
  "$root/[subtask-drop]",
  "$root/[async-lower][subtask-cancel]",
  "$root/[context-get-0]",
  "$root/[context-set-0]",
  "[export]$root/[task-return]call-host-combine",
  "[export]$root/[task-return]call-host-flag",
  "[export]$root/[task-return]call-host-http-request",
  "[export]$root/[task-return]call-host-load",
  "[export]$root/[task-cancel]",
  "$root/[waitable-set-drop]",
  "$root/[waitable-set-new]",
  "$root/[waitable-set-wait]",
  "$root/[waitable-join]",
].sort();
if (JSON.stringify(imports) !== JSON.stringify(expectedImports)) {
  throw new Error(`unexpected imports: ${JSON.stringify(imports)}`);
}

let instance;
let nextWaitableSet = 40;
let completions = [];
let boolCompletions = [];
let combineCompletions = [];
let lifecycle = {
  new: 0,
  join: 0,
  wait: 0,
  subtaskDrop: 0,
  setDrop: 0,
  contextGet: 0,
  contextSet: 0,
  subtaskCancel: 0,
  taskCancel: 0,
};
const pending = new Map();
const joined = new Map();
let currentContext = 0;

const allocateText = text => {
  const bytes = Buffer.from(text, "utf8");
  const ptr = instance.exports.cabi_realloc(0, 0, 1, bytes.length);
  new Uint8Array(instance.exports.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
};
const readText = (ptr, len) => Buffer.from(instance.exports.memory.buffer, ptr, len).toString("utf8");
const writeResult = (outPtr, discriminant, text) => {
  const [ptr, len] = allocateText(text);
  const memory = new DataView(instance.exports.memory.buffer);
  memory.setUint8(outPtr, discriminant);
  memory.setUint32(outPtr + 4, ptr, true);
  memory.setUint32(outPtr + 8, len, true);
};

const host = {
  "[async-lower]combine": (argsPtr, outPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    const left = readText(memory.getUint32(argsPtr, true), memory.getUint32(argsPtr + 4, true));
    const right = readText(memory.getUint32(argsPtr + 8, true), memory.getUint32(argsPtr + 12, true));
    const count = memory.getFloat64(argsPtr + 16, true);
    const enabled = memory.getUint8(argsPtr + 24) === 1;
    const [ptr, len] = allocateText(`${left}:${right}:${count}:${enabled}`);
    memory.setUint32(outPtr, ptr, true);
    memory.setUint32(outPtr + 4, len, true);
    return 2;
  },
  "[async-lower]flag": (flag, outPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(outPtr, flag === 0 ? 1 : 0);
    memory.setUint8(outPtr + 1, 0xff);
    memory.setUint8(outPtr + 2, 0xff);
    memory.setUint8(outPtr + 3, 0xff);
    return 2;
  },
  "[async-lower]load": (inputPtr, inputLen, outPtr) => {
    const marker = readText(inputPtr, inputLen);
    if (marker === "immediate-ok") {
      writeResult(outPtr, 0, "ready-now");
      return 2;
    }
    if (marker === "delayed-ok") {
      pending.set(7, { inputPtr, inputLen, outPtr, discriminant: 0, text: "ready-later" });
      return (7 << 4) | 0;
    }
    if (marker === "delayed-error") {
      pending.set(8, { inputPtr, inputLen, outPtr, discriminant: 1, text: "typed-error" });
      return (8 << 4) | 1;
    }
    if (marker === "cancelled") {
      pending.set(9, { inputPtr, inputLen, outPtr, discriminant: 1, text: "unused", cancelBlocked: true });
      return (9 << 4) | 1;
    }
    throw new Error(`unexpected host input ${marker}`);
  },
};
const http = {
  "[async-lower]request": () => {
    throw new Error("HTTP request adapter should not run in the generic async lifecycle smoke");
  },
};
const canonical = {
  "[task-return]call-host-combine": (ptr, len) => {
    combineCompletions.push(readText(ptr, len));
  },
  "[task-return]call-host-flag": flag => {
    boolCompletions.push(flag);
  },
  "[task-return]call-host-http-request": () => {
    throw new Error("HTTP request completion should not run in the generic async lifecycle smoke");
  },
  "[task-return]call-host-load": (discriminant, ptr, len) => {
    completions.push([discriminant, readText(ptr, len)]);
  },
  "[waitable-set-new]": () => {
    lifecycle.new += 1;
    return nextWaitableSet++;
  },
  "[waitable-join]": (subtask, set) => {
    lifecycle.join += 1;
    for (const [currentSet, currentSubtask] of joined) {
      if (currentSubtask === subtask) joined.delete(currentSet);
    }
    if (set !== 0) joined.set(set, subtask);
  },
  "[waitable-set-wait]": (set, eventPtr) => {
    lifecycle.wait += 1;
    throw new Error(`stackless callback export must not block on waitable set ${set} at ${eventPtr}`);
  },
  "[subtask-drop]": subtask => {
    lifecycle.subtaskDrop += 1;
    pending.delete(subtask);
  },
  "[waitable-set-drop]": set => {
    lifecycle.setDrop += 1;
    joined.delete(set);
  },
  "[context-get-0]": () => {
    lifecycle.contextGet += 1;
    return currentContext;
  },
  "[context-set-0]": context => {
    lifecycle.contextSet += 1;
    currentContext = context;
  },
  "[async-lower][subtask-cancel]": subtask => {
    lifecycle.subtaskCancel += 1;
    const task = pending.get(subtask);
    if (!task) throw new Error(`missing cancelled subtask ${subtask}`);
    if (task.cancelBlocked) {
      task.cancelBlocked = false;
      return -1;
    }
    return 4;
  },
  "[task-cancel]": () => {
    lifecycle.taskCancel += 1;
  },
};

WebAssembly.instantiate(module, { host, "calcit:wasi-http/client": http, "$root": canonical, "[export]$root": canonical }).then(result => {
  instance = result;
  const heapBeforeAllocatorProbe = instance.exports.__heap_ptr.value;
  const probe = instance.exports.cabi_realloc(0, 0, 8, 24);
  const heapAfterFirstProbe = instance.exports.__heap_ptr.value;
  instance.exports.cabi_realloc(probe, 24, 8, 0);
  const reusedProbe = instance.exports.cabi_realloc(0, 0, 8, 24);
  if (reusedProbe !== probe || instance.exports.__heap_ptr.value !== heapAfterFirstProbe) {
    throw new Error(`cabi_realloc did not reuse a released block: ${probe}, ${reusedProbe}`);
  }
  if (heapAfterFirstProbe <= heapBeforeAllocatorProbe) throw new Error("allocator probe did not reserve memory");
  instance.exports.cabi_realloc(reusedProbe, 24, 8, 0);
  const start = marker => {
    const [ptr, len] = allocateText(marker);
    const status = instance.exports["[async-lift]call-host-load"](ptr, len);
    return { status, context: currentContext, inputPtr: ptr, inputLen: len };
  };
  const callback = instance.exports["[callback][async-lift]call-host-load"];
  const resume = (task, subtask, state) => {
    currentContext = task.context;
    const pendingTask = pending.get(subtask);
    if (state === 1) readText(pendingTask.inputPtr, pendingTask.inputLen);
    if (state === 2) writeResult(pendingTask.outPtr, pendingTask.discriminant, pendingTask.text);
    const status = callback(1, subtask, state);
    if ((state === 2 || state === 4) && status === 0) {
      instance.exports.cabi_realloc(task.inputPtr, task.inputLen, 1, 0);
    }
    return status;
  };

  const immediate = start("immediate-ok");
  if (immediate.status !== 0) throw new Error("immediate stackless export did not exit");
  instance.exports.cabi_realloc(immediate.inputPtr, immediate.inputLen, 1, 0);
  const delayedOk = start("delayed-ok");
  const delayedError = start("delayed-error");
  if (delayedOk.context === delayedError.context) throw new Error("stackless tasks reused one context record");
  if (delayedOk.status !== ((40 << 4) | 2) || delayedError.status !== ((41 << 4) | 2)) {
    throw new Error(`unexpected wait statuses: ${delayedOk.status}, ${delayedError.status}`);
  }
  if (resume(delayedOk, 7, 0) !== delayedOk.status) throw new Error("started subtask did not keep waiting");
  if (resume(delayedOk, 7, 1) !== delayedOk.status) throw new Error("returned subtask did not keep waiting");
  if (resume(delayedError, 8, 2) !== 0) throw new Error("error subtask did not exit");
  if (resume(delayedOk, 7, 2) !== 0) throw new Error("successful subtask did not exit");
  if (JSON.stringify(completions) !== JSON.stringify([[0, "ready-now"], [1, "typed-error"], [0, "ready-later"]])) {
    throw new Error(`unexpected completions: ${JSON.stringify(completions)}`);
  }
  instance.exports["[async-lift]call-host-flag"](1);
  instance.exports["[async-lift]call-host-flag"](0);
  if (JSON.stringify(boolCompletions) !== JSON.stringify([0, 1])) {
    throw new Error(`async Bool result read beyond one byte: ${JSON.stringify(boolCompletions)}`);
  }
  const [leftPtr, leftLen] = allocateText("left");
  const [rightPtr, rightLen] = allocateText("right");
  instance.exports["[async-lift]call-host-combine"](leftPtr, leftLen, rightPtr, rightLen, 3, 1);
  if (JSON.stringify(combineCompletions) !== JSON.stringify(["left:right:3:true"])) {
    throw new Error(`indirect async parameters did not round-trip: ${JSON.stringify(combineCompletions)}`);
  }
  const cancelled = start("cancelled");
  currentContext = cancelled.context;
  if (callback(6, 0, 0) !== cancelled.status) throw new Error("blocked child cancellation did not resume waiting");
  if (resume(cancelled, 9, 4) !== 0) throw new Error("cancelled child did not exit");
  const expectedLifecycle = {
    new: 3,
    join: 8,
    wait: 0,
    subtaskDrop: 3,
    setDrop: 3,
    contextGet: 6,
    contextSet: 6,
    subtaskCancel: 1,
    taskCancel: 1,
  };
  if (JSON.stringify(lifecycle) !== JSON.stringify(expectedLifecycle)) {
    throw new Error(`unexpected lifecycle: ${JSON.stringify(lifecycle)}`);
  }
  for (const [marker, subtask] of [["delayed-ok", 7], ["delayed-error", 8]]) {
    const task = start(marker);
    if (resume(task, subtask, 2) !== 0) throw new Error(`warm-up task ${marker} did not exit`);
  }
  const heapBeforeReuse = instance.exports.__heap_ptr.value;
  for (let index = 0; index < 32; index += 1) {
    const task = start(index % 2 === 0 ? "delayed-ok" : "delayed-error");
    const subtask = index % 2 === 0 ? 7 : 8;
    if (resume(task, subtask, 2) !== 0) throw new Error(`repeated task ${index} did not exit`);
  }
  if (instance.exports.__heap_ptr.value !== heapBeforeReuse) {
    throw new Error(`repeated post-return calls grew the heap: ${heapBeforeReuse} -> ${instance.exports.__heap_ptr.value}`);
  }
}).catch(error => {
  console.error(error);
  process.exitCode = 1;
});
"#;
  let runtime = Command::new("node")
    .args(["-e", script])
    .arg(&wasm)
    .output()
    .expect("Node.js should validate and instantiate the async Component import core module");
  assert!(
    runtime.status.success(),
    "async Component import runtime failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&runtime.stdout),
    String::from_utf8_lossy(&runtime.stderr)
  );
}

#[test]
fn buffered_http_component_uses_real_network_capabilities_and_bounds_in_js() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("local HTTP fixture should bind");
  let origin = format!("http://{}", listener.local_addr().expect("local HTTP address should resolve"));
  let server = thread::spawn(move || {
    for stream in listener.incoming().take(3) {
      serve_http_response(stream.expect("local HTTP connection should accept"));
    }
  });

  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm-async-import.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("buffered HTTP Component fixture should compile");
  assert!(
    compile.status.success(),
    "buffered HTTP Component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let wasm = output.0.join("program.wasm");
  let runtime = Command::new("node")
    .arg("tests/fixtures/component-wasm-http-host.js")
    .arg(&wasm)
    .arg(&origin)
    .output()
    .expect("Node.js should execute the bounded HTTP Component adapter");
  assert!(
    runtime.status.success(),
    "bounded HTTP Component runtime failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&runtime.stdout),
    String::from_utf8_lossy(&runtime.stderr)
  );
  server.join().expect("local HTTP fixture should finish");
}

fn http_request(url: String, max_response_bytes: u64) -> Val {
  Val::Record(vec![
    ("body".to_owned(), Val::Variant("empty".to_owned(), None)),
    ("headers".to_owned(), Val::List(vec![])),
    ("max-response-bytes".to_owned(), Val::U64(max_response_bytes)),
    ("method".to_owned(), Val::Variant("get".to_owned(), None)),
    ("url".to_owned(), Val::String(url)),
  ])
}

#[tokio::test]
async fn packaged_http_component_executes_through_the_wasmtime_host_adapter() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("local HTTP fixture should bind");
  let origin = format!("http://{}", listener.local_addr().expect("local HTTP address should resolve"));
  let server = thread::spawn(move || {
    for stream in listener.incoming().take(3) {
      serve_http_response(stream.expect("local HTTP connection should accept"));
    }
  });

  let output = TestDirectory::create();
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "examples/wasi-http-client/calcit.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("buffered HTTP Component fixture should compile");
  assert!(
    compile.status.success(),
    "buffered HTTP Component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );
  let contract = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "examples/wasi-http-client/calcit.cirru",
      "ffi",
      "export",
      "--boundary",
      "component",
    ])
    .output()
    .expect("buffered HTTP Component contract should export");
  assert!(
    contract.status.success(),
    "buffered HTTP contract export failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&contract.stdout),
    String::from_utf8_lossy(&contract.stderr)
  );
  let contract_path = output.0.join("interface.cirru");
  fs::write(&contract_path, contract.stdout).expect("buffered HTTP contract should write");
  let contract = calcit_bindgen::load_contract(&contract_path).expect("buffered HTTP contract should load");
  let generated = output.0.join("generated");
  calcit_bindgen::generate_contract_directory(&contract, Some(&output.0.join("program.wasm")), &generated, &[])
    .expect("buffered HTTP Component should package");

  let mut config = Config::new();
  config.wasm_component_model_async(true);
  config.wasm_component_model_more_async_builtins(true);
  config.wasm_component_model_async_stackful(true);
  let engine = Engine::new(&config).expect("Wasmtime Component engine should create");
  let component = Component::from_file(&engine, generated.join(COMPONENT_FILE)).expect("buffered HTTP Component should load");
  let mut linker = Linker::new(&engine);
  let mut fixture_host = linker.instance("host").expect("fixture host instance should define");
  fixture_host
    .func_wrap_concurrent(
      "combine",
      |_accessor, (left, right, count, enabled): (String, String, f64, bool)| {
        Box::pin(async move { Ok((format!("{left}:{right}:{count}:{enabled}"),)) })
      },
    )
    .expect("fixture combine import should define");
  fixture_host
    .func_wrap_concurrent("flag", |_accessor, (flag,): (bool,)| Box::pin(async move { Ok((!flag,)) }))
    .expect("fixture flag import should define");
  fixture_host
    .func_wrap_concurrent("load", |_accessor, (text,): (String,)| {
      Box::pin(async move { Ok((Ok::<String, String>(text),)) })
    })
    .expect("fixture load import should define");
  let host = WasiHttpConfig::default()
    .allow_origin(&origin)
    .expect("local HTTP origin should be granted");
  calcit_bindgen::wasmtime_http::add_to_linker(&mut linker, host).expect("buffered HTTP host adapter should link");
  let mut store = Store::new(&engine, ());
  let instance = linker
    .instantiate_async(&mut store, &component)
    .await
    .expect("buffered HTTP Component should instantiate");
  let request = instance
    .get_func(&mut store, "call-host-http-request")
    .expect("buffered HTTP export should exist");

  let mut success = [Val::Bool(false)];
  request
    .call_async(&mut store, &[http_request(format!("{origin}/ok"), 64)], &mut success)
    .await
    .expect("buffered HTTP success should return");
  let Val::Result(Ok(Some(response))) = &success[0] else {
    panic!("unexpected Wasmtime HTTP success: {success:?}");
  };
  let Val::Record(fields) = response.as_ref() else {
    panic!("unexpected Wasmtime HTTP response: {response:?}");
  };
  assert!(
    matches!(&fields[0], (name, Val::Variant(case, Some(body)))
      if name == "body" && case == "text" && matches!(body.as_ref(), Val::String(text) if text == r#"{"ok":true}"#)),
    "unexpected Wasmtime HTTP body: {:?}",
    fields[0]
  );
  assert!(
    matches!(&fields[1], (name, Val::List(headers)) if name == "headers" && headers.iter().any(|header|
      matches!(header, Val::Record(values)
        if matches!(&values[0], (field, Val::String(value)) if field == "name" && value == "content-type")))),
    "unexpected Wasmtime HTTP headers: {:?}",
    fields[1]
  );
  assert!(matches!(&fields[2], (name, Val::U16(200)) if name == "status"));

  let mut malformed = [Val::Bool(false)];
  request
    .call_async(&mut store, &[http_request("::malformed-url".to_owned(), 64)], &mut malformed)
    .await
    .expect("malformed URL should return a typed error");
  assert!(
    matches!(
      &malformed[0],
      Val::Result(Err(Some(error)))
        if matches!(error.as_ref(), Val::Variant(case, Some(_)) if case == "invalid-request")
    ),
    "unexpected Wasmtime malformed URL error: {malformed:?}"
  );

  let mut denied = [Val::Bool(false)];
  request
    .call_async(&mut store, &[http_request("http://127.0.0.1:1/denied".to_owned(), 64)], &mut denied)
    .await
    .expect("capability denial should return a typed error");
  assert!(
    matches!(
      &denied[0],
      Val::Result(Err(Some(error)))
        if matches!(error.as_ref(), Val::Variant(case, Some(_)) if case == "capability-denied")
    ),
    "unexpected Wasmtime capability error: {denied:?}"
  );

  let mut redirected = [Val::Bool(false)];
  request
    .call_async(&mut store, &[http_request(format!("{origin}/redirect"), 64)], &mut redirected)
    .await
    .expect("redirect response should return without being followed");
  let Val::Result(Ok(Some(response))) = &redirected[0] else {
    panic!("unexpected Wasmtime redirect result: {redirected:?}");
  };
  let Val::Record(fields) = response.as_ref() else {
    panic!("unexpected Wasmtime redirect response: {response:?}");
  };
  assert!(matches!(&fields[2], (name, Val::U16(302)) if name == "status"));
  assert!(
    matches!(&fields[1], (name, Val::List(headers)) if name == "headers" && headers.iter().any(|header|
    matches!(header, Val::Record(values)
      if matches!(
          values.as_slice(),
          [
            (name, Val::String(header_name)),
            (value, Val::List(header_value)),
          ] if name == "name"
            && header_name == "location"
            && value == "value"
            && header_value.len() == b"http://127.0.0.1:1/denied".len()
            && header_value
              .iter()
              .zip(b"http://127.0.0.1:1/denied")
              .all(|(actual, expected)| matches!(actual, Val::U8(value) if value == expected))
      )))),
    "redirect response should preserve the location header: {:?}",
    fields[1]
  );

  let mut oversized = [Val::Bool(false)];
  request
    .call_async(&mut store, &[http_request(format!("{origin}/large"), 8)], &mut oversized)
    .await
    .expect("response overflow should return a typed error");
  assert!(
    matches!(
      &oversized[0],
      Val::Result(Err(Some(error)))
        if matches!(error.as_ref(), Val::Variant(case, Some(value))
          if case == "response-too-large" && matches!(value.as_ref(), Val::U64(9)))
    ),
    "unexpected Wasmtime response limit error: {oversized:?}"
  );
  server.join().expect("local HTTP fixture should finish");
}

#[test]
fn component_boundary_round_trips_variants_lists_and_scalar_values() {
  let output = TestDirectory::create();
  let check = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm.cirru",
      "wasm",
      "--boundary",
      "component",
      "--check-only",
    ])
    .output()
    .expect("component fixture check should run");
  assert!(
    check.status.success(),
    "component check failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&check.stdout),
    String::from_utf8_lossy(&check.stderr)
  );
  let compile = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .args([
      "--tips-level",
      "none",
      "tests/fixtures/component-wasm.cirru",
      "wasm",
      "--boundary",
      "component",
      "--emit-path",
    ])
    .arg(&output.0)
    .output()
    .expect("component fixture should compile");
  assert!(
    compile.status.success(),
    "component compile failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&compile.stdout),
    String::from_utf8_lossy(&compile.stderr)
  );

  let wasm = output.0.join("program.wasm");
  let script = r#"
const fs = require("fs");
const bytes = fs.readFileSync(process.argv[1]);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
const importNames = imports.map(({ module, name }) => `${module}/${name}`).sort();
if (importNames.join(",") !== "host/add-one,host/bool-not,host/buffer,host/echo,host/event,host/numbers,host/numeric-scalars,host/option-number,host/ping,host/profile,host/result-number") {
  throw new Error(`unexpected imports: ${importNames.join(",")}`);
}
let instance;
let hostBoolOverride = null;
let hostBufferReturnsInvalidRange = false;
let hostNumbersReturnsInvalidRange = false;
const host = {
  "add-one": value => value + 1,
  "bool-not": value => {
    if (value !== 0 && value !== 1) throw new Error(`host received invalid bool ${value}`);
    return hostBoolOverride ?? (value === 0 ? 1 : 0);
  },
  buffer: (inputPtr, inputLen, retPtr) => {
    const e = instance.exports;
    if (hostBufferReturnsInvalidRange) {
      const memory = new DataView(e.memory.buffer);
      memory.setUint32(retPtr, e.memory.buffer.byteLength - 1, true);
      memory.setUint32(retPtr + 4, 2, true);
      return;
    }
    const input = Uint8Array.from(new Uint8Array(e.memory.buffer, inputPtr, inputLen));
    input.reverse();
    const outputPtr = e.cabi_realloc(0, 0, 1, input.length);
    new Uint8Array(e.memory.buffer, outputPtr, input.length).set(input);
    const memory = new DataView(e.memory.buffer);
    memory.setUint32(retPtr, outputPtr, true);
    memory.setUint32(retPtr + 4, input.length, true);
  },
  echo: (inputPtr, inputLen, retPtr) => {
    const e = instance.exports;
    const text = Buffer.from(e.memory.buffer, inputPtr, inputLen).toString("utf8");
    const output = Buffer.from(`${text} from host`, "utf8");
    const outputPtr = e.cabi_realloc(0, 0, 1, output.length);
    new Uint8Array(e.memory.buffer, outputPtr, output.length).set(output);
    const memory = new DataView(e.memory.buffer);
    memory.setUint32(retPtr, outputPtr, true);
    memory.setUint32(retPtr + 4, output.length, true);
  },
  numbers: (inputPtr, inputLen, retPtr) => {
    const e = instance.exports;
    const memory = new DataView(e.memory.buffer);
    if (hostNumbersReturnsInvalidRange) {
      memory.setUint32(retPtr, e.memory.buffer.byteLength - 4, true);
      memory.setUint32(retPtr + 4, 1, true);
      return;
    }
    const values = Array.from({ length: inputLen }, (_, index) => memory.getFloat64(inputPtr + index * 8, true)).reverse();
    const outputPtr = e.cabi_realloc(0, 0, 8, values.length * 8);
    const outputMemory = new DataView(e.memory.buffer);
    values.forEach((value, index) => outputMemory.setFloat64(outputPtr + index * 8, value, true));
    outputMemory.setUint32(retPtr, outputPtr, true);
    outputMemory.setUint32(retPtr + 4, values.length, true);
  },
  "numeric-scalars": (f32, f64, i16, i32, i64, i8, u16, u32, u64, u8, retPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setFloat32(retPtr, f32, true);
    memory.setFloat64(retPtr + 8, f64, true);
    memory.setInt16(retPtr + 16, i16, true);
    memory.setInt32(retPtr + 20, i32, true);
    memory.setBigInt64(retPtr + 24, i64, true);
    memory.setInt8(retPtr + 32, i8);
    memory.setUint16(retPtr + 34, u16, true);
    memory.setUint32(retPtr + 36, u32, true);
    memory.setBigUint64(retPtr + 40, u64, true);
    memory.setUint8(retPtr + 48, u8);
  },
  "option-number": (discriminant, payload, retPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, discriminant);
    if (discriminant === 1) memory.setFloat64(retPtr + 8, payload, true);
  },
  "result-number": (discriminant, payload0, payload1, retPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, discriminant);
    if (discriminant === 0) {
      memory.setBigUint64(retPtr + 8, payload0, true);
    } else {
      memory.setUint32(retPtr + 8, Number(payload0), true);
      memory.setUint32(retPtr + 12, payload1, true);
    }
  },
  profile: (active, maybeDiscriminant, maybePtr, maybeLen, namePtr, nameLen, outcomeDiscriminant, outcomePtr, outcomeLen, scoresPtr, scoresLen, score, retPtr) => {
    if (active !== 0 && active !== 1) throw new Error(`host received invalid profile bool ${active}`);
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, active === 0 ? 1 : 0);
    memory.setUint8(retPtr + 4, maybeDiscriminant);
    memory.setUint32(retPtr + 8, maybePtr, true);
    memory.setUint32(retPtr + 12, maybeLen, true);
    memory.setUint32(retPtr + 16, namePtr, true);
    memory.setUint32(retPtr + 20, nameLen, true);
    memory.setUint8(retPtr + 24, outcomeDiscriminant);
    memory.setUint32(retPtr + 28, outcomePtr, true);
    memory.setUint32(retPtr + 32, outcomeLen, true);
    memory.setUint32(retPtr + 36, scoresPtr, true);
    memory.setUint32(retPtr + 40, scoresLen, true);
    memory.setFloat64(retPtr + 48, score + 1, true);
  },
  event: (discriminant, payload0, payload1, payload2, payload3, payload4, payload5, payload6, payload7, payload8, payload9, payload10, payload11, retPtr) => {
    const memory = new DataView(instance.exports.memory.buffer);
    memory.setUint8(retPtr, discriminant);
    if (discriminant === 1) {
      memory.setBigUint64(retPtr + 8, payload0, true);
      memory.setBigUint64(retPtr + 16, payload1, true);
    } else if (discriminant === 2) {
      memory.setUint32(retPtr + 8, Number(payload0), true);
      memory.setUint32(retPtr + 12, Number(payload1), true);
    } else if (discriminant === 3) {
      memory.setUint8(retPtr + 8, Number(payload0));
      memory.setUint8(retPtr + 12, Number(payload1));
      memory.setUint32(retPtr + 16, payload2, true);
      memory.setUint32(retPtr + 20, payload3, true);
      memory.setUint32(retPtr + 24, payload4, true);
      memory.setUint32(retPtr + 28, payload5, true);
      memory.setUint8(retPtr + 32, payload6);
      memory.setUint32(retPtr + 36, payload7, true);
      memory.setUint32(retPtr + 40, payload8, true);
      memory.setUint32(retPtr + 44, payload9, true);
      memory.setUint32(retPtr + 48, payload10, true);
      memory.setFloat64(retPtr + 56, payload11, true);
    }
  },
  ping: () => undefined,
};
WebAssembly.instantiate(module, { host }).then(result => {
  instance = result;
  const e = instance.exports;
  const allocateBytes = bytes => {
    const input = Uint8Array.from(bytes);
    const ptr = e.cabi_realloc(0, 0, 1, input.length);
    new Uint8Array(e.memory.buffer, ptr, input.length).set(input);
    return [ptr, input.length];
  };
  const readBytesResult = ret => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Uint8Array.from(new Uint8Array(e.memory.buffer, ptr, len));
  };
  const allocateNumberList = values => {
    const ptr = e.cabi_realloc(0, 0, 8, values.length * 8);
    const memory = new DataView(e.memory.buffer);
    values.forEach((value, index) => memory.setFloat64(ptr + index * 8, value, true));
    return [ptr, values.length];
  };
  const readNumberListResult = ret => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Array.from({ length: len }, (_, index) => memory.getFloat64(ptr + index * 8, true));
  };
  const allocateBoolList = values => {
    const ptr = e.cabi_realloc(0, 0, 1, values.length);
    new Uint8Array(e.memory.buffer, ptr, values.length).set(values);
    return [ptr, values.length];
  };
  const readBoolListResult = ret => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Array.from(new Uint8Array(e.memory.buffer, ptr, len));
  };
  const allocatePairList = pairs => {
    const ptr = e.cabi_realloc(0, 0, 4, pairs.length * 8);
    const memory = new DataView(e.memory.buffer);
    pairs.forEach(([itemPtr, itemLen], index) => {
      memory.setUint32(ptr + index * 8, itemPtr, true);
      memory.setUint32(ptr + index * 8 + 4, itemLen, true);
    });
    return [ptr, pairs.length];
  };
  const readPairListResult = (ret, readItem) => {
    const memory = new DataView(e.memory.buffer);
    const ptr = memory.getUint32(ret, true);
    const len = memory.getUint32(ret + 4, true);
    return Array.from({ length: len }, (_, index) => {
      const itemPtr = memory.getUint32(ptr + index * 8, true);
      const itemLen = memory.getUint32(ptr + index * 8 + 4, true);
      return readItem(itemPtr, itemLen);
    });
  };
  const readProfile = ret => {
    const memory = new DataView(e.memory.buffer);
    const namePtr = memory.getUint32(ret + 16, true);
    const nameLen = memory.getUint32(ret + 20, true);
    return {
      name: Buffer.from(e.memory.buffer, namePtr, nameLen).toString("utf8"),
      stats: { score: memory.getFloat64(ret + 48, true) },
      maybeName: memory.getUint8(ret + 4) === 0
        ? null
        : Buffer.from(e.memory.buffer, memory.getUint32(ret + 8, true), memory.getUint32(ret + 12, true)).toString("utf8"),
      outcome: memory.getUint8(ret + 24) === 0
        ? { ok: Array.from(
            { length: memory.getUint32(ret + 32, true) },
            (_, index) => memory.getFloat64(memory.getUint32(ret + 28, true) + index * 8, true),
          ) }
        : { err: Buffer.from(
            e.memory.buffer,
            memory.getUint32(ret + 28, true),
            memory.getUint32(ret + 32, true),
          ).toString("utf8") },
      active: memory.getUint8(ret),
      scores: (() => {
        const ptr = memory.getUint32(ret + 36, true);
        const len = memory.getUint32(ret + 40, true);
        return Array.from({ length: len }, (_, index) => memory.getFloat64(ptr + index * 8, true));
      })(),
    };
  };
  const readEvent = ret => {
    const memory = new DataView(e.memory.buffer);
    const discriminant = memory.getUint8(ret);
    if (discriminant === 0) return { tag: "idle" };
    if (discriminant === 1) {
      return { tag: "moved", values: [memory.getFloat64(ret + 8, true), memory.getFloat64(ret + 16, true)] };
    }
    if (discriminant === 2) {
      return {
        tag: "named",
        value: Buffer.from(e.memory.buffer, memory.getUint32(ret + 8, true), memory.getUint32(ret + 12, true)).toString("utf8"),
      };
    }
    if (discriminant === 3) return { tag: "profile", value: readProfile(ret + 8) };
    throw new Error(`unexpected Event discriminant ${discriminant}`);
  };
  const readNumericScalars = ret => {
    const memory = new DataView(e.memory.buffer);
    return [
      memory.getFloat32(ret, true),
      memory.getFloat64(ret + 8, true),
      memory.getInt16(ret + 16, true),
      memory.getInt32(ret + 20, true),
      memory.getBigInt64(ret + 24, true),
      memory.getInt8(ret + 32),
      memory.getUint16(ret + 34, true),
      memory.getUint32(ret + 36, true),
      memory.getBigUint64(ret + 40, true),
      memory.getUint8(ret + 48),
    ];
  };
  const expectBytes = (actual, expected, label) => {
    if (actual.length !== expected.length) {
      throw new Error(`${label}: got length ${actual.length}, expected ${expected.length}`);
    }
    const mismatch = actual.findIndex((byte, index) => byte !== expected[index]);
    if (mismatch !== -1) {
      throw new Error(`${label}: byte ${mismatch} was ${actual[mismatch]}, expected ${expected[mismatch]}`);
    }
  };
  const floatBits = value => {
    const bytes = new ArrayBuffer(8);
    const view = new DataView(bytes);
    view.setFloat64(0, value, true);
    return view.getBigUint64(0, true);
  };
  const readOptionNumber = ret => {
    const view = new DataView(e.memory.buffer);
    const discriminant = view.getUint8(ret);
    return discriminant === 0 ? null : view.getFloat64(ret + 8, true);
  };
  const readTextVariant = (ret, payloadOffset) => {
    const view = new DataView(e.memory.buffer);
    const discriminant = view.getUint8(ret);
    const itemPtr = view.getUint32(ret + payloadOffset, true);
    const itemLen = view.getUint32(ret + payloadOffset + 4, true);
    return [discriminant, Buffer.from(e.memory.buffer, itemPtr, itemLen).toString("utf8")];
  };
  if (e["add-one"](41) !== 42) throw new Error("Number adapter did not round-trip");
  const numericScalars = [1.5, 1.25, -32768, -2147483648, -9007199254740991n, -128, 65535, 4294967295, 9007199254740991n, 255];
  for (const name of ["echo-numeric-scalars", "call-host-numeric-scalars"]) {
    const actual = readNumericScalars(e[name](...numericScalars));
    if (actual.length !== numericScalars.length || actual.some((value, index) => value !== numericScalars[index])) {
      throw new Error(`${name} did not preserve canonical numeric scalars: ${actual}`);
    }
  }
  for (const invalid of [
    [Number.NaN, ...numericScalars.slice(1)],
    [...numericScalars.slice(0, 5), 128, ...numericScalars.slice(6)],
    [...numericScalars.slice(0, 9), -1],
    [...numericScalars.slice(0, 4), -9007199254740992n, ...numericScalars.slice(5)],
    [...numericScalars.slice(0, 8), 9007199254740992n, numericScalars[9]],
  ]) {
    let trapped = false;
    try { e["echo-numeric-scalars"](...invalid); } catch (error) { trapped = error instanceof WebAssembly.RuntimeError; }
    if (!trapped) throw new Error(`invalid canonical numeric input did not trap: ${invalid}`);
  }
  if (e["bool-not"](1) !== 0 || e["bool-not"](0) !== 1) throw new Error("Bool adapter did not round-trip");
  if (e["choose-number"](1, 3, 4) !== 3 || e["choose-number"](0, 3, 4) !== 4) {
    throw new Error("mixed Bool and Number adapter did not round-trip");
  }
  if (readOptionNumber(e["echo-option-number"](0, 0)) !== null) {
    throw new Error("Option<Number> none did not round-trip");
  }
  if (readOptionNumber(e["echo-option-number"](1, 7.5)) !== 7.5) {
    throw new Error("Option<Number> some did not round-trip");
  }
  const optionText = Buffer.from("可选", "utf8");
  const [optionTextPtr, optionTextLen] = allocateBytes(optionText);
  const [optionTextTag, optionTextValue] = readTextVariant(e["echo-option-text"](1, optionTextPtr, optionTextLen), 4);
  if (optionTextTag !== 1 || optionTextValue !== "可选") throw new Error("Option<String> did not round-trip");
  const resultNumberOk = e["echo-result-number"](0, floatBits(9.25), 0);
  const resultNumberView = new DataView(e.memory.buffer);
  if (resultNumberView.getUint8(resultNumberOk) !== 0 || resultNumberView.getFloat64(resultNumberOk + 8, true) !== 9.25) {
    throw new Error("Result<Number,String> ok did not round-trip");
  }
  e["cabi_post_echo-result-number"](resultNumberOk);
  const resultError = Buffer.from("bad", "utf8");
  const [resultErrorPtr, resultErrorLen] = allocateBytes(resultError);
  const resultErrorRet = e["echo-result-number"](1, BigInt(resultErrorPtr), resultErrorLen);
  const [resultErrorTag, resultErrorValue] = readTextVariant(resultErrorRet, 8);
  if (resultErrorTag !== 1 || resultErrorValue !== "bad") throw new Error("Result<Number,String> err did not round-trip");
  e["cabi_post_echo-result-number"](resultErrorRet);
  const unitOk = e["echo-result-unit"](0, 0, 0);
  if (new DataView(e.memory.buffer).getUint8(unitOk) !== 0) throw new Error("Result<Unit,String> ok did not round-trip");
  e["cabi_post_echo-result-unit"](unitOk);
  const unitErrorRet = e["echo-result-unit"](1, resultErrorPtr, resultErrorLen);
  const [unitErrorTag, unitErrorValue] = readTextVariant(unitErrorRet, 4);
  if (unitErrorTag !== 1 || unitErrorValue !== "bad") throw new Error("Result<Unit,String> err did not round-trip");
  e["cabi_post_echo-result-unit"](unitErrorRet);
  const [resultListPtr, resultListLen] = allocateNumberList([2, 4, 8]);
  const resultListRet = e["echo-result-numbers"](0, resultListPtr, resultListLen);
  const resultListView = new DataView(e.memory.buffer);
  const loweredListPtr = resultListView.getUint32(resultListRet + 4, true);
  const loweredListLen = resultListView.getUint32(resultListRet + 8, true);
  const loweredList = Array.from({ length: loweredListLen }, (_, index) => resultListView.getFloat64(loweredListPtr + index * 8, true));
  if (resultListView.getUint8(resultListRet) !== 0 || loweredList.join(",") !== "2,4,8") {
    throw new Error("Result<List<Number>,String> ok did not round-trip");
  }
  if (e.ping() !== undefined) throw new Error("Unit result should use the zero-result canonical shape");
  if (readOptionNumber(e["call-host-option-number"](1, 12.5)) !== 12.5) {
    throw new Error("imported Option<Number> did not round-trip");
  }
  const importedResultOk = e["call-host-result-number"](0, floatBits(6.5), 0);
  if (new DataView(e.memory.buffer).getFloat64(importedResultOk + 8, true) !== 6.5) {
    throw new Error("imported Result<Number,String> ok did not round-trip");
  }
  const [importedResultTag, importedResultError] = readTextVariant(
    e["call-host-result-number"](1, BigInt(resultErrorPtr), resultErrorLen),
    8,
  );
  if (importedResultTag !== 1 || importedResultError !== "bad") {
    throw new Error("imported Result<Number,String> err did not round-trip");
  }
  if (e["call-host-ping"]() !== undefined) throw new Error("imported Unit result should remain zero-result");
  const [profileNamePtr, profileNameLen] = allocateBytes(Buffer.from("Ada", "utf8"));
  const [profileScoresPtr, profileScoresLen] = allocateNumberList([1, 2, 3]);
  const [profileOutcomePtr, profileOutcomeLen] = allocateNumberList([4, 5]);
  const directProfile = readProfile(e["echo-profile"](1, 1, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(directProfile) !== JSON.stringify({ name: "Ada", stats: { score: 7.5 }, maybeName: "Ada", outcome: { ok: [4, 5] }, active: 1, scores: [1, 2, 3] })) {
    throw new Error(`Struct record did not round-trip: ${JSON.stringify(directProfile)}`);
  }
  const hostProfile = readProfile(e["call-host-profile"](1, 1, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(hostProfile) !== JSON.stringify({ name: "Ada", stats: { score: 8.5 }, maybeName: "Ada", outcome: { ok: [4, 5] }, active: 0, scores: [1, 2, 3] })) {
    throw new Error(`imported Struct record did not round-trip: ${JSON.stringify(hostProfile)}`);
  }
  const [profileErrorPtr, profileErrorLen] = allocateBytes(Buffer.from("bad", "utf8"));
  const directAlternateProfile = readProfile(e["echo-profile"](1, 0, 0, 0, profileNamePtr, profileNameLen, 1, profileErrorPtr, profileErrorLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(directAlternateProfile) !== JSON.stringify({ name: "Ada", stats: { score: 7.5 }, maybeName: null, outcome: { err: "bad" }, active: 1, scores: [1, 2, 3] })) {
    throw new Error(`Struct None/Err record did not round-trip: ${JSON.stringify(directAlternateProfile)}`);
  }
  const hostAlternateProfile = readProfile(e["call-host-profile"](1, 0, 0, 0, profileNamePtr, profileNameLen, 1, profileErrorPtr, profileErrorLen, profileScoresPtr, profileScoresLen, 7.5));
  if (JSON.stringify(hostAlternateProfile) !== JSON.stringify({ name: "Ada", stats: { score: 8.5 }, maybeName: null, outcome: { err: "bad" }, active: 0, scores: [1, 2, 3] })) {
    throw new Error(`imported Struct None/Err record did not round-trip: ${JSON.stringify(hostAlternateProfile)}`);
  }
  const eventIdle = [0, 0n, 0n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  const eventNamed = [2, BigInt(profileNamePtr), BigInt(profileNameLen), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  const eventMoved = [1, floatBits(3), floatBits(4), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  const eventProfile = [3, 1n, 1n, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5];
  for (const [args, expected] of [
    [eventIdle, { tag: "idle" }],
    [eventNamed, { tag: "named", value: "Ada" }],
    [eventMoved, { tag: "moved", values: [3, 4] }],
    [eventProfile, { tag: "profile", value: { name: "Ada", stats: { score: 7.5 }, maybeName: "Ada", outcome: { ok: [4, 5] }, active: 1, scores: [1, 2, 3] } }],
  ]) {
    const directEvent = readEvent(e["echo-event"](...args));
    if (JSON.stringify(directEvent) !== JSON.stringify(expected)) {
      throw new Error(`Enum variant did not round-trip: ${JSON.stringify(directEvent)}`);
    }
    const hostEvent = readEvent(e["call-host-event"](...args));
    if (JSON.stringify(hostEvent) !== JSON.stringify(expected)) {
      throw new Error(`imported Enum variant did not round-trip: ${JSON.stringify(hostEvent)}`);
    }
  }
  for (const discriminant of [2, 256]) {
    let invalidVariantTrapped = false;
    try { e["echo-option-number"](discriminant, 0); } catch (error) {
      invalidVariantTrapped = error instanceof WebAssembly.RuntimeError;
    }
    if (!invalidVariantTrapped) throw new Error(`invalid Option discriminant ${discriminant} did not trap`);
  }
  for (const invoke of [
    () => e["bool-not"](2),
    () => e["choose-number"](2, 3, 4),
    () => e["echo-profile"](2, 1, profileNamePtr, profileNameLen, profileNamePtr, profileNameLen, 0, profileOutcomePtr, profileOutcomeLen, profileScoresPtr, profileScoresLen, 7.5),
    () => e["echo-event"](4, 0n, 0n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
  ]) {
    let trapped = false;
    try { invoke(); } catch (error) { trapped = error instanceof WebAssembly.RuntimeError; }
    if (!trapped) throw new Error("invalid canonical Bool input did not trap");
  }
  const input = Buffer.from("你好 Calcit", "utf8");
  const ptr = e.cabi_realloc(0, 0, 1, input.length);
  new Uint8Array(e.memory.buffer, ptr, input.length).set(input);
  const ret = e["echo-text"](ptr, input.length);
  const memory = new DataView(e.memory.buffer);
  const outputPtr = memory.getUint32(ret, true);
  const outputLen = memory.getUint32(ret + 4, true);
  const text = Buffer.from(e.memory.buffer, outputPtr, outputLen).toString("utf8");
  if (text !== "你好 Calcit") throw new Error(`String adapter returned ${text}`);
  e["cabi_post_echo-text"](ret);
  for (const bytes of [[], [0, 255, 17], [128, 0, 254, 1]]) {
    const [bufferPtr, bufferLen] = allocateBytes(bytes);
    expectBytes(readBytesResult(e["echo-buffer"](bufferPtr, bufferLen)), bytes, "Buffer adapter did not round-trip");
  }
  const [taggedBufferPtr, taggedBufferLen] = allocateBytes([0, 255, 17]);
  if (e["is-buffer"](taggedBufferPtr, taggedBufferLen) !== 1) {
    throw new Error("Buffer lift did not preserve the Calcit type tag");
  }
  for (const numbers of [[], [1, -2.5, 3, 3]]) {
    const [listPtr, listLen] = allocateNumberList(numbers);
    const actual = readNumberListResult(e["echo-numbers"](listPtr, listLen));
    if (actual.length !== numbers.length || actual.some((value, index) => !Object.is(value, numbers[index]))) {
      throw new Error(`Number List adapter returned ${actual}`);
    }
  }
  const [boolListPtr, boolListLen] = allocateBoolList([1, 0, 1]);
  if (readBoolListResult(e["echo-bools"](boolListPtr, boolListLen)).join(",") !== "1,0,1") {
    throw new Error("Bool List adapter did not round-trip");
  }
  const textValues = ["alpha", "", "世界"];
  const textPairs = textValues.map(value => allocateBytes(Buffer.from(value, "utf8")));
  const [textListPtr, textListLen] = allocatePairList(textPairs);
  const textResultRet = e["echo-texts"](textListPtr, textListLen);
  const textResultView = new DataView(e.memory.buffer);
  const textResultBacking = textResultView.getUint32(textResultRet, true);
  const textResultItems = Array.from({ length: textListLen }, (_, index) => textResultView.getUint32(textResultBacking + index * 8, true));
  const textResult = readPairListResult(
    textResultRet,
    (itemPtr, itemLen) => Buffer.from(e.memory.buffer, itemPtr, itemLen).toString("utf8"),
  );
  if (JSON.stringify(textResult) !== JSON.stringify(textValues)) throw new Error(`String List adapter returned ${textResult}`);
  e["cabi_post_echo-texts"](textResultRet);
  const reclaimedRoot = e.cabi_realloc(0, 0, 4, 8);
  const reclaimedBacking = e.cabi_realloc(0, 0, 4, textListLen * 8);
  const reclaimedLongItem = e.cabi_realloc(0, 0, 1, Buffer.byteLength("世界"));
  const reclaimedShortItem = e.cabi_realloc(0, 0, 1, Buffer.byteLength("alpha"));
  if (
    reclaimedRoot !== textResultRet
    || reclaimedBacking !== textResultBacking
    || JSON.stringify([reclaimedLongItem, reclaimedShortItem].toSorted()) !== JSON.stringify(textResultItems.filter(Boolean).toSorted())
  ) {
    throw new Error("recursive List<String> post-return did not reclaim its result area, backing storage, and elements");
  }
  e.cabi_realloc(reclaimedRoot, 8, 4, 0);
  e.cabi_realloc(reclaimedBacking, textListLen * 8, 4, 0);
  e.cabi_realloc(reclaimedLongItem, Buffer.byteLength("世界"), 1, 0);
  e.cabi_realloc(reclaimedShortItem, Buffer.byteLength("alpha"), 1, 0);
  const bufferValues = [[0, 255], [], [17, 0, 128]];
  const bufferPairs = bufferValues.map(allocateBytes);
  const [bufferListPtr, bufferListLen] = allocatePairList(bufferPairs);
  const bufferResult = readPairListResult(
    e["echo-buffers"](bufferListPtr, bufferListLen),
    (itemPtr, itemLen) => Array.from(new Uint8Array(e.memory.buffer, itemPtr, itemLen)),
  );
  if (JSON.stringify(bufferResult) !== JSON.stringify(bufferValues)) throw new Error(`Buffer List adapter returned ${bufferResult}`);
  const nestedValues = [[1, 2], [], [-3, 4.5, 7]];
  const nestedPairs = nestedValues.map(allocateNumberList);
  const [nestedPtr, nestedLen] = allocatePairList(nestedPairs);
  const nestedResult = readPairListResult(
    e["echo-number-lists"](nestedPtr, nestedLen),
    (itemPtr, itemLen) => Array.from(
      { length: itemLen },
      (_, index) => new DataView(e.memory.buffer).getFloat64(itemPtr + index * 8, true),
    ),
  );
  if (JSON.stringify(nestedResult) !== JSON.stringify(nestedValues)) throw new Error(`nested List adapter returned ${nestedResult}`);
  const [invalidBoolListPtr, invalidBoolListLen] = allocateBoolList([0, 2]);
  let invalidBoolListTrapped = false;
  try { e["echo-bools"](invalidBoolListPtr, invalidBoolListLen); } catch (error) {
    invalidBoolListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidBoolListTrapped) throw new Error("invalid Bool List element did not trap");
  let invalidNumberListTrapped = false;
  try { e["echo-numbers"](e.memory.buffer.byteLength - 4, 1); } catch (error) {
    invalidNumberListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidNumberListTrapped) throw new Error("out-of-bounds Number List input did not trap");
  const misalignedNumberPtr = e.cabi_realloc(0, 0, 8, 16) + 4;
  let misalignedNumberListTrapped = false;
  try { e["echo-numbers"](misalignedNumberPtr, 1); } catch (error) {
    misalignedNumberListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!misalignedNumberListTrapped) throw new Error("misaligned Number List input did not trap");
  let overflowingNumberListTrapped = false;
  try { e["echo-numbers"](0, 0x20000000); } catch (error) {
    overflowingNumberListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!overflowingNumberListTrapped) throw new Error("overflowing Number List length did not trap");
  const [misalignedNestedPtr, misalignedNestedLen] = allocatePairList([[nestedPairs[0][0] + 4, 1]]);
  let misalignedNestedListTrapped = false;
  try { e["echo-number-lists"](misalignedNestedPtr, misalignedNestedLen); } catch (error) {
    misalignedNestedListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!misalignedNestedListTrapped) throw new Error("misaligned nested Number List input did not trap");
  const [pairListPtr] = allocatePairList([nestedPairs[0]]);
  let misalignedPairListTrapped = false;
  try { e["echo-number-lists"](pairListPtr + 2, 1); } catch (error) {
    misalignedPairListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!misalignedPairListTrapped) throw new Error("misaligned nested List pair input did not trap");
  const [invalidNestedPtr, invalidNestedLen] = allocatePairList([[e.memory.buffer.byteLength - 4, 1]]);
  let invalidNestedListTrapped = false;
  try { e["echo-number-lists"](invalidNestedPtr, invalidNestedLen); } catch (error) {
    invalidNestedListTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidNestedListTrapped) throw new Error("out-of-bounds nested Number List input did not trap");
  const growingNumberCount = e.memory.buffer.byteLength / 8 + 257;
  const growingNumbers = Array.from({ length: growingNumberCount }, (_, index) => index % 251);
  const [growingNumberPtr, growingNumberLen] = allocateNumberList(growingNumbers);
  for (let round = 0; round < 2; round += 1) {
    const actual = readNumberListResult(e["echo-numbers"](growingNumberPtr, growingNumberLen));
    if (actual.length !== growingNumbers.length || actual.some((value, index) => value !== growingNumbers[index])) {
      throw new Error(`growing Number List round ${round} did not preserve values`);
    }
  }
  const growingBytes = Uint8Array.from({ length: e.memory.buffer.byteLength + 257 }, (_, index) => index % 251);
  const [growingPtr, growingLen] = allocateBytes(growingBytes);
  for (let round = 0; round < 2; round += 1) {
    expectBytes(
      readBytesResult(e["echo-buffer"](growingPtr, growingLen)),
      growingBytes,
      `growing Buffer round ${round}`,
    );
  }
  const [yesPtr, yesLen] = allocateBytes([0, 255]);
  const [noPtr, noLen] = allocateBytes([17, 0, 128]);
  expectBytes(readBytesResult(e["choose-buffer"](1, yesPtr, yesLen, noPtr, noLen)), [0, 255], "Bool+Buffer true branch");
  expectBytes(readBytesResult(e["choose-buffer"](0, yesPtr, yesLen, noPtr, noLen)), [17, 0, 128], "Bool+Buffer false branch");
  let invalidBufferInputTrapped = false;
  try { e["echo-buffer"](e.memory.buffer.byteLength - 1, 2); } catch (error) {
    invalidBufferInputTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidBufferInputTrapped) throw new Error("out-of-bounds Buffer input did not trap");
  const moved = e.cabi_realloc(ptr, input.length, 8, input.length + 4);
  if (moved % 8 !== 0) throw new Error("cabi_realloc ignored alignment");
  const preserved = Buffer.from(e.memory.buffer, moved, input.length).toString("utf8");
  if (preserved !== "你好 Calcit") throw new Error("cabi_realloc did not preserve bytes");
  if (e["call-host-add-one"](9) !== 10) throw new Error("Number import adapter did not round-trip");
  if (e["call-host-bool-not"](1) !== 0) throw new Error("Bool import adapter did not round-trip");
  hostBoolOverride = 2;
  let invalidHostBoolTrapped = false;
  try { e["call-host-bool-not"](1); } catch (error) { invalidHostBoolTrapped = error instanceof WebAssembly.RuntimeError; }
  if (!invalidHostBoolTrapped) throw new Error("invalid imported canonical Bool did not trap");
  const hostInput = Buffer.from("你好", "utf8");
  const hostInputPtr = e.cabi_realloc(0, 0, 1, hostInput.length);
  new Uint8Array(e.memory.buffer, hostInputPtr, hostInput.length).set(hostInput);
  const hostRet = e["call-host-echo"](hostInputPtr, hostInput.length);
  const hostMemory = new DataView(e.memory.buffer);
  const hostOutputPtr = hostMemory.getUint32(hostRet, true);
  const hostOutputLen = hostMemory.getUint32(hostRet + 4, true);
  const hostText = Buffer.from(e.memory.buffer, hostOutputPtr, hostOutputLen).toString("utf8");
  if (hostText !== "你好 from host") throw new Error(`String import adapter returned ${hostText}`);
  const [hostBufferPtr, hostBufferLen] = allocateBytes([0, 255, 17]);
  expectBytes(
    readBytesResult(e["call-host-buffer"](hostBufferPtr, hostBufferLen)),
    [17, 255, 0],
    "Buffer import adapter did not round-trip",
  );
  hostBufferReturnsInvalidRange = true;
  let invalidHostBufferTrapped = false;
  try { e["call-host-buffer"](hostBufferPtr, hostBufferLen); } catch (error) {
    invalidHostBufferTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidHostBufferTrapped) throw new Error("out-of-bounds imported Buffer result did not trap");
  const [hostNumbersPtr, hostNumbersLen] = allocateNumberList([1, -2.5, 7]);
  const hostNumbersResult = readNumberListResult(e["call-host-numbers"](hostNumbersPtr, hostNumbersLen));
  if (JSON.stringify(hostNumbersResult) !== JSON.stringify([7, -2.5, 1])) {
    throw new Error(`Number List import adapter returned ${hostNumbersResult}`);
  }
  hostNumbersReturnsInvalidRange = true;
  let invalidHostNumbersTrapped = false;
  try { e["call-host-numbers"](hostNumbersPtr, hostNumbersLen); } catch (error) {
    invalidHostNumbersTrapped = error instanceof WebAssembly.RuntimeError;
  }
  if (!invalidHostNumbersTrapped) throw new Error("out-of-bounds imported Number List result did not trap");
  const headerBoundary = e.memory.buffer.byteLength;
  e.__heap_ptr.value = headerBoundary;
  const boundaryPtr = e.cabi_realloc(0, 0, 1, 1);
  if (boundaryPtr !== headerBoundary + 8 || boundaryPtr + 1 > e.memory.buffer.byteLength) {
    throw new Error(`cabi_realloc did not grow memory before writing a boundary header: ${boundaryPtr}`);
  }
  const pagesBefore = e.memory.buffer.byteLength / 65536;
  const largeSize = e.memory.buffer.byteLength + 1;
  const largePtr = e.cabi_realloc(0, 0, 1, largeSize);
  const pagesAfter = e.memory.buffer.byteLength / 65536;
  if (pagesAfter <= pagesBefore) throw new Error("cabi_realloc did not grow memory");
  if (largePtr + largeSize > e.memory.buffer.byteLength) throw new Error("grown allocation is out of bounds");
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
"#;
  let runtime = Command::new("node")
    .args(["-e", script])
    .arg(&wasm)
    .output()
    .expect("Node.js should validate and instantiate the generated module");
  assert!(
    runtime.status.success(),
    "component runtime failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&runtime.stdout),
    String::from_utf8_lossy(&runtime.stderr)
  );
}

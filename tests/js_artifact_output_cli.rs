use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn emit_js(emit_path: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .args(["--tips-level", "none", "--init-fn", "app.main/manifest-main!", "--emit-path"])
    .arg(emit_path)
    .args(["examples/wasi-command/calcit.cirru", "js"])
    .output()
    .expect("Calcit CLI should run")
}

#[test]
fn js_codegen_creates_nested_output_and_only_reports_written_artifacts() {
  let root = tempfile::tempdir().expect("temporary output root");
  let emit_path = root.path().join("not-created/js");
  assert!(!emit_path.exists());

  let output = emit_js(&emit_path);
  assert!(
    output.status.success(),
    "stdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(emit_path.join("app.main.mjs").is_file());
  let stdout = String::from_utf8_lossy(&output.stdout);
  let emitted: Vec<_> = stdout.lines().filter_map(|line| line.strip_prefix("emitted: ")).collect();
  assert!(!emitted.is_empty(), "expected artifact paths in stdout: {stdout}");
  for path in emitted {
    assert!(Path::new(path).is_file(), "reported artifact does not exist: {path}");
  }
}

#[test]
fn generated_core_graphs_preserve_each_others_nominal_builtin_dispatch() {
  // Module loading and shared-runtime lifetime are JS host boundaries, not
  // operations that an ordinary Calcit definition test can express.
  let root = tempfile::tempdir().expect("temporary graph root");
  let graphs = [root.path().join("graph-a"), root.path().join("graph-b")];
  for graph in &graphs {
    let output = emit_js(graph);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  }
  assert_eq!(
    fs::read(graphs[0].join("calcit.core.mjs")).unwrap(),
    fs::read(graphs[1].join("calcit.core.mjs")).unwrap(),
    "the regression must load two instances of identical generated core code"
  );
  let script = r#"
import assert from "node:assert/strict";
import { mkdir, realpath, symlink } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const root = process.env.CALCIT_GRAPH_ROOT;
const runtime = process.env.CALCIT_RUNTIME_ROOT;
const packageScope = join(root, "node_modules", "@calcit");
await mkdir(packageScope, { recursive: true });
const packageLink = join(packageScope, "procs");
try {
  await symlink(runtime, packageLink, process.platform === "win32" ? "junction" : "dir");
} catch (error) {
  if (error.code !== "EEXIST") throw error;
  assert.equal(await realpath(packageLink), await realpath(runtime));
}
const order = JSON.parse(process.env.CALCIT_GRAPH_ORDER);
const load = name => import(pathToFileURL(join(root, name, "calcit.core.mjs")).href);
const first = await load(order[0]);
const tag = first.init_tags(["synthetic"]).synthetic;
assert.equal(first.to_string(tag), "synthetic");
assert.equal(first._$n_assert_traits(tag, first.ToString), tag);
const childTrait = new first.CalcitTrait(first.newTag("RequiresEq"), [first.newTag("probe")], first.ToString.methodTypes, [first.Eq]);
const childImpl = new first.CalcitImpl(childTrait.name, childTrait.methods, [() => "probe"], childTrait);
const childDef = new first.CalcitStructDef(first.newTag("Required"), [], [], [childImpl]);
const childValue = new first.CalcitStructValue(childDef.name, childDef.fields, [], childDef);
assert.equal(first._$n_assert_traits(childValue, childTrait), childValue);
const second = await load(order[1]);
assert.equal(first.CalcitTag, second.CalcitTag, "the graphs share one runtime");
assert.notEqual(first.ToString, second.ToString, "evaluated trait identities remain distinct");
for (const graph of [first, second]) {
  assert.equal(graph.to_string(tag), "synthetic");
  assert.equal(graph._$n_assert_traits(tag, graph.ToString), tag);
  assert.equal(graph.invoke_method("to-string", tag), "synthetic");
}
assert.equal(first._$n_assert_traits(childValue, childTrait), childValue, "required builtin traits retain their origin");
const unrelated = new first.CalcitTrait(first.ToString.name, first.ToString.methods, first.ToString.methodTypes);
assert.throws(() => first._$n_trait_call(unrelated, "to-string", tag), /cannot find impl for trait/);
assert.throws(() => first._$n_assert_traits(tag, unrelated), /does not nominally implement/);
const impl = new first.CalcitImpl(first.ToString.name, [first.newTag("to-string")], [() => "duplicate"], first.ToString);
const definition = new first.CalcitStructDef(first.newTag("Duplicate"), [], [], [impl, impl]);
const value = new first.CalcitStructValue(definition.name, definition.fields, [], definition);
assert.throws(() => first._$n_trait_call(first.ToString, "to-string", value), /E_DUPLICATE_TRAIT_IMPL/);
"#;
  for order in [["graph-a", "graph-b"], ["graph-b", "graph-a"]] {
    let output = Command::new("node")
      .current_dir(env!("CARGO_MANIFEST_DIR"))
      .args(["--input-type=module", "--eval", script])
      .env("CALCIT_GRAPH_ROOT", root.path())
      .env("CALCIT_RUNTIME_ROOT", env!("CARGO_MANIFEST_DIR"))
      .env("CALCIT_GRAPH_ORDER", serde_json::to_string(&order).unwrap())
      .output()
      .expect("Node should load the generated graphs");
    assert!(
      output.status.success(),
      "{order:?} failed\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }
}

#[test]
fn js_codegen_rejects_output_path_that_is_a_file_without_claiming_success() {
  let root = tempfile::tempdir().expect("temporary output root");
  let emit_path = root.path().join("blocked-js");
  fs::write(&emit_path, "keep this file").expect("block output directory with a regular file");

  let output = emit_js(&emit_path);
  assert!(
    !output.status.success(),
    "codegen falsely succeeded:\n{}",
    String::from_utf8_lossy(&output.stdout)
  );
  assert!(!String::from_utf8_lossy(&output.stdout).contains("emitted:"));
  assert!(String::from_utf8_lossy(&output.stderr).contains("blocked-js"));
  assert_eq!(fs::read_to_string(&emit_path).expect("blocking file remains"), "keep this file");
}

#[test]
fn js_codegen_does_not_report_partial_output_when_an_artifact_is_blocked() {
  let root = tempfile::tempdir().expect("temporary output root");
  let emit_path = root.path().join("js-out");
  fs::create_dir(&emit_path).expect("output directory should create");
  fs::create_dir(emit_path.join("app.main.mjs")).expect("block a generated artifact with a directory");

  let output = emit_js(&emit_path);
  assert!(!output.status.success());
  assert!(!String::from_utf8_lossy(&output.stdout).contains("emitted:"));
  assert!(String::from_utf8_lossy(&output.stderr).contains("app.main.mjs"));
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-js-match-destruct-list-{}-{nonce}-{counter}", std::process::id()));
    fs::create_dir(&path).expect("temporary project should create");
    Self(path)
  }

  fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TestDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn run_calcit(args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .args(args)
    .output()
    .expect("calcit command should run")
}

/// Returns the generated body of `export function <name>(...)`, up to the next export.
fn exported_function<'a>(module: &'a str, name: &str) -> &'a str {
  let header = format!("export function {name}(");
  let start = module.find(&header).unwrap_or_else(|| panic!("missing {name} in:\n{module}"));
  let rest = &module[start + header.len()..];
  let end = rest.find("\nexport ").unwrap_or(rest.len());
  &rest[..end]
}

/// `match (destruct-list xs)` with exactly `(:none)` and `(:some h t)` branches is
/// lowered to list primitives on `xs`, without building a `ListDestruct` (#1943).
#[test]
fn match_on_destruct_list_of_local_skips_intermediate_enum() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  let emit_path = directory.path().join("js-out");
  fs::copy("tests/fixtures/js-match-destruct-list.cirru", &snapshot).expect("fixture should copy");

  let output = run_calcit(&[
    "--emit-path",
    emit_path.to_str().expect("emit path should be UTF-8"),
    snapshot.to_str().expect("snapshot path should be UTF-8"),
    "js",
  ]);
  assert!(
    output.status.success(),
    "js codegen failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );

  let module = fs::read_to_string(emit_path.join("js-match-destruct-list.main.mjs")).expect("generated module should exist");

  for name in ["sum_list", "first_or", "rest_size", "head_plus_one"] {
    let body = exported_function(&module, name);
    assert!(
      !body.contains("destruct_list") && !body.contains("_$n_enum_$o_nth") && !body.contains("ListDestruct"),
      "{name} must not build or read a ListDestruct:\n{body}"
    );
    assert!(
      body.contains("_$n_list_$o_empty_$q_(xs)"),
      "{name} must test the list directly:\n{body}"
    );
  }

  // Return position needs no closure; both binders read the subject directly.
  let sum_list = exported_function(&module, "sum_list");
  assert!(!sum_list.contains("function _fn_"), "{sum_list}");
  assert!(sum_list.contains("let h = $clt._$n_list_$o_nth(xs, 0);"), "{sum_list}");
  assert!(sum_list.contains("let t = $clt._$n_list_$o_rest(xs);"), "{sum_list}");

  // `_` binders compute nothing.
  let first_or = exported_function(&module, "first_or");
  assert!(!first_or.contains("_$n_list_$o_rest"), "{first_or}");
  let rest_size = exported_function(&module, "rest_size");
  assert!(!rest_size.contains("_$n_list_$o_nth"), "{rest_size}");

  // A binder that shadows the subject keeps the generic enum path.
  let shadowed = exported_function(&module, "shadowed_subject");
  assert!(shadowed.contains("destruct_list(xs)"), "{shadowed}");

  // `a_b` and the subject `a-b` escape to the same JS name, which the fused
  // `let a_b = nth(a_b, 0)` would read in its temporal dead zone.
  let escaped = exported_function(&module, "escaped_shadow");
  assert!(escaped.contains("destruct_list(a_b)"), "{escaped}");

  // Head and tail binders sharing a name also keep the generic path, which
  // binds only the later one (the value native sees) instead of redeclaring it.
  let same = exported_function(&module, "same_binders");
  assert!(same.contains("destruct_list(xs)"), "{same}");
  assert_eq!(same.matches("let h = ").count(), 1, "{same}");
}

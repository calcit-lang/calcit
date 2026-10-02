use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn nominal_write_proof_checks_method_value_without_rewriting_source() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/WriteState",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defstruct WriteState (:count 'Number) (:payload 'Dynamic)",
      ],
    ),
    "create nominal state",
  );
  for (name, input, body) in [
    ("write-known", "Number", "state .assoc :count incoming"),
    ("write-storage", "Dynamic", "state .assoc :payload incoming"),
    ("write-open", "Dynamic", "state .assoc :count incoming"),
    ("write-prefix", "Dynamic", ".assoc state :count incoming"),
    ("write-wrong", "String", "state .assoc :count incoming"),
    ("write-missing", "Number", "state .assoc :missing incoming"),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (state incoming)\n  {body}"),
        ],
      ),
      "create method write",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ [] 'WriteState '{input}) (:return 'WriteState)"),
        ],
      ),
      "declare method contract",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "fix-command.main/write-known",
        "retains-nominal-value",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= 2 $ :count $ write-known (WriteState :count 1 :payload nil) 2",
      ],
    ),
    "attach method semantics",
  );
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/write-known", "--require-match"]),
    "run method semantics",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "fix-command.main/write-storage",
        "preserves-open-storage",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= ([] 1 2) $ :payload $ write-storage (WriteState :count 1 :payload nil) ([] 1 2)",
      ],
    ),
    "attach explicitly open storage semantics",
  );
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/write-storage", "--require-match"]),
    "run explicitly open storage semantics",
  );
  let original = fs::read(&snapshot).unwrap();
  for name in ["write-known", "write-storage", "write-open", "write-prefix"] {
    let output = run_fix(
      &snapshot,
      &[
        "--rule",
        "nominal-write-proof-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    let report = parse_stdout(&output);
    let suggestions = report["data"]["suggestions"].as_array().unwrap();
    if matches!(name, "write-known" | "write-storage") {
      assert_success(&output, "accept proven nominal write");
      assert!(suggestions.is_empty());
    } else {
      assert!(!output.status.success());
      assert_eq!(suggestions.len(), 1);
      assert_eq!(suggestions[0]["diagnostic_code"], "E_CALL_ARGUMENT_UNPROVEN");
      assert_eq!(suggestions[0]["path"], "code@3.3");
      assert!(suggestions[0]["replacement"].is_null());
      assert_eq!(suggestions[0]["applicability"], "requires-review");
      assert!(
        suggestions[0]["origin_chain"][0]["diagnostic"]["hint"]
          .as_str()
          .unwrap()
          .contains(":count")
      );
      let edn = run_fix(
        &snapshot,
        &[
          "--rule",
          "nominal-write-proof-v1",
          "--ns",
          "fix-command.main",
          "--def",
          name,
          "--format",
          "edn",
        ],
      );
      assert!(!edn.status.success());
      cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one native nominal-write report");
      let applied = run_fix(
        &snapshot,
        &[
          "--rule",
          "nominal-write-proof-v1",
          "--ns",
          "fix-command.main",
          "--def",
          name,
          "--apply",
          "--format",
          "json",
        ],
      );
      assert!(!applied.status.success());
      assert!(parse_stdout(&applied)["data"]["suggestions"][0]["replacement"].is_null());
    }
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }
  for name in ["write-wrong", "write-missing"] {
    let output = run_fix(
      &snapshot,
      &[
        "--rule",
        "nominal-write-proof-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "edn",
      ],
    );
    assert!(!output.status.success());
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("E_CALL_ARGUMENT_MISMATCH"),
      "{}",
      String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }
}

#[test]
fn callable_contract_proof_preserves_open_storage_and_reviews_concrete_use() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).unwrap();
  let callback = "(:: 'Fn $ {} (:args $ [] 'Number) (:return 'Number))";
  let rest_callback = "(:: 'Fn $ {} (:args $ [] 'Number) (:rest 'Number) (:return 'Number))";
  let feature_callback = "(:: 'Fn $ {} (:args $ [] 'Number) (:return 'Number) (:features $ #{} :js-ffi))";
  let async_callback = "(:: 'Fn $ {} (:args $ [] 'Number) (:return 'Number) (:async true))";
  for (name, argument, returns, body) in [
    ("consume", callback, "'Number", "callback 1"),
    ("erased", "'Fn", "'Number", "consume callback"),
    ("open-input", "'Dynamic", "'Number", "consume callback"),
    ("closed", callback, "'Number", "consume callback"),
    ("generic-closed", callback, "'Number", "generic-consume 1 callback"),
    ("generic-erased", "'Fn", "'Number", "generic-consume 1 callback"),
    (
      "generic-nested",
      "'Fn",
      "'Number",
      "(if true generic-consume generic-consume) 1 (fn (n) (&+ n 1))",
    ),
    (
      "generic-inline",
      "'Fn",
      "'Number",
      "let ((apply-one generic-consume)) (apply-one 1 (fn (n) (&+ n 1)))",
    ),
    ("storage", "'Fn", "'Fn", ", callback"),
    (
      "watch-context",
      "'Fn",
      "'Number",
      "let ((watched (atom 1)) (observed (atom 0))) (add-watch watched :proof (fn (current previous) (reset! observed (&+ current previous)) &unit)) (reset! watched 2) (remove-watch watched :proof) (deref observed)",
    ),
    (
      "owned-feature",
      "'Fn",
      "'Number",
      "consume-feature (fn (n) (hint-fn ({} (:args ([] 'Number)) (:return 'Number) (:features (#{} :js-ffi)))) (unsafe-coerce n 'Number))",
    ),
    ("consume-async", async_callback, "'Number", ", 1"),
    ("erased-async", "'Fn", "'Number", "consume-async callback"),
    ("sync-for-async", "'Fn", "'Number", "consume-async (fn (n) (&+ n 1))"),
    ("consume-rest", rest_callback, "'Number", "callback 1 2"),
    ("erased-rest", "'Fn", "'Number", "consume-rest callback"),
    ("closed-rest", rest_callback, "'Number", "consume-rest callback"),
    ("consume-feature", feature_callback, "'Number", "callback 1"),
    ("erased-feature", "'Fn", "'Number", "consume-feature callback"),
    ("closed-feature", feature_callback, "'Number", "consume-feature callback"),
    (
      "wrong-signature",
      "(:: 'Fn $ {} (:args $ [] 'String) (:return 'String))",
      "'Number",
      "consume callback",
    ),
    ("unknown-invocation", "'Fn", "'Number", "callback 1"),
    ("foreign-owner", "'Fn", "'Number", "erased callback"),
    ("unscoped", "'Fn", "'Number", "unsafe-coerce callback 'Number"),
    (
      "borrowed-feature",
      "'Fn",
      "'Number",
      "consume-feature (fn (n) (unsafe-coerce n 'Number))",
    ),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (callback)\n  {body}"),
        ],
      ),
      "install callable source",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ [] {argument}) (:return {returns})"),
        ],
      ),
      "declare callable contract",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/generic-consume",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn generic-consume (value callback)\n  callback value",
      ],
    ),
    "install generic callable source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/generic-consume",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:generics $ [] 'T) (:args $ [] 'T (:: 'Fn $ {} (:args $ [] 'T) (:return 'T))) (:return 'T)",
      ],
    ),
    "declare generic callable contract",
  );
  for (name, code) in [
    ("closed", "quote $ assert= 2 $ closed $ fn (n) (&+ n 1)"),
    ("storage", "quote $ assert= true $ fn? $ storage $ fn (n) (&+ n 1)"),
    ("watch-context", "quote $ assert= 3 $ watch-context $ fn (n) (&+ n 1)"),
    ("generic-closed", "quote $ assert= 2 $ generic-closed $ fn (n) (&+ n 1)"),
    ("generic-inline", "quote $ assert= 2 $ generic-inline $ fn (n) (&+ n 1)"),
    ("generic-nested", "quote $ assert= 2 $ generic-nested $ fn (n) (&+ n 1)"),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-test",
          &target,
          "callable-contract",
          "--tags",
          "unit",
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "attach callable semantics",
    );
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "replay callable definition tests",
    );
  }
  let original = fs::read(&snapshot).unwrap();
  for (name, rejected) in [
    ("erased", true),
    ("open-input", true),
    ("closed", false),
    ("storage", false),
    ("erased-rest", true),
    ("closed-rest", false),
    ("erased-feature", true),
    ("closed-feature", false),
    ("owned-feature", false),
    ("watch-context", false),
    ("generic-closed", false),
    ("generic-inline", false),
    ("generic-nested", false),
    ("generic-erased", true),
    ("erased-async", true),
  ] {
    let selectors = [
      "--rule",
      "callable-contract-proof-v1",
      "--ns",
      "fix-command.main",
      "--def",
      name,
      "--format",
      "json",
    ];
    let output = run_fix(&snapshot, &selectors);
    assert_eq!(output.status.success(), !rejected, "{}", String::from_utf8_lossy(&output.stderr));
    let report = parse_stdout(&output);
    let suggestions = report["data"]["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), usize::from(rejected));
    if rejected {
      let suggestion = &suggestions[0];
      assert_eq!(suggestion["diagnostic_code"], "E_CALL_ARGUMENT_UNPROVEN");
      assert_eq!(suggestion["applicability"], "requires-review");
      assert!(suggestion["replacement"].is_null());
      assert_eq!(suggestion["path"], if name == "generic-erased" { "code@3.2" } else { "code@3.1" });
      assert!(
        suggestion["message"]
          .as_str()
          .unwrap()
          .contains(if name == "generic-erased" { "-> 'T" } else { "-> :number" }),
        "{suggestion}"
      );
      let hint = suggestion["origin_chain"][0]["diagnostic"]["hint"].as_str().unwrap();
      assert!(hint.contains(if name == "erased-rest" { "rest: :number" } else { "rest: none" }));
      assert!(
        hint.contains(if name == "erased-feature" {
          "declared features: [js-ffi]"
        } else {
          "declared features: []"
        }),
        "{name}: {hint}"
      );
      assert!(hint.contains("not just fn?"));
      assert!(!hint.contains("$async-invocation"));
      if name == "erased-async" {
        assert!(hint.contains("async fn(:number) -> :number"));
      }
      let applied = run_fix(&snapshot, &[selectors.as_slice(), &["--apply"]].concat());
      assert!(!applied.status.success(), "unknown callback must never acquire a guessed schema");
      let mut edn_selectors = selectors;
      edn_selectors[7] = "edn";
      let edn = run_fix(&snapshot, &edn_selectors);
      assert!(!edn.status.success());
      cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one native EDN review document");
    }
    assert_eq!(fs::read(&snapshot).unwrap(), original, "proof audit must preserve source bytes");
  }
  for (name, expected_error) in [
    ("wrong-signature", "E_CALL_ARGUMENT_MISMATCH"),
    ("unknown-invocation", "E_FN_RETURN_UNPROVEN"),
    ("foreign-owner", "outside the selected project scope"),
    ("unscoped", "E_UNSCOPED_UNSAFE_COERCE"),
    ("borrowed-feature", "E_UNSCOPED_UNSAFE_COERCE"),
    ("sync-for-async", "E_CALL_ARGUMENT_MISMATCH"),
  ] {
    let output = run_fix(
      &snapshot,
      &[
        "--rule",
        "callable-contract-proof-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--apply",
        "--format",
        "edn",
      ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected_error), "{name}: {stderr}");
    assert_eq!(fs::read(&snapshot).unwrap(), original, "a failed boundary audit must not write");
  }
}

#[test]
fn concrete_return_proof_navigates_the_implementation_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy return proof fixture");
  assert_success(
    &run_calcit(&snapshot, &["edit", "add-ns", "fix-command.returns"]),
    "add return proof owners",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.returns/sink",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn sink (n) , n",
      ],
    ),
    "add closed sink implementation",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.returns/sink",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
    ),
    "declare closed sink",
  );
  for (name, argument, returns, body, rejected) in [
    ("closed", "Number", "Number", ", x", false),
    ("generic", "T", "T", ", x", false),
    ("open-storage", "Dynamic", "Dynamic", ", x", false),
    ("open-result", "Dynamic", "Number", ", x", true),
    ("wrapped-result", "Dynamic", "Number", "open-result x", true),
    ("closed-call", "Number", "Number", "sink x", false),
    ("open-call", "Dynamic", "Number", "sink x", true),
    ("local-call", "Dynamic", "Number", "let ((f sink)) (f x)", true),
    ("proc-call", "Dynamic", "Number", "&+ x 1", true),
    ("method-call", "Dynamic", "Number", ".rem 3 x", true),
    ("postfix-call", "Dynamic", "Number", "3 .rem x", true),
    ("closed-method-call", "Number", "Number", ".rem 3 x", false),
    ("closed-postfix-call", "Number", "Number", "3 .rem x", false),
    ("cond-call", "Bool", "Number", "cond (x 1) (true 2)", false),
    ("spread-call", "Dynamic", "Number", "sink & ([] x)", true),
    ("closed-spread-call", "Number", "Number", "sink & ([] x)", false),
    ("unknown-spread-call", "Dynamic", "Number", "sink & x", true),
    ("local-spread-call", "Dynamic", "Number", "let ((f sink)) (f & ([] x))", true),
    ("proc-spread-call", "Dynamic", "Number", "&+ 1 & ([] x)", true),
    ("core-spread-call", "Dynamic", "Number", "&call-spread sink & ([] x)", true),
    ("closed-core-spread-call", "Number", "Number", "&call-spread sink & ([] x)", false),
    (
      "checked-call",
      "Dynamic",
      "Number",
      "match (try-decode-map-as x 'Number)\n    (:ok n) (sink n)\n    (:err reason) (raise reason)",
      false,
    ),
  ] {
    let target = format!("fix-command.returns/{name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (x)\n  {body}"),
        ],
      ),
      "add return implementation",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!(
            "quote $ :: 'Fn $ {{}} (:args $ [] '{argument}) (:return '{returns}){}",
            if argument == "T" { " (:generics $ [] 'T)" } else { "" }
          ),
        ],
      ),
      "declare return contract",
    );
    let original = fs::read(&snapshot).unwrap();
    let selectors = [
      "--rule",
      "concrete-return-proof-v1",
      "--ns",
      "fix-command.returns",
      "--format",
      "json",
    ];
    let output = run_fix(&snapshot, &[selectors.as_slice(), &["--def", name]].concat());
    assert_eq!(output.status.success(), !rejected, "{}", String::from_utf8_lossy(&output.stderr));
    let report = if name == "wrapped-result" {
      assert!(String::from_utf8_lossy(&output.stderr).contains("outside the selected project scope"));
      serde_json::Value::Null
    } else {
      parse_stdout(&output)
    };
    if rejected {
      // A wrapper declaration cannot hide the upstream implementation debt.
      // Single-owner selection refuses navigation outside its explicit scope.
      if name == "wrapped-result" {
        // Select both owners to make the source debt navigable.
        let scoped = run_fix(&snapshot, &selectors);
        assert!(!scoped.status.success());
        let scoped_report = parse_stdout(&scoped);
        assert!(
          scoped_report["data"]["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|suggestion| suggestion["definition"] == "fix-command.returns/open-result")
        );
      } else {
        assert_eq!(
          report["diagnostics"][0]["code"],
          if [
            "open-call",
            "local-call",
            "proc-call",
            "method-call",
            "postfix-call",
            "spread-call",
            "unknown-spread-call",
            "local-spread-call",
            "proc-spread-call",
            "core-spread-call"
          ]
          .contains(&name)
          {
            "E_CALL_ARGUMENT_UNPROVEN"
          } else {
            "E_FN_RETURN_UNPROVEN"
          },
          "{name}: {report}"
        );
        let suggestion = &report["data"]["suggestions"][0];
        assert_eq!(suggestion["rule_id"], "concrete-return-proof-v1");
        assert_eq!(suggestion["definition"], target);
        if matches!(
          name,
          "open-result" | "open-call" | "local-call" | "proc-call" | "method-call" | "postfix-call"
        ) {
          let evidence = report["diagnostics"][0]["provenance"]
            .as_array()
            .expect("shared relation provenance");
          assert!(!evidence.is_empty(), "{name}: {report}");
          assert!(evidence.iter().any(|item| item["definition"] == target), "{name}: {report}");
        }
        assert_eq!(suggestion["applicability"], "requires-review");
        assert!(suggestion["replacement"].is_null());
        let edn = run_fix(
          &snapshot,
          &[
            "--rule",
            "concrete-return-proof-v1",
            "--ns",
            "fix-command.returns",
            "--def",
            name,
            "--format",
            "edn",
          ],
        );
        assert!(!edn.status.success());
        cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one Cirru EDN return-proof report");
        let applied = run_fix(
          &snapshot,
          &[
            selectors.as_slice(),
            &["--def", name, "--apply", "--expect-revision", report["revision"].as_str().unwrap()],
          ]
          .concat(),
        );
        assert!(!applied.status.success());
        assert_eq!(parse_stdout(&applied)["data"]["changed"], false);
      }
    } else {
      assert!(report["data"]["suggestions"].as_array().unwrap().is_empty());
    }
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.returns/closed-call",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'String) (:return 'Number)",
      ],
    ),
    "create a definite call contradiction",
  );
  let original = fs::read(&snapshot).unwrap();
  let mismatch = run_fix(
    &snapshot,
    &[
      "--rule",
      "concrete-return-proof-v1",
      "--ns",
      "fix-command.returns",
      "--def",
      "closed-call",
      "--format",
      "edn",
    ],
  );
  assert!(!mismatch.status.success());
  assert!(String::from_utf8_lossy(&mismatch.stderr).contains("E_CALL_ARGUMENT_MISMATCH"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.returns/closed-call",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn closed-call (x) $ cond (true (sink x))",
      ],
    ),
    "retain runtime contradictions after macro expansion",
  );
  let original = fs::read(&snapshot).unwrap();
  let mismatch = run_fix(
    &snapshot,
    &[
      "--rule",
      "concrete-return-proof-v1",
      "--ns",
      "fix-command.returns",
      "--def",
      "closed-call",
      "--format",
      "edn",
    ],
  );
  assert!(!mismatch.status.success());
  assert!(String::from_utf8_lossy(&mismatch.stderr).contains("E_CALL_ARGUMENT_MISMATCH"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);
}

#[test]
fn concrete_return_proof_rejects_a_known_spread_argument_contradiction() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(&snapshot, &["edit", "add-ns", "fix-command.spread"]),
    "add spread owner",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.spread/bad",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn bad () $ &+ 1 & ([] |wrong)",
      ],
    ),
    "add contradictory spread",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.spread/bad",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
    ),
    "declare spread return",
  );
  let original = fs::read(&snapshot).unwrap();
  let output = run_fix(
    &snapshot,
    &[
      "--rule",
      "concrete-return-proof-v1",
      "--ns",
      "fix-command.spread",
      "--def",
      "bad",
      "--format",
      "edn",
    ],
  );
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("E_CALL_ARGUMENT_MISMATCH"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);
}

#[test]
fn assertion_proof_error_does_not_grant_ffi_to_the_next_definition() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy feature isolation fixture");
  assert_success(
    &run_calcit(&snapshot, &["edit", "add-ns", "fix-command.features"]),
    "add isolated source owners",
  );
  for (target, source, schema) in [
    (
      "fix-command.features/a-return",
      "quote $ defn a-return (x) , x",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Number) (:features $ #{} :js-ffi)",
    ),
    (
      "fix-command.features/b-unscoped",
      "quote $ def b-unscoped $ unsafe-coerce 1 'Number",
      "quote 'Number",
    ),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target, "--input-format", "cirru", "--code", source]),
      "add permission isolation source",
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", target, "--input-format", "cirru", "--code", schema]),
      "declare each definition's lexical permissions",
    );
  }
  let original = fs::read(&snapshot).unwrap();
  let output = run_fix(
    &snapshot,
    &["--rule", "assert-type-proof-v1", "--ns", "fix-command.features", "--format", "edn"],
  );
  assert!(!output.status.success());
  let error = String::from_utf8_lossy(&output.stderr);
  assert!(error.contains("E_UNSCOPED_UNSAFE_COERCE"), "{error}");
  assert!(error.contains("fix-command.features/b-unscoped"), "{error}");
  assert_eq!(fs::read(&snapshot).unwrap(), original);
}

#[test]
fn assertion_proof_fix_reports_original_evidence_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  for (name, body) in [
    ("source", ", x"),
    ("helper", "source x"),
    ("recursive-helper", "let ((alias $ recursive-helper x)) , alias"),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (x)\n  {body}"),
        ],
      ),
      "add provenance helper",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
        ],
      ),
      "declare open helper contract",
    );
  }
  for (name, source, argument, diagnostic) in [
    ("open", "assert-type x Number", "Dynamic", "E_ASSERT_TYPE_UNPROVEN"),
    ("generic", "assert-type x Number", "T", "E_ASSERT_TYPE_UNPROVEN"),
    (
      "erased-callable",
      "assert-type x $ :: Fn $ {} (:args $ [] Number) (:return Number)",
      "Fn",
      "E_ASSERT_TYPE_UNPROVEN",
    ),
    ("contradiction", "assert-type x Number", "String", "E_ASSERT_TYPE_MISMATCH"),
    (
      "circular",
      "assert-type (assert-type x Number) Number",
      "Dynamic",
      "E_ASSERT_TYPE_UNPROVEN",
    ),
    (
      "helper-chain",
      "let ((result $ helper x)) (assert-type result Number)",
      "Dynamic",
      "E_ASSERT_TYPE_UNPROVEN",
    ),
    (
      "shadow-chain",
      "let ((x $ helper 1)) (assert-type x Number)",
      "Number",
      "E_ASSERT_TYPE_UNPROVEN",
    ),
    (
      "restored-scope",
      "do (let ((x 1)) , x) (assert-type x Number)",
      "Dynamic",
      "E_ASSERT_TYPE_UNPROVEN",
    ),
    (
      "recursive-chain",
      "assert-type (recursive-helper x) Number",
      "Dynamic",
      "E_ASSERT_TYPE_UNPROVEN",
    ),
  ] {
    let target = format!("fix-command.main/{name}");
    let generics = if argument == "T" { " (:generics $ [] 'T)" } else { "" };
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (x)\n  {source}"),
        ],
      ),
      "add assertion source",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ [] '{argument}) (:return 'Number){generics}"),
        ],
      ),
      "add assertion contract",
    );
    let original = fs::read(&snapshot).unwrap();
    let args = [
      "--rule",
      "assert-type-proof-v1",
      "--ns",
      "fix-command.main",
      "--def",
      name,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &args);
    assert!(!preview.status.success());
    let report = parse_stdout(&preview);
    assert_eq!(report["diagnostics"][0]["code"], diagnostic, "{report}");
    let suggestion = &report["data"]["suggestions"][0];
    assert_eq!(suggestion["rule_id"], "assert-type-proof-v1");
    assert_eq!(suggestion["diagnostic_code"], diagnostic);
    assert_eq!(suggestion["definition"], target);
    assert_eq!(suggestion["applicability"], "requires-review");
    assert!(suggestion["replacement"].is_null());
    assert!(suggestion["fingerprint"].is_string());
    if diagnostic == "E_ASSERT_TYPE_UNPROVEN" {
      let evidence = report["diagnostics"][0]["provenance"].as_array().expect("bounded proof evidence");
      assert!(!evidence.is_empty(), "{name}: {report}");
      assert!(evidence.len() <= 16, "{name}: {report}");
      if matches!(name, "helper-chain" | "shadow-chain") {
        for owner in ["fix-command.main/helper", "fix-command.main/source"] {
          assert!(
            evidence
              .iter()
              .any(|item| item["operation"] == owner && item["kind"] == "resolved-producer"),
            "{name}: {report}"
          );
        }
      }
      if name == "restored-scope" {
        assert!(
          evidence.iter().all(|item| item["type"] != ":number"),
          "inner binding must not leak: {report}"
        );
      }
      assert_eq!(
        suggestion["origin_chain"][0]["diagnostic"]["provenance"],
        report["diagnostics"][0]["provenance"]
      );
    }
    let mut apply = args.to_vec();
    apply.extend(["--apply", "--expect-revision", report["revision"].as_str().unwrap()]);
    let applied = run_fix(&snapshot, &apply);
    assert!(!applied.status.success());
    assert_eq!(parse_stdout(&applied)["data"]["changed"], false);
    assert_eq!(fs::read(&snapshot).unwrap(), original);
    let mut edn = args;
    edn[7] = "edn";
    let output = run_fix(&snapshot, &edn);
    assert!(!output.status.success());
    cirru_edn::parse(&String::from_utf8_lossy(&output.stdout)).expect("one EDN review report");
  }
}

#[test]
fn assertion_proof_fix_does_not_borrow_a_producers_declared_return() {
  for (producer_body, return_type, asserted_type, expected_path) in [
    (", x", "'Number", "Number", "code@3"),
    ("&{}", "(:: 'Map 'Tag 'Dynamic)", "(:: Map Tag Dynamic)", "code"),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.path().join("calcit.cirru");
    fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
    assert_success(
      &run_calcit(&snapshot, &["edit", "add-ns", "fix-command.proof"]),
      "add isolated proof owner",
    );
    for (name, body) in [
      ("open-producer", producer_body.to_owned()),
      ("assert-consumer", format!("assert-type (open-producer x) {asserted_type}")),
    ] {
      let target = format!("fix-command.proof/{name}");
      assert_success(
        &run_calcit(
          &snapshot,
          &[
            "edit",
            "def",
            &target,
            "--input-format",
            "cirru",
            "--code",
            &format!("quote $ defn {name} (x)\n  {body}"),
          ],
        ),
        "add independent producer proof fixture",
      );
      assert_success(
        &run_calcit(
          &snapshot,
          &[
            "edit",
            "schema",
            &target,
            "--input-format",
            "cirru",
            "--code",
            &format!("quote $ :: 'Fn $ {{}} (:args $ [] 'Dynamic) (:return {return_type})"),
          ],
        ),
        "declare unproven producer and consumer",
      );
    }
    let original = fs::read(&snapshot).unwrap();
    let preview = run_fix(
      &snapshot,
      &["--rule", "assert-type-proof-v1", "--ns", "fix-command.proof", "--format", "json"],
    );
    assert!(
      !preview.status.success(),
      "a producer declaration is not proof of its implementation"
    );
    let report = parse_stdout(&preview);
    assert_eq!(report["diagnostics"][0]["code"], "E_FN_RETURN_UNPROVEN", "{report}");
    assert_eq!(
      report["data"]["suggestions"][0]["definition"], "fix-command.proof/open-producer",
      "{report}"
    );
    assert_eq!(report["data"]["suggestions"][0]["path"], expected_path, "{report}");
    let navigation = run_calcit(
      &snapshot,
      &[
        "query",
        "type-at",
        "fix-command.proof/open-producer",
        "--path",
        expected_path,
        "--format",
        "json",
      ],
    );
    assert_success(&navigation, "navigate to the reported lowered-tail source owner");
    assert_eq!(parse_stdout(&navigation)["data"]["path"], expected_path);
    assert!(report["data"]["suggestions"][0]["replacement"].is_null());
    assert_eq!(fs::read(&snapshot).unwrap(), original);
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/main!",
          "--input-format",
          "cirru",
          "--code",
          "quote $ defn main! () (fix-command.proof/assert-consumer 3) &unit",
          "--overwrite",
        ],
      ),
      "make strict entry preflight warm the producer cache",
    );
    let cached_source = fs::read(&snapshot).unwrap();
    let cached = run_calcit(
      &snapshot,
      &[
        "--strict-types",
        "fix",
        "--rule",
        "assert-type-proof-v1",
        "--ns",
        "fix-command.proof",
        "--def",
        "assert-consumer",
        "--format",
        "json",
      ],
    );
    assert!(!cached.status.success(), "a cached producer cannot yield a clear audit");
    assert!(String::from_utf8_lossy(&cached.stderr).contains("E_FN_RETURN_UNPROVEN"));
    assert_eq!(fs::read(&snapshot).unwrap(), cached_source);
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          "fix-command.proof/open-producer",
          "--input-format",
          "cirru",
          "--code",
          "quote $ defn open-producer (x)\n  , |wrong",
          "--overwrite",
        ],
      ),
      "make the producer implementation contradict its declaration",
    );
    let contradictory_source = fs::read(&snapshot).unwrap();
    for rule in ["assert-type-proof-v1", "concrete-return-proof-v1"] {
      let rejected = run_fix(&snapshot, &["--rule", rule, "--ns", "fix-command.proof", "--format", "json"]);
      assert!(!rejected.status.success(), "a contradictory producer cannot yield a clear audit");
      assert!(String::from_utf8_lossy(&rejected.stderr).contains("W_FN_RETURN_TYPE_MISMATCH"));
      assert_eq!(fs::read(&snapshot).unwrap(), contradictory_source);
    }
  }
}

#[test]
fn assertion_proof_fix_accepts_independently_proven_values() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  let target = "fix-command.main/proven-assertion";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn proven-assertion (x)\n  assert-type x Number",
      ],
    ),
    "add proven source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
    ),
    "add proven contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "identity",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= 3 $ proven-assertion 3",
      ],
    ),
    "attach Calcit identity semantics",
  );
  let original = fs::read(&snapshot).unwrap();
  let output = run_fix(
    &snapshot,
    &[
      "--rule",
      "assert-type-proof-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "proven-assertion",
      "--format",
      "json",
    ],
  );
  assert_success(&output, "independent Number evidence");
  assert_eq!(parse_stdout(&output)["data"]["suggestions"], serde_json::json!([]));
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit assertion semantics",
  );
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  for (name, annotation, generics) in [("generic-identity", "T", " (:generics $ [] 'T)"), ("open-identity", "Dynamic", "")] {
    let target = format!("fix-command.main/{name}");
    let asserted_type = if annotation == "T" { "'T" } else { annotation };
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (x)\n  assert-type x {asserted_type}"),
        ],
      ),
      "add identity assertion",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ [] '{annotation}) (:return '{annotation}){generics}"),
        ],
      ),
      "add identity signature",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-test",
          &target,
          "identity",
          "--tags",
          "unit",
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ assert= 3 $ {name} 3"),
        ],
      ),
      "attach generic/open Calcit semantics",
    );
    let original = fs::read(&snapshot).unwrap();
    let output = run_fix(
      &snapshot,
      &[
        "--rule",
        "assert-type-proof-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&output, "a legitimate identity must not require narrowing");
    assert_eq!(parse_stdout(&output)["data"]["suggestions"], serde_json::json!([]));
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "identity semantic replay",
    );
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }
}

#[test]
fn type_at_next_navigation_executes_at_root_and_nested_paths() {
  let directory = TestDirectory::create();
  let snapshot_directory = directory.path().join("snapshot with ' quote");
  fs::create_dir(&snapshot_directory).expect("quoted project directory");
  let snapshot = snapshot_directory.join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  let original = fs::read(&snapshot).unwrap();
  for path in ["code", "code@2", "code@3"] {
    let query = run_calcit(
      &snapshot,
      &["query", "type-at", "fix-command.main/main!", "--path", path, "--format", "json"],
    );
    assert_success(&query, "query source navigation");
    let report = parse_stdout(&query);
    assert_eq!(report["data"]["path"], path);
    let command = report["next"][0].as_str().unwrap();
    if path == "code" {
      assert!(!command.contains("--path"), "{command}");
    } else {
      assert!(
        command.contains(&format!("--path '{}'", path.strip_prefix("code").unwrap())),
        "{command}"
      );
    }
    let binary_directory = Path::new(env!("CARGO_BIN_EXE_calcit")).parent().unwrap();
    let navigation = Command::new("sh")
      .args(["-c", &format!("{command} --json --raw --depth 0")])
      .current_dir(directory.path())
      .env(
        "PATH",
        format!("{}:{}", binary_directory.display(), std::env::var("PATH").unwrap_or_default()),
      )
      .output()
      .expect("execute the actual suggested command without splitting shell quoting");
    assert_success(&navigation, "execute query next command from another working directory");
    let stdout = String::from_utf8(navigation.stdout).unwrap();
    if report["data"]["tree"].is_array() {
      let json = stdout
        .split("```json\n")
        .nth(1)
        .expect("tree JSON fence")
        .split("```")
        .next()
        .unwrap();
      let tree: serde_json::Value = serde_json::from_str(json.trim()).expect("tree AST");
      assert_eq!(tree, report["data"]["tree"]);
    } else {
      let leaf = stdout
        .split("```cirru\n")
        .nth(1)
        .expect("leaf Cirru fence")
        .split("```")
        .next()
        .unwrap();
      assert_eq!(leaf.trim(), report["data"]["tree"].as_str().unwrap());
    }
    assert_eq!(fs::read(&snapshot).unwrap(), original);
    let edn = run_calcit(
      &snapshot,
      &["query", "type-at", "fix-command.main/main!", "--path", path, "--format", "edn"],
    );
    assert_success(&edn, "EDN navigation contract");
    let cirru_edn::Edn::Map(envelope) = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).unwrap() else {
      panic!("EDN query envelope");
    };
    let expected_next = cirru_edn::Edn::List(cirru_edn::EdnListView(
      report["next"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| cirru_edn::Edn::str(command.as_str().unwrap()))
        .collect(),
    ));
    assert_eq!(envelope.get(&cirru_edn::Edn::tag("next")), Some(&expected_next));
    let human = run_calcit(&snapshot, &["query", "type-at", "fix-command.main/main!", "--path", path]);
    assert_success(&human, "human navigation contract");
    assert!(String::from_utf8_lossy(&human.stdout).contains(command));
  }
  let invalid = run_calcit(
    &snapshot,
    &[
      "query",
      "type-at",
      "fix-command.main/main!",
      "--path",
      "code@999",
      "--format",
      "edn",
    ],
  );
  assert!(!invalid.status.success());
  assert_eq!(fs::read(&snapshot).unwrap(), original);
}

#[test]
fn unsafe_boundary_fix_preserves_compiler_rejection_and_source_bytes() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  for (name, source, features) in [
    ("unscoped", "quote $ defn unscoped (x)\n  unsafe-coerce x 'Number", ""),
    (
      "nested",
      "quote $ defn nested (x)\n  let\n      f $ fn (y)\n        unsafe-coerce y 'Number\n    f x",
      "",
    ),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", source]),
      "add boundary source",
    );
    let schema = format!("quote $ :: 'Fn $ {{}} (:args $ [] 'Number) (:return 'Number) {features}");
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "schema", &target, "--input-format", "cirru", "--code", &schema],
      ),
      "add boundary schema",
    );
    let original = fs::read(&snapshot).unwrap();
    let selector = [
      "--rule",
      "unsafe-coerce-boundary-v1",
      "--ns",
      "fix-command.main",
      "--def",
      name,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &selector);
    assert!(!preview.status.success(), "a plan must not downgrade the compiler error");
    let report = parse_stdout(&preview);
    assert_eq!(report["data"]["validation"]["status"], "requires-review", "{report}");
    assert_eq!(report["diagnostics"][0]["code"], "E_UNSCOPED_UNSAFE_COERCE", "{report}");
    assert_eq!(report["diagnostics"][0]["severity"], "error");
    let suggestions = report["data"]["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), 1, "{report}");
    assert_eq!(suggestions[0]["definition"], target);
    assert_eq!(suggestions[0]["applicability"], "requires-review");
    assert!(suggestions[0]["replacement"].is_null());
    assert!(suggestions[0]["path"].as_str().unwrap().starts_with("code@3"), "{report}");
    assert!(suggestions[0]["message"].as_str().unwrap().contains("first compiler error"));
    assert!(suggestions[0]["origin_chain"][0]["lexical_source_path"].is_string());
    let mut edn_selector = selector;
    edn_selector[7] = "edn";
    let edn = run_fix(&snapshot, &edn_selector);
    assert!(!edn.status.success());
    cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("one EDN error plan");
    let mut human_selector = selector;
    human_selector[7] = "human";
    let human = run_fix(&snapshot, &human_selector);
    assert!(!human.status.success());
    let human_text = String::from_utf8_lossy(&human.stdout);
    assert!(human_text.contains("# Compiler-guided source fixes"));
    assert!(human_text.contains("requires-review"));
    let mut apply = selector.to_vec();
    apply.extend(["--apply", "--expect-revision", report["revision"].as_str().unwrap()]);
    let applied = run_fix(&snapshot, &apply);
    assert!(!applied.status.success());
    assert_eq!(parse_stdout(&applied)["data"]["changed"], false);
    assert_eq!(fs::read(&snapshot).unwrap(), original);
    assert_eq!(parse_stdout(&run_fix(&snapshot, &selector))["revision"], report["revision"]);
    let mut stale = selector.to_vec();
    stale.extend(["--apply", "--expect-revision", "stale"]);
    assert!(!run_fix(&snapshot, &stale).status.success());
    assert_eq!(fs::read(&snapshot).unwrap(), original);
  }
}

#[test]
fn unsafe_boundary_fix_accepts_explicit_adapters_and_rejects_unrelated_errors() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  let target = "fix-command.main/adapter";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn adapter (x)\n  unsafe-coerce x 'Number",
      ],
    ),
    "add explicit adapter",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number) (:features $ #{} :js-ffi)",
      ],
    ),
    "mark lexical adapter",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "identity",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= 3 $ adapter 3",
      ],
    ),
    "attach adapter semantics",
  );
  let original = fs::read(&snapshot).unwrap();
  let selector = [
    "--rule",
    "unsafe-coerce-boundary-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "adapter",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "proven lexical adapter");
  assert_eq!(parse_stdout(&preview)["data"]["suggestions"], serde_json::json!([]));
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "adapter semantic replay",
  );
  assert_eq!(fs::read(&snapshot).unwrap(), original);

  let compat = Command::new(env!("CARGO_BIN_EXE_calcit"))
    .arg("--compat-types")
    .arg(&snapshot)
    .arg("fix")
    .args(selector)
    .output()
    .expect("compatibility-mode preview");
  assert!(!compat.status.success());
  assert!(String::from_utf8_lossy(&compat.stderr).contains("requires strict types"));
  assert_eq!(fs::read(&snapshot).unwrap(), original);

  assert_success(
    &run_calcit(&snapshot, &["edit", "add-ns", "fix-command.boundary"]),
    "add independent owner",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.boundary/raw",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn raw (x)\n  unsafe-coerce x 'Number",
      ],
    ),
    "add unscoped independent definition",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.boundary/raw",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
    ),
    "add independent schema",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn adapter (x)\n  fix-command.boundary/raw x",
      ],
    ),
    "reference independent owner from privileged adapter",
  );
  let original = fs::read(&snapshot).unwrap();
  let outside = run_fix(&snapshot, &selector);
  assert!(!outside.status.success());
  assert!(String::from_utf8_lossy(&outside.stderr).contains("outside the selected project scope"));
  assert!(outside.stdout.is_empty());
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  let owned = run_fix(
    &snapshot,
    &[
      "--rule",
      "unsafe-coerce-boundary-v1",
      "--ns",
      "fix-command.boundary",
      "--def",
      "raw",
      "--format",
      "json",
    ],
  );
  assert!(!owned.status.success());
  assert_eq!(
    parse_stdout(&owned)["data"]["suggestions"][0]["definition"],
    "fix-command.boundary/raw"
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn adapter (x)\n  assert-type 1 String",
        "--overwrite",
      ],
    ),
    "add unrelated compiler contradiction",
  );
  let original = fs::read(&snapshot).unwrap();
  let unrelated = run_fix(&snapshot, &selector);
  assert!(!unrelated.status.success());
  assert!(String::from_utf8_lossy(&unrelated.stderr).contains("E_ASSERT_TYPE_MISMATCH"));
  assert!(
    unrelated.stdout.is_empty(),
    "unrelated errors cannot become an empty successful plan"
  );
  assert_eq!(fs::read(&snapshot).unwrap(), original);
}

#[test]
fn literal_spread_fix_is_guarded_atomic_and_idempotent() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  let target = "fix-command.main/literal-spread";
  let source = "quote $ defn literal-spread ()\n  let\n      *counter $ atom 0\n      f $ fn (a b)\n        hint-fn $ {} (:args $ [] 'Number 'Number) (:return 'Number)\n        + (* a 10) b\n    f & $ []\n      do (reset! *counter $ + @*counter 1) @*counter\n      do (reset! *counter $ + @*counter 1) @*counter";
  assert_success(
    &run_calcit(&snapshot, &["edit", "def", target, "--input-format", "cirru", "--code", source]),
    "add spread source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
    ),
    "add spread schema",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "result",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= 12 $ literal-spread",
      ],
    ),
    "attach semantic test",
  );
  let selector = [
    "--rule",
    "spread-call-proof-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "literal-spread",
    "--format",
    "json",
  ];
  let original = fs::read(&snapshot).expect("read original");
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "spread preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  let mut stale = selector.to_vec();
  stale.extend(["--apply", "--allow-no-vcs", "--expect-revision", "stale"]);
  assert!(!run_fix(&snapshot, &stale).status.success());
  assert_eq!(fs::read(&snapshot).unwrap(), original);
  let mut apply = selector.to_vec();
  apply.extend([
    "--apply",
    "--allow-no-vcs",
    "--expect-revision",
    report["revision"].as_str().unwrap(),
  ]);
  assert_success(&run_fix(&snapshot, &apply), "apply spread");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "replay preserved semantic test",
  );
  let applied = fs::read(&snapshot).unwrap();
  let second = run_fix(&snapshot, &selector);
  assert_success(&second, "second preview");
  assert!(parse_stdout(&second)["data"]["suggestions"].as_array().unwrap().is_empty());
  assert_eq!(fs::read(&snapshot).unwrap(), applied);
}

#[test]
fn spread_fix_keeps_unproved_calls_for_review_without_writes() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  let cases = [
    ("unknown-length", "(xs)", "f & xs", "[] (:: 'List 'Number)"),
    ("wrong-item", "()", "f & ([] 1 |two)", "[]"),
    ("wrong-length", "()", "f & ([] 1)", "[]"),
    ("multiple-spreads", "()", "f & ([] 1) & ([] 2)", "[]"),
    ("open-item", "(x)", "f & ([] 1 x)", "[] 'Dynamic"),
  ];
  for (name, parameters, call, arguments) in cases {
    let target = format!("fix-command.main/{name}");
    let source = format!(
      "quote $ defn {name} {parameters}\n  let\n      f $ fn (a b)\n        hint-fn $ {{}} (:args $ [] 'Number 'Number) (:return 'Number)\n        + a b\n    {call}"
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", &source]),
      "add review case",
    );
    let schema = format!("quote $ :: 'Fn $ {{}} (:args $ {arguments}) (:return 'Number)");
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "schema", &target, "--input-format", "cirru", "--code", &schema],
      ),
      "add review schema",
    );
    let original = fs::read(&snapshot).unwrap();
    let result = run_fix(
      &snapshot,
      &[
        "--rule",
        "spread-call-proof-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
        "--apply",
        "--allow-no-vcs",
      ],
    );
    assert_success(&result, name);
    let report = parse_stdout(&result);
    let suggestions = report["data"]["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), 1, "{name}: {report}");
    assert_eq!(suggestions[0]["applicability"], "requires-review", "{name}: {report}");
    assert!(suggestions[0]["replacement"].is_null(), "{report}");
    assert_eq!(fs::read(&snapshot).unwrap(), original, "{name}");
  }
}

#[test]
fn spread_fix_preserves_rest_optional_and_unknown_macro_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fixture");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/spread-wrapper",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defmacro spread-wrapper (x) x",
      ],
    ),
    "add unknown macro",
  );
  for (name, source) in [
    (
      "fixed-sum",
      "quote $ defn fixed-sum (a b)\n  hint-fn $ {} (:args $ [] 'Number 'Number) (:return 'Number)\n  + a b",
    ),
    (
      "spread-syntax",
      "quote $ defmacro spread-syntax (marker literal)\n  quasiquote $ fixed-sum & ~literal",
    ),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          source,
        ],
      ),
      "add macro-headed spread fixture",
    );
  }
  let cases = [
    (
      "rest-spread",
      "quote $ defn rest-spread ()\n  let\n      f $ fn (& xs)\n        hint-fn $ {} (:args $ []) (:rest 'Number) (:return 'Number)\n        , 1\n    f & $ [] 1 2",
    ),
    (
      "optional-spread",
      "quote $ defn optional-spread ()\n  let\n      f $ fn (x)\n        hint-fn $ {} (:args $ [] (:: 'Optional 'Number)) (:return 'Number)\n        , 1\n    f & $ [] 1",
    ),
    (
      "macro-spread",
      "quote $ defn macro-spread ()\n  let\n      f $ fn (a b)\n        hint-fn $ {} (:args $ [] 'Number 'Number) (:return 'Number)\n        + a b\n    spread-wrapper $ f & $ [] 1 2",
    ),
    (
      "macro-headed-spread",
      "quote $ defn macro-headed-spread ()\n  spread-syntax & $ [] 1 2",
    ),
    (
      "qualified-macro-headed-spread",
      "quote $ defn qualified-macro-headed-spread ()\n  fix-command.main/spread-syntax & $ [] 1 2",
    ),
  ];
  for (name, source) in cases {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", source]),
      "add boundary source",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
        ],
      ),
      "add boundary schema",
    );
    if name.ends_with("macro-headed-spread") {
      let assertion = format!("quote $ assert= 3 $ {name}");
      assert_success(
        &run_calcit(
          &snapshot,
          &[
            "edit",
            "add-test",
            &target,
            "result",
            "--tags",
            "unit",
            "--input-format",
            "cirru",
            "--code",
            &assertion,
          ],
        ),
        "attach macro syntax semantic test",
      );
      assert_success(
        &run_calcit(&snapshot, &["test", &target, "--require-match"]),
        "run original macro syntax",
      );
    }
    let before = fs::read(&snapshot).unwrap();
    let result = run_fix(
      &snapshot,
      &[
        "--rule",
        "spread-call-proof-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
        "--apply",
        "--allow-no-vcs",
      ],
    );
    assert_success(&result, name);
    let report = parse_stdout(&result);
    let suggestions = report["data"]["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), 1, "{name}: {report}");
    assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
    assert!(suggestions[0]["replacement"].is_null(), "{report}");
    assert_eq!(fs::read(&snapshot).unwrap(), before);
    if name.ends_with("macro-headed-spread") {
      assert_success(
        &run_calcit(&snapshot, &["test", &target, "--require-match"]),
        "replay retained macro syntax",
      );
    }
  }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
  fn create() -> Self {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("test clock should be valid")
      .as_nanos();
    let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("calcit-fix-cli-{}-{nonce}-{counter}", std::process::id()));
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

fn run_fix(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .arg("fix")
    .args(args)
    .output()
    .expect("fix command should run")
}

fn run_fix_with_entry(snapshot: &Path, entry: &str, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg("--entry")
    .arg(entry)
    .arg(snapshot)
    .arg("fix")
    .args(args)
    .output()
    .expect("entry-scoped fix command should run")
}

fn run_calcit(snapshot: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run")
}

fn run_calcit_with_emit_path(snapshot: &Path, emit_path: &Path, args: &[&str]) -> Output {
  Command::new(env!("CARGO_BIN_EXE_calcit"))
    .env("NO_COLOR", "1")
    .arg("--tips-level")
    .arg("none")
    .arg("--emit-path")
    .arg(emit_path)
    .arg(snapshot)
    .args(args)
    .output()
    .expect("calcit command should run with an isolated emit path")
}

fn parse_stdout(output: &Output) -> serde_json::Value {
  serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
    panic!(
      "fix stdout should contain one JSON value: {error}\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    )
  })
}

fn assert_success(output: &Output, context: &str) {
  assert!(
    output.status.success(),
    "{context} failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
}

/// Verify proven core effect aliases across explicit and compact source forms.
#[test]
fn core_effect_method_fix_handles_explicit_and_compact_calls() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/legacy-effects";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn legacy-effects (path task response)\n  fn () $ path .write-text |payload\n  fn () task.cancel\n  fn () $ task.cancel-with :shutdown\n  fn () $ response.resolve :ok\n  fn () $ response.reject :error\n  , true",
      ],
    ),
    "install effect calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'FsPath 'FfiTask 'FfiResponse) (:return 'Bool)",
      ],
    ),
    "declare nominal effect receivers",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "keeps-deferred-effects",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= true $ legacy-effects (fs:path |unused) (ffi:task nil) (ffi:response nil)",
      ],
    ),
    "attach Calcit effect contract",
  );
  let selector = [
    "--rule",
    "core-effect-method-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "legacy-effects",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "effect method preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 5, "{report}");
  assert!(
    suggestions.iter().all(|item| item["applicability"] == "machine-applicable"),
    "{report}"
  );
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  for (nominal, old, preferred) in [
    ("calcit.core/FsPath", ".write-text", ".write-text!"),
    ("calcit.core/FfiTask", ".cancel", ".cancel!"),
    ("calcit.core/FfiResponse", ".resolve", ".resolve!"),
  ] {
    let queried = run_calcit(&snapshot, &["query", "type", nominal, "--format", "json"]);
    assert_success(&queried, "effect method discovery");
    let context = parse_stdout(&queried);
    let methods = context["data"]["methods"].as_array().expect("queried methods");
    let legacy = methods.iter().find(|method| method["name"] == old).expect("legacy method");
    let current = methods.iter().find(|method| method["name"] == preferred).expect("preferred method");
    assert_eq!(legacy["role"], "compatibility", "{context}");
    assert_eq!(legacy["preferred_name"], preferred, "{context}");
    assert_eq!(legacy["fix_rule"], "core-effect-method-v1", "{context}");
    assert_eq!(current["role"], "preferred", "{context}");
  }
  let replacements = suggestions
    .iter()
    .map(|item| item["replacement"]["value"].as_str().unwrap_or_default())
    .collect::<Vec<_>>();
  for expected in [
    ".write-text!",
    "task.cancel!",
    "task.cancel-with!",
    "response.resolve!",
    "response.reject!",
  ] {
    assert!(replacements.contains(&expected), "missing {expected}: {report}");
  }
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-effect-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-effects",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject effect migration");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-effect-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-effects",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "effect method apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit effect after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent effect preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

/// Keep quoted code untouched and leave macro-expanded effect calls for review.
#[test]
fn core_effect_method_fix_skips_quoted_calls_and_reviews_macro_context() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (target, source) in [
    ("fix-command.main/pass-effect", "quote $ defmacro pass-effect (body) body"),
    (
      "fix-command.main/macro-cancel",
      "quote $ defn macro-cancel (task) $ pass-effect $ task.cancel-with :reason",
    ),
    (
      "fix-command.main/macro-cancel-zero",
      "quote $ defn macro-cancel-zero (task) $ pass-effect task.cancel",
    ),
    (
      "fix-command.main/quoted-cancel",
      "quote $ defn quoted-cancel ()\n  quote $ task.cancel-with :reason\n  , true",
    ),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target, "--input-format", "cirru", "--code", source]),
      "install effect boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-cancel",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'FfiTask) (:return 'Unit)",
      ],
    ),
    "declare task receiver",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-cancel-zero",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'FfiTask) (:return 'Unit)",
      ],
    ),
    "declare zero-arg task receiver",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/quoted-cancel",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare quoted source",
  );
  let macro_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-effect-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "macro-cancel",
      "--format",
      "json",
    ],
  );
  assert_success(&macro_preview, "macro effect preview");
  let report = parse_stdout(&macro_preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
  let zero_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-effect-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "macro-cancel-zero",
      "--format",
      "json",
    ],
  );
  assert_success(&zero_preview, "zero-arg macro effect preview");
  let zero_report = parse_stdout(&zero_preview);
  let zero_suggestions = zero_report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(zero_suggestions.len(), 1, "{zero_report}");
  assert_eq!(zero_suggestions[0]["applicability"], "requires-review", "{zero_report}");
  let quoted_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-effect-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "quoted-cancel",
      "--format",
      "json",
    ],
  );
  assert_success(&quoted_preview, "quoted effect preview");
  assert_eq!(parse_stdout(&quoted_preview)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn list_fold_fix_preserves_seeded_method_semantics_and_revision_guard() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/fold-values";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn fold-values (xs)\n  xs .reduce |n $ fn (acc item) (str acc |: item)",
      ],
    ),
    "install seeded reduce method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return 'String)",
      ],
    ),
    "declare a concrete list receiver",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-order-and-empty-seed",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ do\n  assert= |n:1:2:3 $ fold-values ([] 1 2 3)\n  assert= |n $ fold-values ([]) ",
      ],
    ),
    "attach Calcit fold contract",
  );
  let selector = [
    "--rule",
    "core-list-fold-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "fold-values",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "fold preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-fold-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "fold-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-fold-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "fold-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "fold apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit fold after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent fold preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn list_fold_fix_preserves_quoted_and_macro_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("quoted-fold", "quote $ defn quoted-fold ()\n  quote $ ([] 1 2) .reduce 0 +\n  , 0"),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-fold", "quote $ defn macro-fold (xs) $ pass-form $ xs .reduce 0 +"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install fold boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-fold",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return 'Number)",
      ],
    ),
    "declare macro fold receiver",
  );
  for (name, expected) in [("quoted-fold", 0), ("macro-fold", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-list-fold-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "fold boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if expected == 1 {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
}

#[test]
fn list_intersperse_fix_preserves_separator_semantics_and_revision_guard() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/separator-values";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn separator-values (xs) (xs .join 0)",
      ],
    ),
    "install List separator method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return $ :: 'List 'Number)",
      ],
    ),
    "declare List separator contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-duplicates-empty-and-singleton",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ do\n  assert= ([] 1 0 1 0 2) $ separator-values ([] 1 1 2)\n  assert= ([]) $ separator-values ([])\n  assert= ([] 7) $ separator-values ([] 7)",
      ],
    ),
    "attach Calcit separator contract",
  );
  let selector = [
    "--rule",
    "core-list-intersperse-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "separator-values",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "intersperse preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-intersperse-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "separator-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-intersperse-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "separator-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "intersperse apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit separator after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent intersperse preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn list_intersperse_fix_preserves_quoted_and_macro_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("quoted-join", "quote $ defn quoted-join ()\n  quote $ ([] 1 2) .join 0\n  , 0"),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-join", "quote $ defn macro-join (xs) $ pass-form $ xs .join 0"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install separator boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-join",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return $ :: 'List 'Number)",
      ],
    ),
    "declare macro separator receiver",
  );
  for (name, expected) in [("quoted-join", 0), ("macro-join", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-list-intersperse-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "separator boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if expected == 1 {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
}

#[test]
fn list_flat_map_fix_preserves_typed_output_and_revision_guard() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/duplicate-as-strings";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn duplicate-as-strings (xs) (xs .bind $ fn (x) ([] (str x) (str x)))",
      ],
    ),
    "install List bind method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return $ :: 'List 'String)",
      ],
    ),
    "declare List flat-map contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-order-and-empty",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ do\n  assert= ([] |1 |1 |2 |2) $ duplicate-as-strings $ [] 1 2\n  assert= ([]) $ duplicate-as-strings ([])",
      ],
    ),
    "attach Calcit flat-map contract",
  );
  let selector = [
    "--rule",
    "core-list-flat-map-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "duplicate-as-strings",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "flat-map preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-flat-map-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "duplicate-as-strings",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-flat-map-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "duplicate-as-strings",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "flat-map apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit flat-map after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent flat-map preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn list_flat_map_fix_keeps_quoted_and_unknown_macro_calls_unmodified() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    (
      "quoted-bind",
      "quote $ defn quoted-bind ()\n  quote $ ([] 1 2) .bind $ fn (x) ([] x)\n  , 0",
    ),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-bind", "quote $ defn macro-bind (xs) $ pass-form $ xs .bind $ fn (x) ([] x)"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install flat-map boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-bind",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return $ :: 'List 'Number)",
      ],
    ),
    "declare macro List receiver",
  );
  for (name, expected) in [("quoted-bind", 0), ("macro-bind", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-list-flat-map-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "flat-map boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if expected == 1 {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
}

#[test]
fn list_join_string_fix_preserves_rendering_and_revision_guard() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/render-values";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn render-values (xs) (xs .join-str |,)",
      ],
    ),
    "install List string join method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return 'String)",
      ],
    ),
    "declare List string join contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-number-rendering-duplicates-and-empty",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ do\n  assert= |1,1,2 $ render-values ([] 1 1 2)\n  assert= | $ render-values ([])\n  assert= |7 $ render-values ([] 7)",
      ],
    ),
    "attach Calcit string join contract",
  );
  let selector = [
    "--rule",
    "core-list-join-string-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "render-values",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "join-string preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-join-string-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "render-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-join-string-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "render-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "join-string apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit string join after migration",
  );
  let invalid_separator = run_calcit(&snapshot, &["eval", "join-string ([] 1 2) 3"]);
  assert!(
    !invalid_separator.status.success(),
    "non-String separator must fail strict checking"
  );
  assert!(
    String::from_utf8_lossy(&invalid_separator.stderr).contains("W_FN_ARG_TYPE_MISMATCH"),
    "invalid separator should retain a type diagnostic"
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent join-string preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn list_join_string_fix_preserves_quoted_and_macro_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    (
      "quoted-join-str",
      "quote $ defn quoted-join-str ()\n  quote $ ([] 1 2) .join-str |,\n  , 0",
    ),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-join-str", "quote $ defn macro-join-str (xs) $ pass-form $ xs .join-str |,"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install string join boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-join-str",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return 'String)",
      ],
    ),
    "declare macro string join receiver",
  );
  for (name, expected) in [("quoted-join-str", 0), ("macro-join-str", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-list-join-string-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "string join boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if expected == 1 {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
}

#[test]
fn list_get_fix_preserves_option_lookup_and_revision_guard() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/read-list-index";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn read-list-index (xs index) (xs .nth index)",
      ],
    ),
    "install List nth method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] (:: 'List 'Number) 'Number) (:return $ :: 'Option 'Number)",
      ],
    ),
    "declare List index lookup contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-some-and-none",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ do\n  assert= (%some 2) $ read-list-index ([] 1 2 2) 2\n  assert= (%none) $ read-list-index ([] 1 2) -1\n  assert= (%none) $ read-list-index ([]) 0",
      ],
    ),
    "attach Calcit List lookup contract",
  );
  let selector = [
    "--rule",
    "core-list-get-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "read-list-index",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "List lookup preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(
    suggestions[0]["replacement"],
    serde_json::json!({"$type": "quote", "value": ".get"}),
    "{report}"
  );
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-get-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-list-index",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-get-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-list-index",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "List lookup apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit List lookup after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent List lookup preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn list_get_fix_preserves_non_list_quoted_and_macro_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("string-index", "quote $ defn string-index () $ |abc .nth 1"),
    (
      "quoted-list-index",
      "quote $ defn quoted-list-index ()\n  quote $ ([] 1 2) .nth 1\n  , 0",
    ),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install excluded List lookup source",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-list-get-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "excluded List lookup preview");
    assert_eq!(parse_stdout(&preview)["data"]["suggestions"], serde_json::json!([]));
  }
  for (name, code) in [
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    (
      "macro-list-index",
      "quote $ defn macro-list-index (xs index) $ pass-form $ xs .nth index",
    ),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install macro lookup boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-list-index",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] (:: 'List 'Number) 'Number) (:return $ :: 'Option 'Number)",
      ],
    ),
    "declare macro lookup receiver",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-get-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "macro-list-index",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "macro lookup preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
  assert!(suggestions[0]["replacement"].is_null(), "{report}");
}

#[test]
fn list_get_fix_keeps_user_defined_nth_method() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("IndexedBox0", "quote $ defstruct IndexedBox0 (:value 'Number)"),
    ("IndexedBoxTrait", "quote $ deftrait IndexedBoxTrait (.nth :fn)"),
    (
      "IndexedBoxImpl",
      "quote $ defimpl IndexedBoxImpl IndexedBoxTrait\n  .nth $ fn (box index)\n    %some $ &struct:get box :value",
    ),
    ("IndexedBox", "quote $ def IndexedBox $ impl-traits IndexedBox0 IndexedBoxImpl"),
    ("read-box-index", "quote $ defn read-box-index (box index) (box .nth index)"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install custom index trait boundary",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/read-box-index",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'fix-command.main/IndexedBox 'Number) (:return $ :: 'Option 'Number)",
      ],
    ),
    "declare custom index receiver",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "fix-command.main/read-box-index",
        "retains-custom-nth-behavior",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= (%some 7) $ read-box-index (%{} IndexedBox (:value 7)) 0",
      ],
    ),
    "attach custom index behavior test",
  );
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/read-box-index", "--require-match"]),
    "custom index method behavior",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-get-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-box-index",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "custom index method preview");
  assert_eq!(parse_stdout(&preview)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn map_distinct_values_fix_preserves_deduplication_and_revision_guard() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/distinct-map-values";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn distinct-map-values (xs) (xs .values)",
      ],
    ),
    "install Map values method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'Map 'Tag 'Number) (:return $ :: 'Set 'Number)",
      ],
    ),
    "declare Map values contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "deduplicates-and-handles-empty",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ do\n  assert= (#{} 1 2) $ distinct-map-values $ &{} :a 1 :b 2 :c 2\n  assert= (#{}) $ distinct-map-values $ &{}",
      ],
    ),
    "attach Calcit deduplication contract",
  );
  let selector = [
    "--rule",
    "core-map-distinct-values-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "distinct-map-values",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "distinct-values preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-map-distinct-values-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "distinct-map-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-map-distinct-values-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "distinct-map-values",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "distinct-values apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit deduplication after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "idempotent distinct-values preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn map_distinct_values_fix_respects_quoted_macro_and_open_boundaries() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    (
      "quoted-values",
      "quote $ defn quoted-values ()\n  quote $ (&{} :a 1) .values\n  , 0",
    ),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-values", "quote $ defn macro-values (xs) $ pass-form $ xs .values"),
    ("open-values", "quote $ defn open-values (xs) (xs .values)"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install Map values boundary source",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-values",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'Map 'Tag 'Number) (:return $ :: 'Set 'Number)",
      ],
    ),
    "declare the macro Map receiver",
  );
  for (name, expected) in [("quoted-values", 0), ("macro-values", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-map-distinct-values-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "Map values boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if expected == 1 {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
  let open = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-map-distinct-values-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "open-values",
      "--format",
      "json",
    ],
  );
  assert!(!open.status.success(), "strict preprocessing must reject an untyped receiver");
}

#[test]
fn collection_combine_fix_preserves_map_set_semantics_and_revision_guard() {
  for (name, code, schema, test_code, replacement) in [
    (
      "combine-maps",
      "quote $ defn combine-maps (xs ys zs) (xs .mappend ys zs)",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Map 'Tag 'Number) (:: 'Map 'Tag 'Number) (:: 'Map 'Tag 'Number)) (:return $ :: 'Map 'Tag 'Number)",
      "quote $ do\n  assert= ({} (:a 3) (:b 2)) $ combine-maps ({} (:a 1)) ({} (:b 2)) ({} (:a 3))\n  assert= ({}) $ combine-maps ({}) ({}) ({})",
      ".merge",
    ),
    (
      "combine-sets",
      "quote $ defn combine-sets (xs ys zs) (xs .mappend ys zs)",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Set 'Number) (:: 'Set 'Number) (:: 'Set 'Number)) (:return $ :: 'Set 'Number)",
      "quote $ do\n  assert= (#{} 1 2 3) $ combine-sets (#{} 1 2) (#{} 2 3) (#{} 1)\n  assert= (#{}) $ combine-sets (#{}) (#{}) (#{})",
      ".union",
    ),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.path().join("calcit.cirru");
    fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", code]),
      "install collection combine method",
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", &target, "--input-format", "cirru", "--code", schema]),
      "declare collection combine contract",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-test",
          &target,
          "preserves-combination",
          "--tags",
          "unit",
          "--input-format",
          "cirru",
          "--code",
          test_code,
        ],
      ),
      "attach Calcit combination contract",
    );
    let selector = [
      "--rule",
      "core-collection-combine-v1",
      "--ns",
      "fix-command.main",
      "--def",
      name,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &selector);
    assert_success(&preview, "collection combine preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), 1, "{report}");
    assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
    assert_eq!(
      suggestions[0]["replacement"],
      serde_json::json!({"$type": "quote", "value": replacement}),
      "{report}"
    );
    assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
    let stale = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-collection-combine-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--apply",
        "--allow-no-vcs",
        "--expect-revision",
        "stale-revision",
        "--format",
        "json",
      ],
    );
    assert!(!stale.status.success(), "stale revision must reject apply");
    let applied = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-collection-combine-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--apply",
        "--allow-no-vcs",
        "--expect-revision",
        report["revision"].as_str().expect("preview revision"),
        "--format",
        "json",
      ],
    );
    assert_success(&applied, "collection combine apply");
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "Calcit combination after migration",
    );
    let repeated = run_fix(&snapshot, &selector);
    assert_success(&repeated, "idempotent collection combine preview");
    assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
  }
}

#[test]
fn collection_combine_fix_skips_list_and_quoted_calls() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("list-combine", "quote $ defn list-combine () $ ([] 1) .mappend ([] 2)"),
    (
      "quoted-combine",
      "quote $ defn quoted-combine ()\n  quote $ ({} (:a 1)) .mappend ({} (:b 2))\n  , 0",
    ),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install excluded combination source",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-collection-combine-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "excluded combination preview");
    assert_eq!(parse_stdout(&preview)["data"]["suggestions"], serde_json::json!([]));
  }
}

#[test]
fn collection_len_fix_rewrites_only_proven_builtin_count_calls() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/count-builtins";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn count-builtins (xs m s text)\n  assert= 2 $ xs .count\n  assert= 1 $ m .count\n  assert= 2 $ s .count\n  text .count",
      ],
    ),
    "install four built-in count calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] (:: 'List 'Number) (:: 'Map 'Tag 'Number) (:: 'Set 'Number) 'String) (:return 'Number)",
      ],
    ),
    "declare concrete List, Map, Set and String receivers",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-collection-length",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= 2 $ count-builtins ([] 1 2) ({} (:a 1)) (#{} 1 2) |A😀",
      ],
    ),
    "attach Calcit collection length test",
  );
  let selector = [
    "--rule",
    "core-collection-len-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "count-builtins",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "collection length preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 4, "{report}");
  assert!(
    suggestions
      .iter()
      .all(|suggestion| suggestion["applicability"] == "machine-applicable"),
    "{report}"
  );
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");

  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-collection-len-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "count-builtins",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-collection-len-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "count-builtins",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "collection length apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit length semantics after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "collection length idempotent preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn collection_len_fix_preserves_nominal_count_and_unknown_macro_source() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("struct-count", "quote $ defn struct-count (person) person .count"),
    ("enum-count", "quote $ defn enum-count (choice) choice .count"),
    ("quoted-count", "quote $ defn quoted-count ()\n  quote $ ([] 1 2) .count\n  , 0"),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-count", "quote $ defn macro-count (xs) $ pass-form $ xs .count"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install nominal or macro count source",
    );
  }
  for (name, argument) in [
    ("struct-count", "'fix-command.main/FixPerson"),
    ("enum-count", "'fix-command.main/FixPersonChoice"),
    ("macro-count", ":: 'List 'Number"),
  ] {
    let args = if name == "macro-count" {
      format!("[] $ {argument}")
    } else {
      format!("[] {argument}")
    };
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ {args}) (:return 'Number)"),
        ],
      ),
      "declare nominal or List argument",
    );
  }
  for (name, expected) in [("struct-count", 0), ("enum-count", 0), ("quoted-count", 0), ("macro-count", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-collection-len-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "collection length boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if name == "macro-count" {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
}

#[test]
fn reader_deref_receivers_preserve_types_and_collection_migration_guards() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, initial, returns, rule, expected) in [
    ("reader-number", "3", "'Number", "core-list-add-v1", "4"),
    ("reader-list", "([] 1 2)", "(:: 'List 'Number)", "core-list-add-v1", "([] 1 2 3)"),
    ("reader-set", "(#{} 1 2)", "(:: 'Set 'Number)", "core-set-include-v1", "(#{} 1 2 3)"),
  ] {
    let target = format!("fix-command.main/{name}");
    let source = format!(
      "quote $ defn {name} () (let ((cell (atom {initial}))) (.add @cell {}))",
      if name == "reader-number" { "1" } else { "3" }
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", &source]),
      "install reader receiver",
    );
    let schema = format!("quote $ :: 'Fn $ {{}} (:args $ []) (:return {returns})");
    assert_success(
      &run_calcit(
        &snapshot,
        &["edit", "schema", &target, "--input-format", "cirru", "--code", &schema],
      ),
      "declare reader return",
    );
    let behavior = format!("quote $ assert= {expected} $ {name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-test",
          &target,
          "reader-contract",
          "--tags",
          "unit",
          "--input-format",
          "cirru",
          "--code",
          &behavior,
        ],
      ),
      "attach Calcit reader semantics",
    );
    let before = fs::read(&snapshot).expect("snapshot bytes");
    let preview = run_fix(
      &snapshot,
      &["--rule", rule, "--ns", "fix-command.main", "--def", name, "--format", "json"],
    );
    assert_success(&preview, "preview reader migration");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions");
    assert_eq!(fs::read(&snapshot).expect("snapshot bytes"), before);
    let query = run_calcit(
      &snapshot,
      &["query", "type-at", &target, "--path", "code@3.2.1", "--format", "json"],
    );
    assert_success(&query, "query expanded reader receiver");
    let evidence = parse_stdout(&query);
    assert_eq!(evidence["data"]["confidence"], "exact", "{evidence}");
    let receiver_type = evidence["data"]["inferred_type"].as_str().expect("reader payload type");
    assert!(receiver_type.contains("Number"), "{evidence}");
    if name == "reader-number" {
      assert!(suggestions.is_empty(), "{report}");
      assert_eq!(evidence["data"]["inferred_type"], "'Number", "{evidence}");
      let outer_query = run_calcit(&snapshot, &["query", "type-at", &target, "--path", "code@3.2", "--format", "json"]);
      assert_success(&outer_query, "query outer Number method");
      let outer = parse_stdout(&outer_query);
      assert_eq!(outer["data"]["inferred_type"], "'Number", "{outer}");
      assert_ne!(outer["data"]["lowering"]["lowered_head"], "calcit.core/deref", "{outer}");
      let set_preview = run_fix(
        &snapshot,
        &[
          "--rule",
          "core-set-include-v1",
          "--ns",
          "fix-command.main",
          "--def",
          name,
          "--format",
          "json",
        ],
      );
      assert_success(&set_preview, "Number is not a Set migration");
      assert!(
        parse_stdout(&set_preview)["data"]["suggestions"]
          .as_array()
          .expect("suggestions")
          .is_empty()
      );
    } else {
      assert!(
        receiver_type.contains(if name == "reader-list" { "List" } else { "Set" }),
        "{evidence}"
      );
      assert_eq!(suggestions.len(), 1, "{report}");
      assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
      let revision = report["revision"].as_str().expect("revision");
      let apply = run_fix(
        &snapshot,
        &[
          "--rule",
          rule,
          "--ns",
          "fix-command.main",
          "--def",
          name,
          "--apply",
          "--allow-no-vcs",
          "--expect-revision",
          revision,
          "--format",
          "json",
        ],
      );
      assert_success(&apply, "apply proven reader migration");
      let repeated = run_fix(
        &snapshot,
        &["--rule", rule, "--ns", "fix-command.main", "--def", name, "--format", "json"],
      );
      assert_success(&repeated, "repeat reader migration");
      assert!(
        parse_stdout(&repeated)["data"]["suggestions"]
          .as_array()
          .expect("suggestions")
          .is_empty()
      );
    }
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "run shared Calcit reader semantics",
    );
  }
}

#[test]
fn list_add_fix_rewrites_only_proven_element_append() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/list-add";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn list-add ()\n  ([] 1 2) .add 3",
      ],
    ),
    "install List element-add method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'List 'Number)",
      ],
    ),
    "declare List return type",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-element-append",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= ([] 1 2 3) $ list-add",
      ],
    ),
    "attach Calcit behavior test",
  );
  let selector = [
    "--rule",
    "core-list-add-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "list-add",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "List add migration preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(suggestions[0]["rule_id"], "core-list-add-v1");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");

  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-add-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "list-add",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "stale-revision",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-add-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "list-add",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "List add migration apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Calcit behavior after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "List add idempotent preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  let parameter_target = "fix-command.main/add-to-list";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        parameter_target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn add-to-list (xs)\n  xs .add 3",
      ],
    ),
    "install typed List parameter method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        parameter_target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return $ :: 'List 'Number)",
      ],
    ),
    "declare typed List parameter",
  );
  let parameter_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-list-add-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "add-to-list",
      "--format",
      "json",
    ],
  );
  assert_success(&parameter_preview, "typed List parameter preview");
  let parameter_report = parse_stdout(&parameter_preview);
  assert_eq!(
    parameter_report["data"]["suggestions"][0]["applicability"], "machine-applicable",
    "{parameter_report}"
  );
}

#[test]
fn list_add_fix_keeps_other_collections_and_unknown_macro_for_review() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (target, source) in [
    ("set-add", "quote $ defn set-add ()\n  (#{} 1 2) .add 3"),
    ("quoted-add", "quote $ defn quoted-add ()\n  quote $ ([] 1 2) .add 3\n  [] 1 2"),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-add", "quote $ defn macro-add () $ pass-form $ ([] 1 2) .add 3"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{target}"),
          "--input-format",
          "cirru",
          "--code",
          source,
        ],
      ),
      "install collection or macro source",
    );
  }
  for (definition, return_type) in [
    ("set-add", ":: 'Set 'Number"),
    ("quoted-add", ":: 'List 'Number"),
    ("macro-add", ":: 'List 'Number"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &format!("fix-command.main/{definition}"),
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ []) (:return $ {return_type})"),
        ],
      ),
      "declare collection return type",
    );
  }
  for (definition, expected) in [("set-add", 0), ("quoted-add", 0), ("macro-add", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-list-add-v1",
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "collection add boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{definition}: {report}");
    if definition == "macro-add" {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null(), "{report}");
    }
  }
}

#[test]
fn set_include_fix_migrates_only_proven_set_add_calls() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/set-add";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn set-add ()\n  (#{} 1 2) .add 2 3",
      ],
    ),
    "install Set add method",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Set 'Number)",
      ],
    ),
    "declare Set return type",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-set-members",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= (#{} 1 2 3) $ set-add",
      ],
    ),
    "attach Set behavior test",
  );
  let selector = [
    "--rule",
    "core-set-include-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "set-add",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "Set include migration preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 1, "{report}");
  assert_eq!(suggestions[0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(suggestions[0]["replacement"]["value"], ".include", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-set-include-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "set-add",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale source must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-set-include-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "set-add",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "Set include migration apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "Set behavior after migration",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "Set include idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  for (name, code) in [
    ("map-add", "quote $ defn map-add () $ ({} (:a 1)) .add $ [] :b 2"),
    ("quoted-set-add", "quote $ defn quoted-set-add () $ quote $ (#{} 1) .add 2"),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    ("macro-set-add", "quote $ defn macro-set-add () $ pass-form $ (#{} 1) .add 2"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install Set include negative boundary",
    );
  }
  for name in ["map-add", "quoted-set-add", "macro-set-add"] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)",
        ],
      ),
      "declare Set include boundary schema",
    );
  }
  for (name, expected) in [("map-add", 0), ("quoted-set-add", 0), ("macro-set-add", 1)] {
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-set-include-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "Set include boundary preview");
    let report = parse_stdout(&preview);
    let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), expected, "{name}: {report}");
    if name == "macro-set-add" {
      assert_eq!(suggestions[0]["applicability"], "requires-review", "{report}");
      assert!(suggestions[0]["replacement"].is_null());
    }
  }
  for (name, code) in [
    ("AddBox0", "quote $ defstruct AddBox0 (:value 'Number)"),
    ("AddBoxTrait", "quote $ deftrait AddBoxTrait (.add :fn)"),
    ("AddBoxImpl", "quote $ defimpl AddBoxImpl AddBoxTrait\n  .add $ fn (box item) true"),
    ("AddBox", "quote $ def AddBox $ impl-traits AddBox0 AddBoxImpl"),
    ("read-add-box", "quote $ defn read-add-box (box) box .add 2"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install custom add method boundary",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/read-add-box",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'fix-command.main/AddBox) (:return 'Bool)",
      ],
    ),
    "declare custom add receiver",
  );
  let custom = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-set-include-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-add-box",
      "--format",
      "json",
    ],
  );
  assert_success(&custom, "custom add method preview");
  assert_eq!(parse_stdout(&custom)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn optional_parameter_rule_reports_review_evidence_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/ambiguous",
        "--overwrite",
        "--code",
        "quote $ defn ambiguous (? value) (tuple? value)",
      ],
    ),
    "install legacy optional source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/main!",
        "--overwrite",
        "--code",
        "quote $ defn main! () (ambiguous) (ambiguous false) (ambiguous nil) &unit",
      ],
    ),
    "install direct legacy caller",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/reload!",
        "--overwrite",
        "--code",
        "quote $ defn reload! () (identity ambiguous) &unit",
      ],
    ),
    "install function-value legacy caller",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/shadowed-optional",
        "--code",
        "quote $ defn shadowed-optional () (let ((ambiguous $ fn (x) x)) (ambiguous 3))",
      ],
    ),
    "install shadowed local name",
  );
  let before = fs::read(&snapshot).expect("snapshot should be readable");
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "optional-parameters-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "ambiguous",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "optional parameter evidence preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 1);
  assert_eq!(suggestions[0]["diagnostic_code"], "E_LEGACY_OPTIONAL_PARAM");
  assert_eq!(suggestions[0]["applicability"], "needs-review");
  assert_eq!(suggestions[0]["origin_chain"][0]["parameter"], "value");
  assert_eq!(suggestions[0]["origin_chain"][0]["argument_index"], 0);
  assert_eq!(suggestions[0]["path"], "code@2.1");
  let origins = suggestions[0]["origin_chain"]
    .as_array()
    .expect("origin evidence should be an array");
  assert!(
    origins
      .iter()
      .any(|item| item["kind"] == "project-reference-scan" && item["external_consumers"] == "unproven")
  );
  assert!(origins.iter().any(|item| {
    item["kind"] == "resolved-project-reference"
      && item["definition"] == "fix-command.main/main!"
      && item["call_kind"] == "direct-call"
      && item["provided_arguments"] == 1
      && item["explicit_nil_arguments"] == serde_json::json!([0])
  }));
  assert!(origins.iter().any(|item| {
    item["kind"] == "resolved-project-reference"
      && item["definition"] == "fix-command.main/main!"
      && item["provided_arguments"] == 0
      && item["explicit_nil_arguments"] == serde_json::json!([])
  }));
  assert!(origins.iter().any(|item| {
    item["kind"] == "resolved-project-reference"
      && item["definition"] == "fix-command.main/main!"
      && item["provided_arguments"] == 1
      && item["explicit_false_arguments"] == serde_json::json!([0])
      && item["explicit_nil_arguments"] == serde_json::json!([])
  }));
  assert!(
    !origins
      .iter()
      .any(|item| item["definition"] == "fix-command.main/shadowed-optional")
  );
  assert!(
    !origins[1]["failed_definitions"]
      .as_array()
      .expect("scan failures should be an array")
      .iter()
      .any(|item| item == "fix-command.main/shadowed-optional")
  );
  assert!(
    origins.iter().any(|item| {
      item["kind"] == "resolved-project-reference"
        && item["definition"] == "fix-command.main/reload!"
        && item["call_kind"] == "function-value"
    }),
    "expected direct function-value evidence: {origins:?}"
  );
  assert!(suggestions[0]["origin_chain"][0]["candidate_type"].is_null());
  assert!(suggestions[0]["origin_chain"][0]["candidate_fn_schema_edn"].is_null());
  assert!(suggestions[0]["replacement"].is_null());
  assert_eq!(fs::read(&snapshot).expect("snapshot should remain readable"), before);

  let apply = run_fix(
    &snapshot,
    &[
      "--rule",
      "optional-parameters-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "ambiguous",
      "--apply",
      "--format",
      "json",
    ],
  );
  assert!(!apply.status.success(), "review-only rule must reject apply");
  assert!(String::from_utf8_lossy(&apply.stderr).contains("review-only"));
  assert_eq!(fs::read(&snapshot).expect("snapshot should remain readable"), before);
}

#[test]
fn optional_parameter_rule_reports_declared_type_candidate() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/ambiguous",
        "--overwrite",
        "--code",
        "quote $ defn ambiguous (? value) (tuple? value)",
      ],
    ),
    "install typed optional source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/ambiguous",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Bool)",
      ],
    ),
    "install typed optional schema",
  );
  let before = fs::read(&snapshot).expect("snapshot should be readable");
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "optional-parameters-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "ambiguous",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "typed optional parameter evidence preview");
  let report = parse_stdout(&preview);
  let suggestion = &report["data"]["suggestions"][0];
  assert_eq!(suggestion["origin_chain"][0]["declared_type"], ":number");
  assert_eq!(suggestion["origin_chain"][0]["candidate_type"], "Option<:number>");
  let candidate_schema = suggestion["origin_chain"][0]["candidate_fn_schema_edn"]
    .as_str()
    .expect("declared function should have a complete review-only schema candidate");
  assert!(candidate_schema.contains("Option"), "candidate schema: {candidate_schema}");
  assert!(candidate_schema.contains("Number"), "candidate schema: {candidate_schema}");
  assert!(candidate_schema.contains("Bool"), "candidate schema: {candidate_schema}");
  assert_eq!(suggestion["applicability"], "needs-review");
  assert_eq!(fs::read(&snapshot).expect("snapshot should remain readable"), before);
}

#[test]
fn optional_parameter_rule_omits_open_fn_candidates() {
  for (case, schema, has_slot_candidate) in [
    (
      "bare List tail",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number 'List) (:return 'Unit)",
      false,
    ),
    (
      "bare Map tail",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number 'Map) (:return 'Unit)",
      false,
    ),
    (
      "open required argument",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic 'Number) (:return 'Unit)",
      true,
    ),
    (
      "open return",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number 'Number) (:return 'Dynamic)",
      true,
    ),
  ] {
    let directory = TestDirectory::create();
    let snapshot = directory.path().join("calcit.cirru");
    fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/ambiguous",
          "--overwrite",
          "--code",
          "quote $ defn ambiguous (required ? value) &unit",
        ],
      ),
      case,
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", "fix-command.main/ambiguous", "--code", schema]),
      case,
    );
    let before = fs::read(&snapshot).expect("snapshot should be readable");
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "optional-parameters-v1",
        "--ns",
        "fix-command.main",
        "--def",
        "ambiguous",
        "--format",
        "json",
      ],
    );
    assert_success(&preview, case);
    let report = parse_stdout(&preview);
    let suggestion = &report["data"]["suggestions"][0];
    assert_eq!(
      suggestion["origin_chain"][0]["candidate_type"].is_string(),
      has_slot_candidate,
      "{case}"
    );
    assert!(suggestion["origin_chain"][0]["candidate_fn_schema_edn"].is_null(), "{case}");
    assert_eq!(suggestion["applicability"], "needs-review", "{case}");
    assert_eq!(fs::read(&snapshot).expect("snapshot should remain readable"), before, "{case}");
  }
}

#[test]
fn optional_parameter_rule_does_not_call_a_schema_alias_complete() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "def", "fix-command.main/OpenAlias", "--code", "quote $ def OpenAlias &unit"],
    ),
    "install schema-backed alias definition",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/OpenAlias",
        "--code",
        "quote $ :: 'List 'Dynamic",
      ],
    ),
    "install open alias schema",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/ambiguous",
        "--overwrite",
        "--code",
        "quote $ defn ambiguous (required ? value) &unit",
      ],
    ),
    "install optional function using alias",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/ambiguous",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'fix-command.main/OpenAlias 'Number) (:return 'Unit)",
      ],
    ),
    "install function schema with alias",
  );
  let before = fs::read(&snapshot).expect("snapshot should be readable");
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "optional-parameters-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "ambiguous",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "schema-backed alias candidate preview");
  let report = parse_stdout(&preview);
  let suggestion = &report["data"]["suggestions"][0];
  assert_eq!(suggestion["origin_chain"][0]["candidate_type"], "Option<:number>");
  assert!(suggestion["origin_chain"][0]["candidate_fn_schema_edn"].is_null());
  assert_eq!(suggestion["applicability"], "needs-review");
  assert_eq!(fs::read(&snapshot).expect("snapshot should remain readable"), before);
}

#[test]
fn optional_parameter_rule_keeps_multiple_trailing_candidates_review_only() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/ambiguous",
        "--overwrite",
        "--code",
        "quote $ defn ambiguous (url ? trace timeout) &unit",
      ],
    ),
    "install multiple trailing optional parameters",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/ambiguous",
        "--code",
        "quote $ :: 'Fn $ {} (:generics $ [] 'T) (:args $ [] 'String 'T 'Number) (:return 'Unit) (:features $ #{} :js-ffi)",
      ],
    ),
    "install complete declared function schema",
  );
  let before = fs::read(&snapshot).expect("snapshot should be readable");
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "optional-parameters-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "ambiguous",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "multiple optional parameter preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 2);
  let candidate_schema = suggestions[0]["origin_chain"][0]["candidate_fn_schema_edn"]
    .as_str()
    .expect("complete declaration should have a review-only Fn candidate");
  assert_eq!(
    candidate_schema.matches("Option").count(),
    2,
    "candidate schema: {candidate_schema}"
  );
  assert!(candidate_schema.contains("String") && candidate_schema.contains("Number") && candidate_schema.contains("Unit"));
  assert!(
    candidate_schema.contains("generics") && candidate_schema.contains("T"),
    "candidate schema: {candidate_schema}"
  );
  assert!(candidate_schema.contains("js-ffi"), "candidate schema: {candidate_schema}");
  let candidate_edn = cirru_edn::parse(candidate_schema).expect("candidate Fn schema should be valid Cirru EDN");
  calcit::snapshot::schema_edn_to_cirru(&candidate_edn).expect("candidate Fn schema should map to source syntax");
  assert_eq!(suggestions[1]["origin_chain"][0]["candidate_fn_schema_edn"], candidate_schema);
  assert!(
    suggestions
      .iter()
      .all(|suggestion| suggestion["applicability"] == "needs-review" && suggestion["replacement"].is_null())
  );
  let edn_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "optional-parameters-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "ambiguous",
      "--format",
      "edn",
    ],
  );
  assert_success(&edn_preview, "native EDN optional parameter preview");
  cirru_edn::parse(&String::from_utf8_lossy(&edn_preview.stdout)).expect("optional parameter EDN should parse");
  assert_eq!(fs::read(&snapshot).expect("snapshot should remain readable"), before);
}

#[test]
fn strict_workflow_checks_proofs_and_resumes_without_fabricating_repairs() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/wasi-command-03-exit.cirru", &snapshot).unwrap();
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/main!",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn main! () &unit",
      ],
    ),
    "use a side-effect-free entry",
  );
  for (name, body, returns) in [
    ("store-open", "([] value)", "(:: 'List 'Dynamic)"),
    ("unproven", "(assert-type value Number) value", "'Number"),
  ] {
    let target = format!("app.main/{name}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ defn {name} (value) {body}"),
        ],
      ),
      "create proof fixture",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ :: 'Fn $ {{}} (:args $ [] 'Dynamic) (:return {returns})"),
        ],
      ),
      "declare fixture boundary",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "app.main/store-open",
        "transports-open-values",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= ([] 1) $ store-open 1",
      ],
    ),
    "attach open-storage behavior",
  );
  assert_success(
    &run_calcit(&snapshot, &["test", "app.main/store-open", "--require-match"]),
    "run storage behavior",
  );
  let before = fs::read(&snapshot).unwrap();
  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(!preview.status.success());
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["workflow"]["status"], "requires-review");
  assert_eq!(report["diagnostics"].as_array().unwrap().len(), 1);
  assert_eq!(report["diagnostics"][0]["code"], "E_ASSERT_TYPE_UNPROVEN");
  assert_eq!(report["data"]["suggestions"][0]["rule_id"], "assert-type-proof-v1");
  assert!(report["data"]["suggestions"][0]["replacement"].is_null());
  let revision = report["revision"].as_str().unwrap();
  for mode in ["--apply", "--verify"] {
    let mut args = vec!["--workflow", "strict", mode, "--expect-revision", revision, "--format", "json"];
    if mode == "--apply" {
      args.push("--allow-no-vcs");
    }
    let output = run_fix(&snapshot, &args);
    assert!(!output.status.success());
    let result = parse_stdout(&output);
    assert_eq!(result["diagnostics"][0]["code"], "E_ASSERT_TYPE_UNPROVEN");
    assert_eq!(
      result["data"]["workflow"]["status"],
      if mode == "--verify" { "failed" } else { "requires-review" }
    );
    assert_eq!(fs::read(&snapshot).unwrap(), before);
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/unproven",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn unproven (value) (assert-type |wrong Number) 1",
      ],
    ),
    "introduce a contradiction rejected even during legacy evidence collection",
  );
  let contradictory_bytes = fs::read(&snapshot).unwrap();
  for format in ["edn", "json"] {
    let output = run_fix(&snapshot, &["--workflow", "strict", "--format", format]);
    assert!(!output.status.success());
    if format == "edn" {
      cirru_edn::parse(&String::from_utf8_lossy(&output.stdout)).expect("contradictions must preserve the EDN manifest");
      assert!(String::from_utf8_lossy(&output.stdout).contains("E_ASSERT_TYPE_MISMATCH"));
    } else {
      let report = parse_stdout(&output);
      assert_eq!(report["data"]["workflow"]["status"], "requires-review");
      let diagnostics = report["diagnostics"].as_array().unwrap();
      assert_eq!(diagnostics.len(), 1);
      assert_eq!(diagnostics[0]["code"], "E_ASSERT_TYPE_MISMATCH");
      assert_eq!(diagnostics[0]["definition"], "app.main/unproven");
      assert!(report["data"]["suggestions"][0]["replacement"].is_null());
    }
    assert_eq!(fs::read(&snapshot).unwrap(), contradictory_bytes);
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "app.main/unproven",
        "--overwrite",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn unproven (value) 1",
      ],
    ),
    "independently prove return without narrowing open input",
  );
  let proven = run_fix(&snapshot, &["--workflow", "strict", "--verify", "--format", "edn"]);
  assert_success(&proven, "verify repaired proof and retained open storage");
  cirru_edn::parse(&String::from_utf8_lossy(&proven.stdout)).expect("one native workflow report");

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "app.main/store-open",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return $ :: 'List 'Number)",
      ],
    ),
    "request a concrete storage contract",
  );
  let concrete = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(!concrete.status.success());
  assert!(
    parse_stdout(&concrete)["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "E_FN_RETURN_UNPROVEN")
  );
}

#[test]
fn strict_workflow_composes_a_resumable_project_manifest() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(!preview.status.success(), "legacy helpers still need return proof");
  let report = parse_stdout(&preview);
  let workflow = &report["data"]["workflow"];
  assert_eq!(workflow["workflow"], "strict-v1");
  assert_eq!(workflow["mode"], "preview");
  assert_eq!(workflow["status"], "requires-review");
  assert!(
    report["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "E_FN_RETURN_UNPROVEN")
  );
  assert_eq!(workflow["safe_fixes"]["preset"], "surface-latest-v2");
  assert!(workflow["safe_fixes"]["suggestions"].as_u64().is_some_and(|count| count > 0));
  assert_eq!(workflow["entries"][0]["name"], "default");
  assert_eq!(workflow["preflight"]["status"], "passed");
  assert!(workflow["preflight"]["tools"].as_array().is_some_and(|tools| !tools.is_empty()));
  assert_eq!(workflow["verification"]["commands"][0][0], "calcit");
  assert_eq!(workflow["verification"]["external_commands"], serde_json::json!([]));
  assert!(
    workflow["review_required"]["type_findings"]
      .as_array()
      .expect("review-required type findings should be an array")
      .iter()
      .all(|finding| matches!(
        finding["intent"].as_str(),
        Some("unresolved" | "declared-optional" | "explicit-unsafe")
      ))
  );
  assert!(
    workflow["retained_type_boundaries"]
      .as_array()
      .expect("retained type boundaries should be an array")
      .iter()
      .all(|finding| matches!(
        finding["intent"].as_str(),
        Some("intentional-js-ffi" | "intentional-type-slot-dynamic")
      ))
  );
  assert_eq!(workflow["resume"]["revision"], report["revision"]);
  assert_eq!(workflow["resume"]["apply_command"][3], "--workflow");
  assert_eq!(workflow["resume"]["apply_command"][4], "strict");

  let conflict = run_fix(
    &snapshot,
    &["--workflow", "strict", "--preset", "surface-latest-v2", "--format", "json"],
  );
  assert!(!conflict.status.success());
  assert!(String::from_utf8_lossy(&conflict.stderr).contains("project-scoped"));

  let unbound_apply = run_fix(
    &snapshot,
    &["--workflow", "strict", "--apply", "--allow-no-vcs", "--format", "json"],
  );
  assert!(!unbound_apply.status.success());
  assert!(String::from_utf8_lossy(&unbound_apply.stderr).contains("requires `--expect-revision`"));
}

#[test]
fn strict_workflow_plans_cross_namespace_macro_generated_ffi() {
  // A macro imported from another namespace generates a closure that performs a
  // typed external-object call. Strict `--check-only` inherits the macro
  // capability scope, but lenient migration planning used to drop that
  // inheritance because it is gated on strict typing. The planner then rejected
  // the project before it could produce any plan.
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-cross-ns-macro-ffi.cirru", &snapshot).expect("fixture should copy");

  let check = run_calcit(&snapshot, &["--check-only"]);
  assert_success(&check, "cross-namespace macro FFI check-only");

  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(!preview.status.success(), "untyped generated helpers still require evidence");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["workflow"]["status"], "requires-review");
  assert!(
    report["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| { item["code"] == "E_CALL_ARGUMENT_UNPROVEN" && item["definition"] == "defprobe.main/gen%" })
  );
}

#[test]
fn fix_preview_reports_malformed_external_object_trait_without_macro_capability_noise() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/util.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec!["edit", "def", "util.core/Host", "--code", "quote $ deftrait Host (:icons 'Dynamic)"],
      "valid external-object trait creation",
    ),
    (
      vec!["edit", "schema", "util.core/Host", "--code", "quote $ :: 'Trait"],
      "external-object trait schema",
    ),
    (
      vec![
        "edit",
        "ffi",
        "util.core/Host",
        "--code",
        "{} (:backend :js) (:kind :external-object)",
      ],
      "external-object trait FFI metadata",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let valid_snapshot = fs::read(&snapshot).expect("valid snapshot should read");
  let valid_preview = run_fix(&snapshot, &["--preset", "surface-latest-v2", "--format", "json"]);
  assert_success(&valid_preview, "whole-project preview with valid external-object trait");
  assert_eq!(fs::read(&snapshot).expect("valid snapshot should remain readable"), valid_snapshot);

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "util.core/Host",
        "--overwrite",
        "--code",
        "quote $ deftrait Host ((:icons 'Dynamic))",
      ],
    ),
    "malformed external-object trait setup",
  );
  let invalid_snapshot = fs::read(&snapshot).expect("invalid snapshot should read");
  let invalid_preview = run_fix(&snapshot, &["--preset", "surface-latest-v2", "--format", "json"]);
  assert!(!invalid_preview.status.success(), "malformed trait preview must fail");
  let diagnostic = String::from_utf8_lossy(&invalid_preview.stderr);
  assert!(
    diagnostic.contains("util.core/Host"),
    "diagnostic must locate the source definition: {diagnostic}"
  );
  assert!(
    diagnostic.contains("deftrait expects each method as (method type)"),
    "diagnostic must describe the bad shape: {diagnostic}"
  );
  assert!(
    diagnostic.contains("((:icons (quote Dynamic)))"),
    "diagnostic must include the offending nested method entry: {diagnostic}"
  );
  assert!(
    !diagnostic.contains("capability :log"),
    "diagnostic must not expose an unrelated macro capability: {diagnostic}"
  );
  assert_eq!(
    fs::read(&snapshot).expect("invalid snapshot should remain readable"),
    invalid_snapshot
  );
}

#[test]
fn whole_project_fix_loads_modules_from_all_entries_and_reports_missing_dependencies() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/fibo.cirru", &snapshot).expect("multi-entry fixture should copy");
  fs::copy("calcit/util.cirru", directory.path().join("util.cirru")).expect("module fixture should copy");

  assert_success(
    &run_calcit(&snapshot, &["config", "add-module", "--entry", "prime", "./missing.cirru"]),
    "unrelated entry module declaration",
  );
  assert_success(
    &run_fix(
      &snapshot,
      &["--ns", "app.main", "--preset", "surface-latest-v2", "--format", "json"],
    ),
    "scoped fix must not load an unrelated missing module",
  );
  assert_success(
    &run_calcit(&snapshot, &["config", "rm-module", "--entry", "prime", "./missing.cirru"]),
    "remove unrelated entry module declaration",
  );

  for (args, context) in [
    (
      vec!["config", "add-module", "--entry", "prime", "./util.cirru"],
      "prime entry module declaration",
    ),
    (
      vec![
        "edit",
        "add-import",
        "app.main",
        "--code",
        "quote $ util.core :refer $ make-reel-for-tag-access",
      ],
      "project import of prime-only module",
    ),
    (
      vec![
        "edit",
        "def",
        "app.main/use-prime-module",
        "--code",
        "quote $ defn use-prime-module () $ make-reel-for-tag-access",
      ],
      "definition outside the default entry closure",
    ),
    (
      vec![
        "edit",
        "schema",
        "app.main/use-prime-module",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Map 'Tag 'Number)",
      ],
      "project definition schema",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  assert_success(&run_calcit(&snapshot, &["--check-only"]), "default entry strict check");
  assert_success(
    &run_calcit(&snapshot, &["--entry", "prime", "--check-only"]),
    "prime entry strict check",
  );

  let source = fs::read(&snapshot).expect("source snapshot should read");
  let preview = run_fix(&snapshot, &["--preset", "surface-latest-v2", "--format", "json"]);
  assert_success(&preview, "whole-project fix with prime-only module");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["validation"]["status"], "passed");
  assert_eq!(fs::read(&snapshot).expect("source snapshot should remain readable"), source);

  assert_success(
    &run_calcit(&snapshot, &["config", "rm-module", "--entry", "prime", "./util.cirru"]),
    "remove intentionally missing dependency",
  );
  let missing_source = fs::read(&snapshot).expect("missing-dependency snapshot should read");
  let missing = run_fix(&snapshot, &["--preset", "surface-latest-v2", "--format", "json"]);
  assert!(!missing.status.success(), "truly missing module must fail");
  let diagnostic = String::from_utf8_lossy(&missing.stderr);
  assert!(
    diagnostic.contains("app.main/use-prime-module"),
    "diagnostic must locate the caller: {diagnostic}"
  );
  assert!(
    diagnostic.contains("util.core/make-reel-for-tag-access"),
    "diagnostic must name the missing definition: {diagnostic}"
  );
  assert!(
    diagnostic.contains("config modules --entry"),
    "diagnostic must provide a repair action: {diagnostic}"
  );
  assert_eq!(
    fs::read(&snapshot).expect("missing-dependency snapshot should remain readable"),
    missing_source
  );
}

#[test]
fn whole_project_fix_previews_node_only_definitions_without_weakening_entry_checks() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  let fixture = fs::read_to_string("calcit/fibo.cirru").expect("fixture should read");
  let fixture = fixture.replace(
    "(:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)",
    "(:init-fn 'app.main/main!) (:mode :js) (:reload-fn 'app.main/reload!) (:target :browser)",
  );
  let fixture = fixture.replace(
    "(:init-fn 'app.main/try-prime) (:mode :native) (:reload-fn 'app.main/try-prime)",
    "(:init-fn 'app.main/try-prime) (:mode :js) (:reload-fn 'app.main/try-prime) (:target :node)",
  );
  let fixture = fixture.replace(
    "        'reload! $ %{} 'CodeEntry",
    "        'NodeClock $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ deftrait NodeClock\n            .get-date $ :: 'Fn $ {} (:args $ [] 'app.main/NodeClock) (:return 'Number)\n          :examples $ []\n          :ffi $ {} (:backend :js) (:kind :external-object) (:target :node)\n            :names $ {} (:get-date |getDate)\n          :schema $ :: 'Trait\n        'node-date $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ defn node-date ()\n            do $ let\n                date $ unsafe-coerce (new js/Date) NodeClock\n              date .get-date\n          :examples $ []\n          :ffi $ {} (:backend :js) (:target :node)\n          :schema $ :: 'Fn $ {} (:args $ []) (:return 'Number) (:features $ #{} :js-ffi)\n        'reload! $ %{} 'CodeEntry",
  );
  fs::write(&snapshot, &fixture).expect("multi-target fixture should write");

  let source = fs::read(&snapshot).expect("source snapshot should read");
  let preview = run_fix(&snapshot, &["--preset", "surface-latest-v2", "--format", "json"]);
  assert_success(&preview, "whole-project multi-target fix preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["command"], "fix");
  assert!(
    report["data"]["suggestions"].as_array().is_some_and(|suggestions| suggestions
      .iter()
      .any(|suggestion| { suggestion["definition"] == "app.main/node-date" && suggestion["rule_id"] == "redundant-do-v1" })),
    "whole-project preview must visit the Node-only definition: {report}"
  );
  assert_eq!(fs::read(&snapshot).expect("source snapshot should remain readable"), source);

  let browser_scope = run_fix(&snapshot, &["--ns", "app.main", "--def", "node-date", "--format", "json"]);
  assert!(!browser_scope.status.success(), "browser-scoped node-only definition must fail");
  assert!(String::from_utf8_lossy(&browser_scope.stderr).contains("selected entry targets `browser`"));

  let server_scope = run_fix_with_entry(&snapshot, "prime", &["--ns", "app.main", "--def", "node-date", "--format", "json"]);
  assert_success(&server_scope, "server-scoped node-only definition");
}

#[test]
fn strict_workflow_applies_safe_fixes_without_claiming_unproven_contracts() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(!preview.status.success());
  let preview_report = parse_stdout(&preview);
  let revision = preview_report["revision"].as_str().expect("workflow revision should be text");

  let applied = run_fix(
    &snapshot,
    &[
      "--workflow",
      "strict",
      "--apply",
      "--expect-revision",
      revision,
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    !applied.status.success(),
    "safe migrations do not discharge unrelated proof obligations"
  );
  let applied_report = parse_stdout(&applied);
  assert_eq!(applied_report["data"]["workflow"]["status"], "requires-review");
  assert_eq!(applied_report["data"]["workflow"]["safe_fixes"]["status"], "applied");

  let verification = run_fix(&snapshot, &["--workflow", "strict", "--verify", "--format", "json"]);
  assert!(!verification.status.success());
  let verification_report = parse_stdout(&verification);
  assert_eq!(verification_report["data"]["workflow"]["status"], "failed");
  assert!(
    verification_report["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "E_FN_RETURN_UNPROVEN")
  );
  assert_eq!(verification_report["data"]["workflow"]["safe_fixes"]["status"], "clear");
  assert!(
    verification_report["data"]["workflow"]["verification"]["results"]
      .as_array()
      .is_some_and(|results| !results.is_empty() && results.iter().all(|result| result["status"] == "passed"))
  );
}

#[test]
fn staged_fix_validation_preserves_the_selected_browser_entry() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  let fixture = fs::read_to_string("tests/fixtures/fix-command.cirru").expect("fixture should read");
  let fixture = fixture.replace(":entries $ {} $ :default\n    {}", ":entries $ {}\n    :default $ {}");
  let fixture = fixture.replace(
    "      :type-slots $ {}\n  :files $ {} $ 'fix-command.main",
    "      :type-slots $ {}\n    :browser $ {} (:description |Browser) (:init-fn 'fix-command.main/main!) (:mode :js) (:reload-fn 'fix-command.main/reload!) (:target :browser)\n      :feature-policy $ {} (:js-ffi :error)\n      :modules $ []\n      :type-slots $ {}\n  :files $ {} $ 'fix-command.main",
  );
  let fixture = fixture.replace(
    "        'main! $ %{} 'CodeEntry",
    "        'BrowserElement $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ deftrait BrowserElement\n            .focus! $ :: 'Fn $ {} (:args $ [] 'fix-command.main/BrowserElement) (:return 'Unit)\n          :examples $ []\n          :ffi $ {} (:backend :js) (:kind :external-object) (:target :browser)\n            :names $ {} (:focus! |focus)\n          :schema $ :: 'Trait\n        'browser-focus! $ %{} 'CodeEntry (:doc |)\n          :code $ quote $ defn browser-focus! (element)\n            do (element .focus!) &unit\n          :examples $ []\n          :ffi $ {} (:backend :js) (:target :browser)\n          :schema $ :: 'Fn $ {} (:return 'Unit)\n            :args $ [] 'fix-command.main/BrowserElement\n            :features $ #{} :js-ffi\n        'main! $ %{} 'CodeEntry",
  );
  fs::write(&snapshot, fixture).expect("entry fixture should write");

  let preview = run_fix_with_entry(
    &snapshot,
    "browser",
    &[
      "--preset",
      "surface-latest-v2",
      "--ns",
      "fix-command.main",
      "--def",
      "browser-focus!",
      "--format",
      "json",
    ],
  );

  assert_success(&preview, "browser-entry fix preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], true);
  assert_eq!(report["data"]["validation"]["status"], "passed");
  assert_eq!(report["data"]["suggestions"][0]["rule_id"], "redundant-do-v1");
}

#[test]
fn semantic_rename_updates_only_compiler_resolved_usages_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-old",
        "--code",
        "quote $ defn rename-old (x) + x 1",
      ],
      "create rename source",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-old",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type rename source",
    ),
    (vec!["edit", "add-ns", "fix-command.other"], "create consumer namespace"),
    (
      vec![
        "edit",
        "imports",
        "fix-command.other",
        "--code",
        "quote $ [] (fix-command.main :refer $ [] rename-old) (fix-command.main :as m)",
      ],
      "create consumer imports",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.other/caller",
        "--code",
        "quote $ defn caller () [] (rename-old 1) (m/rename-old 2)",
      ],
      "create consumer",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.other/caller",
        "--code",
        "quote $ :: 'Fn $ {} (:return $ :: 'List 'Number)",
      ],
      "type consumer",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-shadow",
        "--code",
        "quote $ defn rename-shadow (rename-old) , rename-old",
      ],
      "create shadowed local",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-shadow",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "type shadowed local",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "semantic rename preview");
  let preview_report = parse_stdout(&preview);
  assert_eq!(preview_report["data"]["changed"], true);
  assert_eq!(preview_report["data"]["validation"]["checked_operations"], 4);
  assert_eq!(preview_report["data"]["suggestions"].as_array().map(Vec::len), Some(4));

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "semantic rename apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("'rename-new $ %{} 'CodeEntry"));
  assert!(updated.contains("defn rename-new (x) + x 1"));
  assert!(updated.contains("fix-command.main/rename-new 1"));
  assert!(updated.contains("m/rename-new 2"));
  assert!(updated.contains("defn rename-shadow (rename-old) rename-old"));
  assert!(!updated.contains("fix-command.main :refer"));
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after semantic rename");

  let repeated = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--format",
      "json",
    ],
  );
  assert_success(&repeated, "semantic rename repeat");
  assert_eq!(parse_stdout(&repeated)["data"]["changed"], false);
}

#[test]
fn semantic_rename_updates_attached_tests_and_examples_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-old",
        "--code",
        "quote $ defn rename-old (x) + x 1",
      ],
      "create rename source",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-old",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type rename source",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/caller",
        "--code",
        "quote $ defn caller () rename-old 1",
      ],
      "create caller",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/caller",
        "--code",
        "quote $ :: 'Fn $ {} (:return 'Number)",
      ],
      "type caller",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/caller",
        "calls-old",
        "--tags",
        "fast,semantic",
        "--code",
        "quote $ = (rename-old 1) 2",
      ],
      "attach target-using test",
    ),
    (
      vec![
        "edit",
        "add-example",
        "fix-command.main/caller",
        "--code",
        "quote $ = (rename-old 2) 3",
      ],
      "attach target-using example",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/caller",
        "shadows-old",
        "--code",
        "quote $ let\n    rename-old 1\n  = rename-old 1",
      ],
      "attach locally shadowed test",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "attached-source semantic rename preview");
  let preview_report = parse_stdout(&preview);
  assert_eq!(preview_report["data"]["validation"]["checked_operations"], 4);
  let paths = preview_report["data"]["suggestions"]
    .as_array()
    .expect("suggestions should be an array")
    .iter()
    .filter_map(|suggestion| suggestion["path"].as_str())
    .collect::<Vec<_>>();
  assert!(paths.contains(&"tests.calls-old"), "paths: {paths:?}");
  assert!(paths.contains(&"examples"), "paths: {paths:?}");

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "attached-source semantic rename apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("fix-command.main/rename-new 1"), "snapshot:\n{updated}");
  assert!(updated.contains("fix-command.main/rename-new 2"), "snapshot:\n{updated}");
  assert!(updated.contains("= rename-old 1"), "snapshot:\n{updated}");
  assert!(updated.contains(":tags $ #{} :fast :semantic"), "snapshot:\n{updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/caller", "--require-match"]),
    "renamed attached test",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["analyze", "check-examples", "--ns", "fix-command.main", "--def", "caller"],
    ),
    "renamed attached example",
  );
}

#[test]
fn semantic_rename_rejects_quoted_attached_source_without_partial_writes() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/rename-old",
        "--code",
        "quote $ defn rename-old (x) + x 1",
      ],
      "create quoted-boundary rename source",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/rename-old",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type quoted-boundary rename source",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/rename-old",
        "quoted-old-name",
        "--code",
        "quote $ quote rename-old",
      ],
      "attach quoted target name",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let before = fs::read(&snapshot).expect("snapshot should read before rejected rename");
  let rejected = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "rename-old",
      "--to",
      "rename-new",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(!rejected.status.success());
  let stderr = String::from_utf8_lossy(&rejected.stderr);
  assert!(stderr.contains("quoted-old-name"), "stderr: {stderr}");
  assert!(stderr.contains("quoted source contains"), "stderr: {stderr}");
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should read"), before);
}

#[test]
fn semantic_rename_updates_schema_type_refs_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/RenameType",
        "--code",
        "quote $ defstruct RenameType",
      ],
      "create rename type",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/typed-value",
        "--code",
        "quote $ defn typed-value (value) , 1",
      ],
      "create typed value",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/typed-value",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'RenameType) (:return 'Number)",
      ],
      "reference rename type from schema",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "RenameType",
      "--to",
      "RenamedType",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "schema semantic rename preview");
  let preview_report = parse_stdout(&preview);
  assert_eq!(preview_report["data"]["validation"]["checked_operations"], 2);
  let schema_suggestion = preview_report["data"]["suggestions"]
    .as_array()
    .and_then(|suggestions| suggestions.iter().find(|suggestion| suggestion["path"] == "schema"))
    .expect("schema rewrite should be included");
  assert_eq!(schema_suggestion["origin_chain"][0]["kind"], "resolved-schema-type");
  assert_eq!(schema_suggestion["origin_chain"][0]["occurrences"], 1);

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "RenameType",
      "--to",
      "RenamedType",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "schema semantic rename apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("defstruct RenamedType"), "snapshot:\n{updated}");
  assert!(updated.contains("fix-command.main/RenamedType"), "snapshot:\n{updated}");
  assert!(!updated.contains("'RenameType"), "snapshot:\n{updated}");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after schema rename");
}

#[test]
fn semantic_rename_updates_schema_trait_bounds_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/RenameTrait",
        "--code",
        "quote $ deftrait RenameTrait (.show :fn)",
      ],
      "create rename trait",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/trait-bound-value",
        "--code",
        "quote $ defn trait-bound-value (value) value",
      ],
      "create trait-bound value",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/trait-bound-value",
        "--code",
        "quote $ :: 'Fn $ {} (:generics $ [] 'T) (:args $ [] 'T) (:where $ {} ('T 'RenameTrait)) (:return 'T)",
      ],
      "reference rename trait from schema bound",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "RenameTrait",
      "--to",
      "RenamedTrait",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "trait-bound semantic rename apply");
  let applied_report = parse_stdout(&applied);
  assert_eq!(applied_report["data"]["validation"]["checked_operations"], 2);
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("deftrait RenamedTrait"), "snapshot:\n{updated}");
  assert!(updated.contains("'T 'RenamedTrait"), "snapshot:\n{updated}");
  assert!(!updated.contains("'T 'RenameTrait"), "snapshot:\n{updated}");
  assert_success(&run_calcit(&snapshot, &["--check-only"]), "strict check after trait-bound rename");
}

#[test]
fn value_to_zero_arg_fn_updates_resolved_reads_atomically() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/deferred-number",
        "--code",
        "quote $ def deferred-number 41",
      ],
      "create value definition",
    ),
    (
      vec!["edit", "schema", "fix-command.main/deferred-number", "--code", "quote 'Number"],
      "type value definition",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/main!",
        "--code",
        "quote $ defn main! () deferred-number",
        "--overwrite",
      ],
      "make the native and JavaScript entry read the value",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/main!",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type the value-reading entry",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/read-deferred-number",
        "--code",
        "quote $ defn read-deferred-number () + deferred-number 1",
      ],
      "create value reader",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/read-deferred-number",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type value reader",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/read-deferred-number",
        "reads-converted-value",
        "--code",
        "quote $ = deferred-number 41",
        "--tags",
        "semantic,fast",
      ],
      "attach direct value-read test",
    ),
    (
      vec![
        "edit",
        "examples",
        "fix-command.main/read-deferred-number",
        "--code",
        "quote $ + deferred-number 2",
      ],
      "attach direct value-read example",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-number",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "value-to-function preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], true);
  assert!(
    report["data"]["suggestions"]
      .as_array()
      .is_some_and(|items| items.iter().any(|item| item["origin_chain"][0]["kind"] == "resolved-value-read"))
  );

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-number",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "value-to-function apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("defn deferred-number () 41"), "snapshot:\n{updated}");
  assert!(updated.contains("+ (deferred-number) 1"), "snapshot:\n{updated}");
  assert!(updated.contains(":tags $ #{} :fast :semantic"), "snapshot:\n{updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/read-deferred-number", "--require-match"]),
    "converted attached test",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "analyze",
        "check-examples",
        "--ns",
        "fix-command.main",
        "--def",
        "read-deferred-number",
      ],
    ),
    "converted attached example",
  );
  assert_success(&run_calcit(&snapshot, &[]), "converted native entry");
  let js_output = directory.path().join("js-out");
  assert_success(
    &run_calcit_with_emit_path(&snapshot, &js_output, &["js"]),
    "converted JavaScript entry codegen",
  );
  assert!(js_output.join("fix-command.main.mjs").is_file());

  let repeated = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-number",
      "--format",
      "json",
    ],
  );
  assert_success(&repeated, "idempotent value-to-function preview");
  assert_eq!(parse_stdout(&repeated)["data"]["changed"], false);
}

#[test]
fn value_to_zero_arg_fn_rejects_quoted_references_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/deferred-data",
        "--code",
        "quote $ def deferred-data 1",
      ],
      "create quoted-boundary value",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/quoted-deferred-data",
        "--code",
        "quote $ def quoted-deferred-data $ quote deferred-data",
      ],
      "create quoted target name",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let before = fs::read(&snapshot).expect("snapshot should read before rejected refactor");
  let rejected = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-data",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(!rejected.status.success());
  assert!(String::from_utf8_lossy(&rejected.stderr).contains("quoted source contains"));
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should read"), before);
}

#[test]
fn value_to_zero_arg_fn_rejects_direct_macro_references_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/deferred-syntax-value",
        "--code",
        "quote $ def deferred-syntax-value 1",
      ],
      "create macro-referenced value",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/read-deferred-syntax",
        "--code",
        "quote $ defmacro read-deferred-syntax () deferred-syntax-value",
      ],
      "create direct macro reference",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let before = fs::read(&snapshot).expect("snapshot should read before rejected refactor");
  let rejected = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "deferred-syntax-value",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(!rejected.status.success());
  assert!(String::from_utf8_lossy(&rejected.stderr).contains("macro source may produce"));
  assert_eq!(fs::read(&snapshot).expect("rejected snapshot should read"), before);
}

#[test]
fn value_to_zero_arg_fn_wraps_reads_used_as_callees() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/stored-adder",
        "--code",
        "quote $ def stored-adder $ fn (x) + x 1",
      ],
      "create function-valued definition",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/stored-adder",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Number)",
      ],
      "type function-valued definition",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/call-stored-adder",
        "--code",
        "quote $ defn call-stored-adder () stored-adder 1",
      ],
      "create stored function caller",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/call-stored-adder",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type stored function caller",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/call-stored-adder",
        "calls-wrapped-callee",
        "--code",
        "quote $ = (call-stored-adder) 2",
      ],
      "attach caller behavior test",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "value-to-zero-arg-fn-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "stored-adder",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "callee-position value-to-function apply");
  let updated = fs::read_to_string(&snapshot).expect("updated snapshot should read");
  assert!(updated.contains("defn stored-adder ()"), "snapshot:\n{updated}");
  assert!(updated.contains("(stored-adder) 1"), "snapshot:\n{updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/call-stored-adder", "--require-match"]),
    "wrapped callee behavior",
  );
}

#[test]
fn schema_synthesis_never_uses_an_unproven_assertion_as_machine_evidence() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy synthesis proof fixture");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/asserted-number",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn asserted-number ()\n  let\n      value $ parse-cirru-edn \"|do nil\"\n    assert-type value 'Number\n    , value",
      ],
    ),
    "create independent source evidence regression",
  );
  let original = fs::read(&snapshot).expect("read original source bytes");
  let arguments = [
    "--rule",
    "synthesize-schema-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "asserted-number",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &arguments);
  assert_eq!(fs::read(&snapshot).unwrap(), original, "preview must be read-only");
  assert_success(&preview, "preview unproven candidate as review evidence");
  let report = parse_stdout(&preview);
  let suggestion = &report["data"]["suggestions"][0];
  assert_eq!(suggestion["applicability"], "needs-review", "{report}");
  assert!(
    suggestion["origin_chain"].as_array().unwrap().iter().any(|evidence| {
      evidence["kind"] == "compiler-diagnostic"
        && evidence["diagnostic"]["code"] == "E_ASSERT_TYPE_UNPROVEN"
        && evidence["diagnostic"]["location"]["ns"] == "fix-command.main"
        && evidence["diagnostic"]["location"]["def"] == "asserted-number"
        && evidence["diagnostic"]["location"]["coord"] == serde_json::json!([3, 2, 1])
    }),
    "review must locate the actual assertion, not an unrelated audit failure: {report}"
  );
  let mut apply_arguments = arguments.to_vec();
  apply_arguments.extend(["--apply", "--allow-no-vcs"]);
  let applied = run_fix(&snapshot, &apply_arguments);
  assert_success(&applied, "apply retains review-only source");
  let after_apply = fs::read(&snapshot).unwrap();
  assert_eq!(
    after_apply, original,
    "apply must not write a concrete return schema using unproven assertion metadata"
  );
}

#[test]
fn schema_synthesis_requires_independent_producer_and_coercion_evidence() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy producer proof fixture");
  for (name, source, schema) in [
    (
      "unproven-producer",
      "quote $ defn unproven-producer ()\n  parse-cirru-edn \"|do nil\"",
      Some("quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"),
    ),
    ("producer-consumer", "quote $ defn producer-consumer ()\n  unproven-producer", None),
    (
      "coerced-number",
      "quote $ defn coerced-number ()\n  unsafe-coerce (parse-cirru-edn \"|do nil\") 'Number",
      Some("quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic) (:features $ #{} :js-ffi)"),
    ),
    ("argument-proof", "quote $ defn argument-proof (value)\n  , 3", None),
    (
      "coerced-caller",
      "quote $ defn coerced-caller ()\n  argument-proof $ unsafe-coerce (parse-cirru-edn \"|do nil\") 'Number",
      Some("quote $ :: 'Fn $ {} (:args $ []) (:return 'Number) (:features $ #{} :js-ffi)"),
    ),
    ("owner-proof", "quote $ defn owner-proof (value)\n  , 3", None),
    (
      "proven-owner",
      "quote $ defn proven-owner ()\n  owner-proof 3",
      Some("quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"),
    ),
    (
      "unproven-owner",
      "quote $ defn unproven-owner ()\n  owner-proof 3\n  let\n      value $ parse-cirru-edn \"|do nil\"\n    assert-type value 'Number\n    , value",
      Some("quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"),
    ),
    (
      "zz-proven-owner",
      "quote $ defn zz-proven-owner ()\n  owner-proof 3",
      Some("quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)"),
    ),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", source]),
      "create proof chain source",
    );
    if let Some(schema) = schema {
      assert_success(
        &run_calcit(&snapshot, &["edit", "schema", &target, "--input-format", "cirru", "--code", schema]),
        "declare the original producer or explicit trusted boundary",
      );
    }
  }
  for name in ["producer-consumer", "coerced-number", "argument-proof", "owner-proof"] {
    let original = fs::read(&snapshot).unwrap();
    let arguments = [
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      name,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &arguments);
    assert_success(&preview, "review unproven producer or trusted coercion");
    let report = parse_stdout(&preview);
    assert_eq!(
      report["data"]["suggestions"][0]["applicability"], "needs-review",
      "{name}: {report}"
    );
    if name == "owner-proof" {
      assert!(
        report["data"]["suggestions"][0]["origin_chain"]
          .as_array()
          .unwrap()
          .iter()
          .any(|evidence| {
            evidence["kind"] == "compiler-diagnostic"
              && evidence["diagnostic"]["code"] == "E_ASSERT_TYPE_UNPROVEN"
              && evidence["diagnostic"]["location"]["def"] == "unproven-owner"
              && evidence["diagnostic"]["location"]["coord"] == serde_json::json!([4, 2, 1])
          }),
        "the audit must recheck a warmed call-site owner, rather than reuse trusted compiled metadata: {report}"
      );
      let evidence = report["data"]["suggestions"][0]["origin_chain"].as_array().unwrap();
      assert!(
        evidence.iter().any(|item| {
          item["kind"] == "unavailable-callsite-owners" && item["definitions"] == serde_json::json!(["fix-command.main/unproven-owner"])
        }),
        "a failed owner must not poison independently proven owners in the same audit checkpoint: {report}"
      );
      for owner in ["proven-owner", "zz-proven-owner"] {
        assert!(
          evidence.iter().any(|item| item["calls"].as_array().is_some_and(|calls| {
            calls.iter().any(|call| {
              call
                .as_str()
                .is_some_and(|path| path.starts_with(&format!("fix-command.main/{owner}@")))
            })
          })),
          "retain argument evidence before and after the rejected owner: {report}"
        );
      }
    }
    let mut apply_arguments = arguments.to_vec();
    apply_arguments.extend(["--apply", "--allow-no-vcs"]);
    assert_success(&run_fix(&snapshot, &apply_arguments), "do not write unproven schema");
    assert_eq!(fs::read(&snapshot).unwrap(), original, "{name} must retain source bytes");
  }
}

#[test]
fn schema_synthesis_keeps_proven_assertions_decoders_and_discarded_coercions() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy independent positive fixture");
  for (name, source, trusted) in [
    (
      "literal-proof",
      "quote $ defn literal-proof ()\n  let\n      value 3\n    assert-type value 'Number\n    , value",
      false,
    ),
    (
      "decoded-proof",
      "quote $ defn decoded-proof ()\n  parse-cirru-edn-as \"|do 3\" 'Number",
      false,
    ),
    (
      "discarded-coercion",
      "quote $ defn discarded-coercion ()\n  unsafe-coerce (parse-cirru-edn \"|do nil\") 'Number\n  , 3",
      true,
    ),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", source]),
      "create independently proven implementation",
    );
    if trusted {
      assert_success(
        &run_calcit(
          &snapshot,
          &[
            "edit",
            "schema",
            &target,
            "--input-format",
            "cirru",
            "--code",
            "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic) (:features $ #{} :js-ffi)",
          ],
        ),
        "keep the explicitly reviewed boundary unchanged",
      );
    }
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "add-test",
          &target,
          "returns-independent-number",
          "--tags",
          "unit",
          "--input-format",
          "cirru",
          "--code",
          &format!("quote $ assert= 3 $ {name}"),
        ],
      ),
      "attach user-visible Calcit contract",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "synthesize-schema-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "preview independent implementation");
    let report = parse_stdout(&preview);
    assert_eq!(
      report["data"]["suggestions"][0]["applicability"], "machine-applicable",
      "{name}: {report}"
    );
    assert_success(
      &run_fix(
        &snapshot,
        &[
          "--rule",
          "synthesize-schema-v1",
          "--ns",
          "fix-command.main",
          "--def",
          name,
          "--apply",
          "--allow-no-vcs",
          "--format",
          "json",
        ],
      ),
      "apply independent schema",
    );
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "run attached language contract",
    );
  }
}

#[test]
fn schema_synthesis_applies_exact_compiler_evidence_and_is_target_stable() {
  let directory = TestDirectory::create();
  let native_snapshot = directory.path().join("native.cirru");
  let js_snapshot = directory.path().join("js.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &native_snapshot).expect("fixture should copy");
  let native_source = fs::read_to_string(&native_snapshot).expect("native fixture should read");
  fs::write(&js_snapshot, native_source.replace("(:mode :native)", "(:mode :js)")).expect("JS fixture should write");

  for snapshot in [&native_snapshot, &js_snapshot] {
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/inferred-number",
          "--code",
          "quote $ defn inferred-number () + 1 2",
        ],
      ),
      "create untyped zero-argument function",
    );
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/inferred-add-one",
          "--code",
          "quote $ defn inferred-add-one (x) + x 1",
        ],
      ),
      "create untyped one-argument function",
    );
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "def",
          "fix-command.main/use-inferred-add-one",
          "--code",
          "quote $ defn use-inferred-add-one () (inferred-add-one 2)",
        ],
      ),
      "create typed callsite",
    );
    assert_success(
      &run_calcit(
        snapshot,
        &[
          "edit",
          "schema",
          "fix-command.main/use-inferred-add-one",
          "--code",
          "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
        ],
      ),
      "type callsite owner",
    );
  }

  let args = [
    "--rule",
    "synthesize-schema-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "inferred-number",
    "--format",
    "json",
  ];
  let native_preview = run_fix(&native_snapshot, &args);
  let js_preview = run_fix(&js_snapshot, &args);
  assert_success(&native_preview, "native schema preview");
  assert_success(&js_preview, "JS schema preview");
  let native_report = parse_stdout(&native_preview);
  let js_report = parse_stdout(&js_preview);
  assert_eq!(
    native_report["data"]["suggestions"][0]["replacement"],
    js_report["data"]["suggestions"][0]["replacement"]
  );
  assert_eq!(native_report["data"]["suggestions"][0]["applicability"], "machine-applicable");

  let argument_args = [
    "--rule",
    "synthesize-schema-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "inferred-add-one",
    "--format",
    "json",
  ];
  let native_argument_preview = run_fix(&native_snapshot, &argument_args);
  let js_argument_preview = run_fix(&js_snapshot, &argument_args);
  assert_success(&native_argument_preview, "native argument schema preview");
  assert_success(&js_argument_preview, "JS argument schema preview");
  let native_argument_report = parse_stdout(&native_argument_preview);
  let js_argument_report = parse_stdout(&js_argument_preview);
  assert_eq!(
    native_argument_report["data"]["suggestions"][0]["replacement"],
    js_argument_report["data"]["suggestions"][0]["replacement"]
  );
  assert!(
    native_argument_report["data"]["suggestions"][0]["origin_chain"]
      .as_array()
      .expect("origin chain should be an array")
      .iter()
      .any(|evidence| evidence["kind"] == "resolved-callsite-arguments" && evidence["slot"] == "schema.args.0"),
    "call-site proof evidence: {native_argument_report}"
  );
  assert_eq!(
    native_argument_report["data"]["suggestions"][0]["replacement"]["value"],
    serde_json::json!(["::", "'Fn", ["{}", [":return", "'Number"], [":args", ["[]", "'Number"]]]])
  );

  let applied = run_fix(
    &native_snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "inferred-number",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "apply inferred schema");
  let argument_applied = run_fix(
    &native_snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "inferred-add-one",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&argument_applied, "apply callsite-backed schema");
  let updated = fs::read_to_string(&native_snapshot).expect("updated snapshot should read");
  assert!(updated.contains("(:return 'Number)"));
  assert!(updated.contains("defn inferred-add-one (x) + x 1"));
  assert_success(
    &run_calcit(&native_snapshot, &["--check-only"]),
    "strict check after schema synthesis",
  );

  let repeated = run_fix(&native_snapshot, &args);
  assert_success(&repeated, "repeat schema synthesis");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn schema_evidence_reuses_schema_synthesis_and_reports_structural_candidates() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/evidence-number",
        "--code",
        "quote $ defn evidence-number () + 1 2",
      ],
      "create exact evidence target",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/evidence-number",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)",
      ],
      "structure exact evidence schema",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/evidence-add-one",
        "--code",
        "quote $ defn evidence-add-one (x) + x 1",
      ],
      "create usage-derived evidence target",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/evidence-add-one",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "structure usage-derived evidence schema",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-evidence-add-one",
        "--code",
        "quote $ defn use-evidence-add-one () (evidence-add-one 2)",
      ],
      "create schema evidence callsite",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/use-evidence-add-one",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type schema evidence callsite",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/conflicting-evidence",
        "--code",
        "quote $ defn conflicting-evidence (x) 1",
      ],
      "create conflicting evidence target",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/conflicting-evidence",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "structure conflicting evidence schema",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-conflicting-number",
        "--code",
        "quote $ defn use-conflicting-number () (conflicting-evidence 1)",
      ],
      "create numeric conflicting callsite",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/use-conflicting-number",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type numeric conflicting callsite",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-conflicting-string",
        "--code",
        "quote $ defn use-conflicting-string () (conflicting-evidence |x)",
      ],
      "create string conflicting callsite",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/use-conflicting-string",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
      "type string conflicting callsite",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/person-a",
        "--code",
        "quote $ defn person-a () ({} (:name |Ada) (:age 1))",
      ],
      "create first repeated map shape",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/person-a",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Map 'Tag 'Dynamic)",
      ],
      "type first repeated map shape",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/person-b",
        "--code",
        "quote $ defn person-b () ({} (:name |Bob) (:age 2))",
      ],
      "create second repeated map shape",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/person-b",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Map 'Tag 'Dynamic)",
      ],
      "type second repeated map shape",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/summarize-evidence",
        "--code",
        "quote $ defn summarize-evidence (x)\n  match x\n    (:ok value) value\n    (:err message) message",
      ],
      "create dispatch evidence",
    ),
    (
      vec![
        "edit",
        "schema",
        "fix-command.main/summarize-evidence",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
      ],
      "structure dispatch evidence schema",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let output = run_calcit(&snapshot, &["analyze", "weak-types", "--schema-evidence", "--format", "json"]);
  assert_success(&output, "schema evidence report");
  let report = parse_stdout(&output);
  assert_eq!(report["schema_version"], 8);
  assert_eq!(report["data"]["filters"]["schema_evidence"], true);
  let schemas = report["data"]["evidence"]["schema_candidates"]
    .as_array()
    .expect("schema candidates should be an array");
  assert!(
    schemas
      .iter()
      .any(|candidate| { candidate["definition"] == "fix-command.main/evidence-number" && candidate["confidence"] == "exact" })
  );
  assert!(
    schemas.iter().any(|candidate| {
      candidate["definition"] == "fix-command.main/conflicting-evidence"
        && candidate["confidence"] == "conflict"
        && candidate["evidence"]
          .as_array()
          .is_some_and(|evidence| evidence.iter().any(|item| item["kind"] == "conflicting-callsite-arguments"))
    }),
    "schema candidates: {schemas:?}"
  );
  assert!(schemas.iter().any(|candidate| {
    candidate["definition"] == "fix-command.main/evidence-add-one"
      && candidate["confidence"] == "usage-derived"
      && candidate["affected_usages"].as_array().is_some_and(|usages| {
        usages
          .iter()
          .any(|usage| usage.as_str().is_some_and(|path| path.contains("use-evidence-add-one")))
      })
  }));
  assert!(
    report["data"]["evidence"]["map_shapes"].as_array().is_some_and(|candidates| {
      candidates.iter().any(|candidate| {
        candidate["fields"]
          .as_array()
          .is_some_and(|fields| fields.iter().any(|field| field["name"] == ":name"))
          && candidate["evidence_paths"].as_array().is_some_and(|paths| paths.len() >= 2)
      })
    }),
    "map shape evidence: {}",
    report["data"]["evidence"]["map_shapes"]
  );
  assert!(
    report["data"]["evidence"]["dispatch"].as_array().is_some_and(|candidates| {
      candidates
        .iter()
        .any(|candidate| candidate["variants"] == serde_json::json!([":err", ":ok"]))
    }),
    "dispatch evidence: {}",
    report["data"]["evidence"]["dispatch"]
  );

  let summary = run_calcit(
    &snapshot,
    &["analyze", "weak-types", "--schema-evidence", "--summary-only", "--format", "json"],
  );
  assert_success(&summary, "schema evidence summary");
  let summary = parse_stdout(&summary);
  assert!(
    summary["data"]["summary"]["schema_candidates"]
      .as_u64()
      .is_some_and(|count| count > 0)
  );
  assert_eq!(summary["data"]["evidence"]["schema_candidates"], serde_json::json!([]));
  assert_eq!(summary["data"]["evidence"]["map_shapes"], serde_json::json!([]));
  assert_eq!(summary["data"]["evidence"]["dispatch"], serde_json::json!([]));

  let edn = run_calcit(
    &snapshot,
    &["analyze", "weak-types", "--schema-evidence", "--summary-only", "--format", "edn"],
  );
  assert_success(&edn, "schema evidence Cirru EDN summary");
  assert!(matches!(
    cirru_edn::parse(String::from_utf8_lossy(&edn.stdout).as_ref()),
    Ok(cirru_edn::Edn::Map(_))
  ));

  let workflow = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(
    !workflow.status.success(),
    "schema candidates do not discharge existing compiler obligations"
  );
  let workflow = parse_stdout(&workflow);
  assert_eq!(workflow["data"]["workflow"]["status"], "requires-review");
  assert!(
    workflow["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "E_FN_RETURN_UNPROVEN")
  );
  assert!(
    workflow["data"]["workflow"]["review_required"]["schema_candidates"]
      .as_array()
      .is_some_and(|candidates| !candidates.is_empty())
  );
  assert!(
    workflow["data"]["workflow"]["review_required"]["structural_candidates"]["map_shapes"]
      .as_array()
      .is_some_and(|candidates| !candidates.is_empty())
  );
}

#[test]
fn schema_synthesis_does_not_treat_sample_namespaces_as_argument_proof() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/sample-observed",
        "--code",
        "quote $ defn sample-observed (x) 1",
      ],
      "create schema target",
    ),
    (vec!["edit", "add-ns", "fix-command.test"], "create test namespace"),
    (
      vec![
        "edit",
        "def",
        "fix-command.test/use-sample-observed",
        "--code",
        "quote $ defn use-sample-observed () (fix-command.main/sample-observed 2)",
      ],
      "create test-only callsite",
    ),
    (vec!["edit", "add-ns", "fix-command.examples"], "create example namespace"),
    (
      vec![
        "edit",
        "def",
        "fix-command.examples/show-sample-observed",
        "--code",
        "quote $ defn show-sample-observed () (fix-command.main/sample-observed 3)",
      ],
      "create example-only callsite",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }

  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "sample-observed",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "sample-only schema preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.args.0"])
  );
  assert!(
    report["data"]["suggestions"][0]["origin_chain"]
      .as_array()
      .expect("origin chain should be an array")
      .iter()
      .all(|evidence| evidence["kind"] != "resolved-callsite-arguments")
  );
}

#[test]
fn schema_synthesis_does_not_invent_a_wasm_import_return_type() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/imported-value",
        "--code",
        "quote $ defwasm-import imported-value (x) |host |read-value",
      ],
    ),
    "create untyped WASM import",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "imported-value",
      "--format",
      "json",
    ],
  );
  assert!(!preview.status.success());
  assert!(
    String::from_utf8_lossy(&preview.stderr).contains("could not recover static implementation evidence"),
    "stderr:\n{}",
    String::from_utf8_lossy(&preview.stderr)
  );
}

#[test]
fn schema_synthesis_preserves_precise_ref_shape_and_partial_holes() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let atom_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "*fix-person-calls",
      "--format",
      "json",
    ],
  );
  assert_success(&atom_preview, "atom schema preview");
  let atom_report = parse_stdout(&atom_preview);
  assert_eq!(
    atom_report["data"]["suggestions"][0]["replacement"]["value"],
    serde_json::json!(["::", "'Ref", "'Number"])
  );

  let before = fs::read(&snapshot).expect("fixture bytes should read");
  let partial = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "option-struct-field",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert_success(&partial, "partial schema preview");
  let partial_report = parse_stdout(&partial);
  assert_eq!(partial_report["data"]["changed"], false);
  assert_eq!(partial_report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    partial_report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.return.type-args.0"])
  );
  assert_eq!(fs::read(&snapshot).expect("fixture bytes should reread"), before);

  for (target, code) in [
    ("fix-command.main/inferred-identity", "quote $ defn inferred-identity (x) 1"),
    (
      "fix-command.main/use-identity-number",
      "quote $ defn use-identity-number () (inferred-identity 1)",
    ),
    (
      "fix-command.main/use-identity-string",
      "quote $ defn use-identity-string () (inferred-identity |x)",
    ),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target, "--code", code]),
      "create conflicting callsite fixture",
    );
  }
  let conflicting = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "inferred-identity",
      "--format",
      "json",
    ],
  );
  assert_success(&conflicting, "conflicting callsite schema preview");
  let conflicting_report = parse_stdout(&conflicting);
  assert_eq!(conflicting_report["data"]["changed"], false);
  assert_eq!(conflicting_report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    conflicting_report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.args.0"])
  );

  for (target, code) in [
    ("fix-command.main/macro-observed", "quote $ defn macro-observed (x) 1"),
    (
      "fix-command.main/use-macro-observed",
      "quote $ defn use-macro-observed () (macro-observed 2)",
    ),
    (
      "fix-command.main/expand-macro-observed",
      "quote $ defmacro expand-macro-observed () macro-observed 3",
    ),
  ] {
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", target, "--code", code]),
      "create macro-boundary schema fixture",
    );
  }
  let macro_boundary = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "macro-observed",
      "--format",
      "json",
    ],
  );
  assert_success(&macro_boundary, "macro-boundary schema preview");
  let macro_report = parse_stdout(&macro_boundary);
  assert_eq!(macro_report["data"]["suggestions"][0]["applicability"], "needs-review");
  assert_eq!(
    macro_report["data"]["suggestions"][0]["origin_chain"][0]["unresolved_slots"],
    serde_json::json!(["schema.args.0"])
  );

  for (args, context) in [
    (
      vec![
        "edit",
        "def",
        "fix-command.main/attached-observed",
        "--code",
        "quote $ defn attached-observed (x) 1",
      ],
      "create attached-source schema target",
    ),
    (
      vec![
        "edit",
        "def",
        "fix-command.main/use-attached-observed",
        "--code",
        "quote $ defn use-attached-observed () (attached-observed 2)",
      ],
      "create ordinary attached-source callsite",
    ),
    (
      vec![
        "edit",
        "add-test",
        "fix-command.main/use-attached-observed",
        "rejects-string",
        "--code",
        "quote $ = (attached-observed |x) 1",
      ],
      "create conflicting attached test",
    ),
  ] {
    assert_success(&run_calcit(&snapshot, &args), context);
  }
  let attached_conflict = run_fix(
    &snapshot,
    &[
      "--rule",
      "synthesize-schema-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "attached-observed",
      "--format",
      "json",
    ],
  );
  assert!(!attached_conflict.status.success());
  assert!(String::from_utf8_lossy(&attached_conflict.stderr).contains("test `rejects-string`"));
}

#[test]
fn fix_preview_apply_and_repeat_are_revision_safe() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let original = fs::read(&snapshot).expect("fixture should read");

  let human = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable"]);
  assert!(human.status.success(), "stderr:\n{}", String::from_utf8_lossy(&human.stderr));
  let human_stdout = String::from_utf8_lossy(&human.stdout);
  assert!(human_stdout.starts_with("# Compiler-guided source fixes\n"));
  assert!(human_stdout.contains("## Suggestion 1\n"));
  assert!(human_stdout.contains("### Before\n\n- node kind: `leaf`\n\n```cirru\n"));
  assert!(human_stdout.contains("### After\n\n- node kind: `leaf`\n\n```cirru\n"));
  assert_eq!(fs::read(&snapshot).expect("human preview should not write"), original);

  let edn = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "edn"]);
  assert!(edn.status.success(), "stderr:\n{}", String::from_utf8_lossy(&edn.stderr));
  let edn_value = cirru_edn::parse(&String::from_utf8_lossy(&edn.stdout)).expect("fix EDN should parse");
  let cirru_edn::Edn::Map(edn_map) = edn_value else {
    panic!("fix EDN should be a map");
  };
  assert_eq!(edn_map.get(&cirru_edn::Edn::tag("command")), Some(&cirru_edn::Edn::str("fix")));
  assert_eq!(fs::read(&snapshot).expect("EDN preview should not write"), original);

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "json"]);
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let preview_json = parse_stdout(&preview);
  assert_eq!(preview_json["command"], "fix");
  assert_eq!(preview_json["data"]["mode"], "preview");
  assert_eq!(preview_json["data"]["changed"], true);
  assert_eq!(preview_json["data"]["suggestions"][0]["rule_id"], "removed-data-api-v1");
  assert_eq!(
    preview_json["data"]["suggestions"][0]["source_file"],
    snapshot.to_string_lossy().as_ref()
  );
  assert_eq!(preview_json["data"]["suggestions"][0]["replacement"]["value"], "enum-definition");
  assert_eq!(preview_json["data"]["validation"]["status"], "passed");
  assert_eq!(preview_json["data"]["validation"]["staged_scope_preprocess"], true);
  assert_eq!(preview_json["data"]["validation"]["checked_operations"], 1);
  assert_eq!(fs::read(&snapshot).expect("preview fixture should read"), original);

  let revision = preview_json["revision"].as_str().expect("preview revision should be a string");
  let no_vcs = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--apply",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert!(!no_vcs.status.success());
  assert!(String::from_utf8_lossy(&no_vcs.stderr).contains("--allow-no-vcs"));
  assert_eq!(fs::read(&snapshot).expect("no-vcs fixture should read"), original);

  let stale = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success());
  assert_eq!(fs::read(&snapshot).expect("stale fixture should read"), original);

  let applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "fixable",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert!(applied.status.success(), "stderr:\n{}", String::from_utf8_lossy(&applied.stderr));
  let applied_json = parse_stdout(&applied);
  assert_eq!(applied_json["data"]["mode"], "apply");
  assert_eq!(applied_json["data"]["changed"], true);
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("enum-definition value"));
  assert!(!updated.contains("defn fixable (value) (tuple-enum value)"));
  assert!(updated.contains("defn shadowed (tuple-enum value) (tuple-enum value)"));
  let calcit_test = run_calcit(&snapshot, &["test", "fix-command.main/fixable", "--require-match"]);
  assert!(
    calcit_test.status.success(),
    "Calcit definition test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_test.stdout),
    String::from_utf8_lossy(&calcit_test.stderr)
  );

  let repeated = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "json"]);
  assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
  let repeated_json = parse_stdout(&repeated);
  assert_eq!(repeated_json["data"]["changed"], false);
  assert_eq!(repeated_json["data"]["validation"]["status"], "not-needed");
  assert_eq!(repeated_json["data"]["validation"]["staged_scope_preprocess"], false);
  assert_eq!(repeated_json["data"]["validation"]["checked_operations"], 0);
  assert_eq!(repeated_json["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn ambiguous_removed_predicate_is_reported_without_a_replacement() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "ambiguous", "--format", "json"]);
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], false);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert!(report["data"]["suggestions"][0]["replacement"].is_null());
}

#[test]
fn local_binding_with_a_removed_core_name_is_not_rewritten() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "shadowed", "--format", "json"]);
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], false);
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(0));
}

#[test]
fn staged_fix_rejects_a_new_compiler_warning_without_writing() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let incompatible_schema = run_calcit(
    &snapshot,
    &[
      "edit",
      "schema",
      "fix-command.main/fixable",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return $ :: 'Option 'Tag)",
    ],
  );
  assert!(
    incompatible_schema.status.success(),
    "schema setup failed: {}",
    String::from_utf8_lossy(&incompatible_schema.stderr)
  );
  let before = fs::read(&snapshot).expect("fixture should read before rejected preview");

  let preview = run_fix(&snapshot, &["--ns", "fix-command.main", "--def", "fixable", "--format", "json"]);
  assert!(!preview.status.success());
  assert!(String::from_utf8_lossy(&preview.stderr).contains("W_FN_RETURN_TYPE_MISMATCH"));
  assert_eq!(fs::read(&snapshot).expect("rejected fixture should read"), before);
}

#[test]
fn retired_surface_rules_point_to_the_published_migration_bridge() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  for rule in ["tag-match-to-match-v1", "required-struct-field-v1"] {
    let output = run_fix(&snapshot, &["--rule", rule, "--format", "json"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Calcit 0.14.15"), "stderr: {stderr}");
    assert!(stderr.contains("before upgrading"), "stderr: {stderr}");
  }
}

#[test]
fn core_nominal_constructor_preview_skips_unrelated_type_slot_declarations() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/declare-dispatch-slot",
        "--code",
        "quote $ deftype-slot :dispatch-op",
      ],
    ),
    "install type-slot declaration",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-nominal-constructor-v1",
      "--ns",
      "fix-command.main",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "whole-namespace constructor preview with unrelated type slot");
  let report = parse_stdout(&preview);
  assert_eq!(report["command"], "fix");
  assert_eq!(report["data"]["suggestions"], serde_json::json!([]));
  let api_preview = run_fix(
    &snapshot,
    &["--preset", "core-api-0.28-v1", "--ns", "fix-command.main", "--format", "json"],
  );
  assert_success(&api_preview, "whole-namespace API preview with unrelated type slot");
  assert_eq!(parse_stdout(&api_preview)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn core_nominal_constructor_rule_previews_applies_and_is_idempotent() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/core-constructors",
        "--code",
        "quote $ defn core-constructors () (%some (%ok 1)) (%none) (%err |bad) &unit",
      ],
    ),
    "install constructor calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/core-constructors",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)",
      ],
    ),
    "declare constructor test schema",
  );

  let args = [
    "--rule",
    "core-nominal-constructor-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "core-constructors",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "constructor preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 3, "report: {report}");
  assert!(
    suggestions
      .iter()
      .all(|suggestion| suggestion["applicability"] == "machine-applicable")
  );
  assert_eq!(report["data"]["validation"]["status"], "passed");
  let revision = report["revision"].as_str().expect("preview should return revision");

  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-nominal-constructor-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "core-constructors",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success());
  assert!(String::from_utf8_lossy(&stale.stderr).contains("Snapshot revision mismatch"));

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-nominal-constructor-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "core-constructors",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "constructor apply");
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("Option :some $ Result :ok 1"), "snapshot: {updated}");
  assert!(updated.contains("Option :none"), "snapshot: {updated}");
  assert!(updated.contains("Result :err |bad"), "snapshot: {updated}");
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "constructor idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn core_nominal_constructor_rule_reviews_shadowed_types_and_function_values() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/shadowed-option",
        "--code",
        "quote $ defn shadowed-option (Option) (%some Option)",
      ],
    ),
    "install shadowed nominal name",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/shadowed-option",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return $ :: 'Option 'Number)",
      ],
    ),
    "declare shadowed constructor schema",
  );
  let shadowed = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-nominal-constructor-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "shadowed-option",
      "--format",
      "json",
    ],
  );
  assert_success(&shadowed, "shadowed constructor preview");
  let report = parse_stdout(&shadowed);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/constructor-as-value",
        "--code",
        "quote $ defn constructor-as-value () %some",
      ],
    ),
    "install function-value constructor reference",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/constructor-as-value",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)",
      ],
    ),
    "declare function-value schema",
  );
  let value = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-nominal-constructor-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "constructor-as-value",
      "--format",
      "json",
    ],
  );
  assert_success(&value, "function-value constructor preview");
  let report = parse_stdout(&value);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
  assert_eq!(report["data"]["validation"]["checked_operations"], 0);
}

#[test]
fn core_option_method_rule_uses_proven_receiver_and_is_idempotent() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/option-methods",
        "--code",
        "quote $ defn option-methods (opt) (+ (option:unwrap opt) (option:unwrap-or opt 0))",
      ],
    ),
    "install Option helper calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/option-methods",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] (:: 'Option 'Number)) (:return 'Number)",
      ],
    ),
    "declare Option receiver schema",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        "fix-command.main/option-methods",
        "typed-option-result",
        "--code",
        "quote $ assert= 4 $ option-methods $ Option :some 2",
      ],
    ),
    "attach Calcit behavior test",
  );

  let args = [
    "--rule",
    "core-option-method-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "option-methods",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "Option method preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 2, "report: {report}");
  assert!(
    suggestions
      .iter()
      .all(|suggestion| suggestion["applicability"] == "machine-applicable"),
    "report: {report}"
  );
  assert_eq!(report["data"]["validation"]["status"], "passed");
  let revision = report["revision"].as_str().expect("preview should return revision");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "option-methods",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success());
  assert!(String::from_utf8_lossy(&stale.stderr).contains("Snapshot revision mismatch"));
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "option-methods",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "Option method apply");
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("opt .unwrap"), "snapshot: {updated}");
  assert!(updated.contains("opt .unwrap-or 0"), "snapshot: {updated}");
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "Option method idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
  assert_success(
    &run_calcit(&snapshot, &["test", "fix-command.main/option-methods", "--require-match"]),
    "Calcit attached test after Option migration",
  );
}

#[test]
fn core_option_method_rule_migrates_proven_empty_option_with_typed_fallback() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/test-wasm.cirru", &snapshot).expect("copy WASM Snapshot");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "tree",
        "replace",
        "test-wasm.main/test-option-unwrap-or",
        "--path",
        "@3",
        "--input-format",
        "cirru",
        "--expect",
        "quote $ (%none) .unwrap-or 7",
        "--code",
        "quote $ option:unwrap-or (%none) 7",
      ],
    ),
    "restore legacy empty Option helper",
  );
  let args = [
    "--rule",
    "core-option-method-v1",
    "--ns",
    "test-wasm.main",
    "--def",
    "test-option-unwrap-or",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "empty Option migration preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "test-wasm.main",
      "--def",
      "test-option-unwrap-or",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "empty Option migration apply");
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "empty Option idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  let named_snapshot = directory.path().join("named-option.cirru");
  fs::copy("calcit/test-types-inference.cirru", &named_snapshot).expect("copy named Option Snapshot");
  let named_args = [
    "--rule",
    "core-option-method-v1",
    "--ns",
    "test-types-inference.main",
    "--def",
    "infer-named-none-fallback",
    "--format",
    "json",
  ];
  let named_preview = run_fix(&named_snapshot, &named_args);
  assert_success(&named_preview, "named empty Option migration preview");
  let named_report = parse_stdout(&named_preview);
  assert_eq!(
    named_report["data"]["suggestions"][0]["applicability"], "machine-applicable",
    "{named_report}"
  );
  assert_eq!(named_report["data"]["validation"]["status"], "passed", "{named_report}");
  let named_applied = run_fix(
    &named_snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "test-types-inference.main",
      "--def",
      "infer-named-none-fallback",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      named_report["revision"].as_str().expect("named preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&named_applied, "named empty Option migration apply");
  let named_repeated = run_fix(&named_snapshot, &named_args);
  assert_success(&named_repeated, "named empty Option idempotence preview");
  assert_eq!(parse_stdout(&named_repeated)["data"]["suggestions"], serde_json::json!([]));
  assert_success(
    &run_calcit(
      &named_snapshot,
      &["test", "test-types-inference.main/infer-named-none-fallback", "--require-match"],
    ),
    "named empty Option behavior after method migration",
  );
}

#[test]
fn empty_option_helpers_infer_fallback_type_for_both_constructor_spellings() {
  for definition in ["infer-none-fallback", "infer-named-none-fallback"] {
    let target = format!("test-types-inference.main/{definition}");
    let result = run_calcit(
      Path::new("calcit/test-types-inference.cirru"),
      &["query", "type-at", &target, "--path", "@3", "--format", "json"],
    );
    assert_success(&result, "empty Option fallback type query");
    let report = parse_stdout(&result);
    assert_eq!(report["data"]["inferred_type"], "'Number", "{report}");
    assert_eq!(report["data"]["confidence"], "exact", "{report}");
  }
}

#[test]
fn core_result_method_rule_migrates_proven_error_with_typed_fallback() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/test-wasm.cirru", &snapshot).expect("copy WASM Snapshot");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "tree",
        "replace",
        "test-wasm.main/test-result-unwrap-or",
        "--path",
        "@3",
        "--input-format",
        "cirru",
        "--expect",
        "quote $ (%err 3) .unwrap-or 7",
        "--code",
        "quote $ result:unwrap-or (%err 3) 7",
      ],
    ),
    "restore legacy Result helper",
  );
  let args = [
    "--rule",
    "core-result-method-v1",
    "--ns",
    "test-wasm.main",
    "--def",
    "test-result-unwrap-or",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "Result method preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-result-method-v1",
      "--ns",
      "test-wasm.main",
      "--def",
      "test-result-unwrap-or",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success());
  assert!(String::from_utf8_lossy(&stale.stderr).contains("Snapshot revision mismatch"));
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-result-method-v1",
      "--ns",
      "test-wasm.main",
      "--def",
      "test-result-unwrap-or",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "Result method apply");
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "Result method idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  let named_snapshot = directory.path().join("named-result.cirru");
  fs::copy("calcit/test-types-inference.cirru", &named_snapshot).expect("copy named Result Snapshot");
  let named_args = [
    "--rule",
    "core-result-method-v1",
    "--ns",
    "test-types-inference.main",
    "--def",
    "infer-named-result-err-fallback",
    "--format",
    "json",
  ];
  let named_preview = run_fix(&named_snapshot, &named_args);
  assert_success(&named_preview, "named Result method preview");
  let named_report = parse_stdout(&named_preview);
  assert_eq!(
    named_report["data"]["suggestions"][0]["applicability"], "machine-applicable",
    "{named_report}"
  );
  assert_eq!(named_report["data"]["validation"]["status"], "passed", "{named_report}");
  let named_applied = run_fix(
    &named_snapshot,
    &[
      "--rule",
      "core-result-method-v1",
      "--ns",
      "test-types-inference.main",
      "--def",
      "infer-named-result-err-fallback",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      named_report["revision"].as_str().expect("named preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&named_applied, "named Result method apply");
  let named_repeated = run_fix(&named_snapshot, &named_args);
  assert_success(&named_repeated, "named Result idempotence preview");
  assert_eq!(parse_stdout(&named_repeated)["data"]["suggestions"], serde_json::json!([]));
  assert_success(
    &run_calcit(
      &named_snapshot,
      &[
        "test",
        "test-types-inference.main/infer-named-result-err-fallback",
        "--require-match",
      ],
    ),
    "named Result behavior after method migration",
  );
}

#[test]
fn result_error_helpers_infer_fallback_without_erasing_error_payload() {
  for definition in ["infer-result-err-fallback", "infer-named-result-err-fallback"] {
    let target = format!("test-types-inference.main/{definition}");
    let result = run_calcit(
      Path::new("calcit/test-types-inference.cirru"),
      &["query", "type-at", &target, "--path", "@3", "--format", "json"],
    );
    assert_success(&result, "Result fallback type query");
    let report = parse_stdout(&result);
    assert_eq!(report["data"]["inferred_type"], "'Number", "{report}");
    assert_eq!(report["data"]["confidence"], "exact", "{report}");
  }
  let receiver = run_calcit(
    Path::new("calcit/test-types-inference.cirru"),
    &[
      "query",
      "type-at",
      "test-types-inference.main/infer-result-err-fallback",
      "--path",
      "@3.1",
      "--format",
      "json",
    ],
  );
  assert_success(&receiver, "Result error payload type query");
  assert_eq!(
    parse_stdout(&receiver)["data"]["inferred_type"],
    ":: 'calcit.core/Result 'Dynamic 'Number"
  );
}

#[test]
fn result_success_with_dynamic_payload_stays_open_across_fallback() {
  for (definition, expected_type, expected_confidence) in [
    ("infer-later-dynamic-generic", "'Dynamic", "unknown"),
    ("infer-result-ok-open", "'Dynamic", "unknown"),
    ("infer-named-result-ok-open", "'Dynamic", "unknown"),
    ("infer-result-ok-concrete", "'Number", "exact"),
    ("infer-named-result-ok-concrete", "'Number", "exact"),
    ("infer-result-err-fallback", "'Number", "exact"),
    ("infer-named-result-err-fallback", "'Number", "exact"),
  ] {
    let target = format!("test-types-inference.main/{definition}");
    let result = run_calcit(
      Path::new("calcit/test-types-inference.cirru"),
      &["query", "type-at", &target, "--path", "@3", "--format", "json"],
    );
    assert_success(&result, "Result success and fallback type query");
    let report = parse_stdout(&result);
    assert_eq!(report["data"]["inferred_type"], expected_type, "{target}: {report}");
    assert_eq!(report["data"]["confidence"], expected_confidence, "{target}: {report}");
  }
}

#[test]
fn result_method_fix_uses_concrete_success_evidence_but_not_dynamic_fallback() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/test-types-inference.cirru", &snapshot).expect("copy type inference Snapshot");
  let args = [
    "--rule",
    "core-result-method-v1",
    "--ns",
    "test-types-inference.main",
    "--def",
    "infer-result-ok-concrete",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "concrete Result success method preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-result-method-v1",
      "--ns",
      "test-types-inference.main",
      "--def",
      "infer-result-ok-concrete",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "concrete Result success method apply");
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "concrete Result success method idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  let open = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-result-method-v1",
      "--ns",
      "test-types-inference.main",
      "--def",
      "infer-result-ok-open",
      "--format",
      "json",
    ],
  );
  assert_success(&open, "dynamic Result success method preview");
  let open_report = parse_stdout(&open);
  assert_eq!(
    open_report["data"]["suggestions"][0]["applicability"], "requires-review",
    "{open_report}"
  );
}

#[test]
fn core_nominal_predicate_rules_migrate_proven_calls_idempotently() {
  let directory = TestDirectory::create();
  for (definition, rule) in [
    ("infer-option-some-predicate", "core-option-method-v1"),
    ("infer-option-none-predicate", "core-option-method-v1"),
    ("infer-result-ok-predicate", "core-result-method-v1"),
    ("infer-result-err-predicate", "core-result-method-v1"),
  ] {
    let snapshot = directory.path().join(format!("{definition}.cirru"));
    fs::copy("calcit/test-types-inference.cirru", &snapshot).expect("copy type inference Snapshot");
    let args = [
      "--rule",
      rule,
      "--ns",
      "test-types-inference.main",
      "--def",
      definition,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &args);
    assert_success(&preview, "nominal predicate preview");
    let report = parse_stdout(&preview);
    assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
    assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");

    let applied = run_fix(
      &snapshot,
      &[
        "--rule",
        rule,
        "--ns",
        "test-types-inference.main",
        "--def",
        definition,
        "--apply",
        "--allow-no-vcs",
        "--expect-revision",
        report["revision"].as_str().expect("preview revision"),
        "--format",
        "json",
      ],
    );
    assert_success(&applied, "nominal predicate apply");
    let repeated = run_fix(&snapshot, &args);
    assert_success(&repeated, "nominal predicate idempotence preview");
    assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
    let target = format!("test-types-inference.main/{definition}");
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "nominal predicate behavior after migration",
    );
  }

  for (definition, helper, rule) in [
    ("open-option-predicate", "option:some?", "core-option-method-v1"),
    ("open-result-predicate", "result:ok?", "core-result-method-v1"),
  ] {
    let snapshot = directory.path().join(format!("{definition}.cirru"));
    fs::copy("calcit/test-types-inference.cirru", &snapshot).expect("copy open predicate Snapshot");
    let target = format!("test-types-inference.main/{definition}");
    let code = format!("quote $ defn {definition} (value) ({helper} value)");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", &code]),
      "install open predicate source",
    );
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &target,
          "--input-format",
          "cirru",
          "--code",
          "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Bool)",
        ],
      ),
      "declare open predicate source",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        rule,
        "--ns",
        "test-types-inference.main",
        "--def",
        definition,
        "--format",
        "json",
      ],
    );
    assert!(
      !preview.status.success(),
      "a raw Dynamic receiver must not produce an automatic rewrite"
    );
    let diagnostic = String::from_utf8_lossy(&preview.stderr);
    assert!(diagnostic.contains("E_ERASED_GENERIC_RELATION"), "{diagnostic}");
  }
}

#[test]
fn core_function_alias_rule_renames_resolved_legacy_functions_and_is_idempotent() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/legacy-function-aliases";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn legacy-function-aliases ()\n  assert= (#{} 1 2) $ vals $ {} (:a 1) (:b 2)\n  assert= |a,b $ join-str ([] |a |b) |,\n  assert= ([] |a |, |b) $ join ([] |a |b) |,\n  assert= (%some 1) $ optionally 1\n  , true",
      ],
    ),
    "install legacy function alias calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare legacy function alias source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-results",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= true $ legacy-function-aliases",
      ],
    ),
    "attach legacy function alias behavior test",
  );

  let args = [
    "--rule",
    "core-function-alias-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "legacy-function-aliases",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "function alias preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 4, "{report}");
  assert!(
    suggestions
      .iter()
      .all(|suggestion| { suggestion["rule_id"] == "core-function-alias-v1" && suggestion["applicability"] == "machine-applicable" })
  );
  for legacy in ["vals", "join-str", "join", "optionally"] {
    let expected = format!("calcit.core/{legacy}");
    assert!(
      suggestions
        .iter()
        .any(|suggestion| suggestion["origin_chain"][0]["target"] == expected.as_str()),
      "missing {legacy}: {report}"
    );
  }
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-function-alias-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-function-aliases",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "function alias apply");
  let updated = fs::read_to_string(&snapshot).expect("updated Snapshot should read");
  for preferred in [
    "calcit.core/distinct-values",
    "calcit.core/join-string",
    "calcit.core/intersperse",
    "calcit.core/nil->option",
  ] {
    assert!(updated.contains(preferred), "{preferred} missing after migration");
  }
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "function alias behavior after migration",
  );

  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "function alias idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  // A first-class reference keeps its function identity, so it is review-only.
  let first_class = "fix-command.main/first-class-alias";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        first_class,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn first-class-alias ()\n  assert= 1 $ count $ [] vals\n  , true",
      ],
    ),
    "install a first-class legacy function reference",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        first_class,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare the first-class reference source",
  );
  let review = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-function-alias-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "first-class-alias",
      "--format",
      "json",
    ],
  );
  assert_success(&review, "first-class alias preview");
  let review_report = parse_stdout(&review);
  let review_suggestions = review_report["data"]["suggestions"]
    .as_array()
    .expect("suggestions should be an array");
  assert_eq!(review_suggestions.len(), 1, "{review_report}");
  assert_eq!(review_suggestions[0]["applicability"], "requires-review", "{review_report}");
}

#[test]
fn core_non_nil_predicate_rule_uses_resolved_references_and_is_idempotent() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/legacy-non-nil";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn legacy-non-nil ()\n  assert= true $ some? false\n  assert= true $ some? 0\n  assert= false $ some? nil\n  some? 42",
      ],
    ),
    "install legacy non-nil predicate calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare legacy non-nil source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-results",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= true $ legacy-non-nil",
      ],
    ),
    "attach non-nil behavior test",
  );

  let args = [
    "--rule",
    "core-non-nil-predicate-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "legacy-non-nil",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "non-nil predicate preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 4, "{report}");
  assert!(suggestions.iter().all(|suggestion| {
    suggestion["rule_id"] == "core-non-nil-predicate-v1"
      && suggestion["applicability"] == "machine-applicable"
      && suggestion["origin_chain"][0]["target"] == "calcit.core/some?"
  }));
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");

  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-non-nil-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-non-nil",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "non-nil predicate apply");
  let updated = fs::read_to_string(&snapshot).expect("updated Snapshot should read");
  assert!(updated.contains("calcit.core/non-nil?"));
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "non-nil behavior after migration",
  );

  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "non-nil predicate idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/shadowed-some",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn shadowed-some ()\n  let\n      some? $ fn (x) false\n    some? nil",
      ],
    ),
    "install a locally shadowed predicate",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/shadowed-some",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare the locally shadowed predicate",
  );
  let shadowed_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-non-nil-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "shadowed-some",
      "--format",
      "json",
    ],
  );
  assert_success(&shadowed_preview, "locally shadowed predicate preview");
  assert_eq!(parse_stdout(&shadowed_preview)["data"]["suggestions"], serde_json::json!([]));

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/pass-form",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defmacro pass-form (body) body",
      ],
    ),
    "install unknown source macro",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/legacy-macro-nil",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn legacy-macro-nil () $ pass-form $ some? nil",
      ],
    ),
    "install macro-wrapped legacy predicate",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/legacy-macro-nil",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare macro-wrapped legacy predicate",
  );
  let macro_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-non-nil-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-macro-nil",
      "--format",
      "json",
    ],
  );
  assert_success(&macro_preview, "macro-wrapped non-nil predicate preview");
  let macro_report = parse_stdout(&macro_preview);
  assert_eq!(macro_report["data"]["suggestions"].as_array().map(Vec::len), Some(1));
  assert_eq!(macro_report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert_eq!(macro_report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
  assert_eq!(
    macro_report["data"]["suggestions"][0]["origin_chain"][0]["macro_origin"][0],
    "fix-command.main/pass-form"
  );
}

#[test]
fn core_integer_predicate_rule_migrates_proven_number_methods_and_guards_source() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/legacy-integer";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn legacy-integer ()\n  assert= true $ round? 0\n  assert= false $ round? 0.25\n  assert= true $ .round? -1\n  quote $ round? 8\n  round? 4",
      ],
    ),
    "install legacy integer calls",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare legacy integer source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-integer-result",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= true $ legacy-integer",
      ],
    ),
    "attach legacy integer behavior test",
  );

  let args = [
    "--rule",
    "core-integer-predicate-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "legacy-integer",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "integer predicate preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions should be an array");
  assert_eq!(suggestions.len(), 4, "{report}");
  assert!(
    suggestions.iter().all(|suggestion| {
      suggestion["rule_id"] == "core-integer-predicate-v1" && suggestion["applicability"] == "machine-applicable"
    })
  );
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  assert_eq!(
    suggestions
      .iter()
      .filter(|suggestion| suggestion["origin_chain"][0]["kind"] == "reader-resolved-builtin-proc")
      .count(),
    3
  );
  assert_eq!(
    suggestions
      .iter()
      .filter(|suggestion| suggestion["origin_chain"][0]["kind"] == "receiver-method-query")
      .count(),
    1
  );

  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-integer-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-integer",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale revision must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-integer-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-integer",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "integer predicate apply");
  let updated = fs::read_to_string(&snapshot).expect("updated Snapshot should read");
  assert!(updated.contains("calcit.core/integer?"));
  assert!(
    updated.contains("assert= true $ .integer? -1"),
    "proven Number method should migrate"
  );
  assert!(updated.contains("quote $ round? 8"), "quoted data stays unchanged");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "integer behavior after migration",
  );
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "integer predicate idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/pass-form",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defmacro pass-form (body) body",
      ],
    ),
    "install unknown source macro",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/macro-integer",
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn macro-integer () $ pass-form $ round? 4",
      ],
    ),
    "install macro-wrapped integer call",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/macro-integer",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
      ],
    ),
    "declare macro-wrapped integer result",
  );
  let macro_preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-integer-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "macro-integer",
      "--format",
      "json",
    ],
  );
  assert_success(&macro_preview, "macro-wrapped integer predicate preview");
  let macro_report = parse_stdout(&macro_preview);
  assert_eq!(macro_report["data"]["suggestions"].as_array().map(Vec::len), Some(1));
  assert_eq!(macro_report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert_eq!(macro_report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);

  for (name, code, schema) in [
    (
      "typed-integer-method",
      "quote $ defn typed-integer-method (value)\n  value .round?",
      "quote $ :: 'Fn $ {} (:args $ [] 'Number) (:return 'Bool)",
    ),
    (
      "macro-integer-method",
      "quote $ defn macro-integer-method () $ pass-form $ .round? 4",
      "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)",
    ),
  ] {
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--input-format", "cirru", "--code", code]),
      "install Number method source",
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", &target, "--input-format", "cirru", "--code", schema]),
      "declare Number method source",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-integer-predicate-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "Number method preview");
    let report = parse_stdout(&preview);
    assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(1), "{report}");
    let suggestion = &report["data"]["suggestions"][0];
    assert_eq!(
      suggestion["applicability"],
      if name.starts_with("typed") {
        "machine-applicable"
      } else {
        "requires-review"
      }
    );
    assert_eq!(suggestion["origin_chain"][0]["kind"], "receiver-method-query");
    assert_eq!(suggestion["origin_chain"][0]["receiver_type"], "number");
  }
}

#[test]
fn integer_predicate_fix_keeps_custom_round_method() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("RoundedBox0", "quote $ defstruct RoundedBox0 (:value 'Number)"),
    ("RoundedBoxTrait", "quote $ deftrait RoundedBoxTrait (.round? :fn)"),
    (
      "RoundedBoxImpl",
      "quote $ defimpl RoundedBoxImpl RoundedBoxTrait\n  .round? $ fn (box) true",
    ),
    ("RoundedBox", "quote $ def RoundedBox $ impl-traits RoundedBox0 RoundedBoxImpl"),
    ("read-box-round", "quote $ defn read-box-round (box) box .round?"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install custom method boundary",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/read-box-round",
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'fix-command.main/RoundedBox) (:return 'Bool)",
      ],
    ),
    "declare custom method receiver",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-integer-predicate-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-box-round",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "custom method preview");
  assert_eq!(parse_stdout(&preview)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn predicate_method_fix_migrates_only_proven_builtin_receivers() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/legacy-predicates";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn legacy-predicates ()\n  assert= true $ ([] 10 20) .contains? 1\n  assert= true $ \"|😀a\" .contains? 1\n  assert= true $ ({} (|key |value)) .contains? |key\n  assert= true $ ({} (|key |value)) .includes? |value\n  assert= true $ (#{} 10 20) .contains? 10\n  quote $ ([] 10) .contains? 0\n  , &unit",
      ],
    ),
    "install predicate method source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Unit)",
      ],
    ),
    "declare predicate method source",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "add-test",
        target,
        "preserves-predicates",
        "--tags",
        "unit",
        "--input-format",
        "cirru",
        "--code",
        "quote $ assert= &unit $ legacy-predicates",
      ],
    ),
    "attach predicate regression",
  );
  let selector = [
    "--rule",
    "core-predicate-method-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "legacy-predicates",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selector);
  assert_success(&preview, "predicate method preview");
  let report = parse_stdout(&preview);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions array");
  assert_eq!(suggestions.len(), 5, "{report}");
  let unique_paths = suggestions
    .iter()
    .map(|item| item["path"].as_str().expect("source path"))
    .collect::<std::collections::HashSet<_>>();
  assert_eq!(
    unique_paths.len(),
    suggestions.len(),
    "a source call must have one migration suggestion"
  );
  assert!(suggestions.iter().all(|item| item["applicability"] == "machine-applicable"));
  assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
  let replacements = suggestions
    .iter()
    .map(|item| item["replacement"]["value"].as_str().unwrap_or_default())
    .collect::<Vec<_>>();
  assert_eq!(replacements.iter().filter(|name| **name == ".contains-index?").count(), 2);
  assert!(replacements.contains(&".contains-key?"));
  assert!(replacements.contains(&".contains-value?"));
  assert!(replacements.contains(&".includes?"));
  let stale = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-predicate-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-predicates",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      "md5:stale",
      "--format",
      "json",
    ],
  );
  assert!(!stale.status.success(), "stale source must reject apply");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-predicate-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-predicates",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "predicate method apply");
  let updated = fs::read_to_string(&snapshot).expect("updated Snapshot should read");
  assert!(updated.contains(".contains-index?"));
  assert!(updated.contains(".contains-key?"));
  assert!(updated.contains(".contains-value?"));
  assert!(updated.contains(".contains? 0"), "quoted data must remain unchanged: {updated}");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "predicate behavior after fix",
  );
  let repeated = run_fix(&snapshot, &selector);
  assert_success(&repeated, "predicate method idempotence preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn predicate_method_fix_preserves_custom_methods_and_reviews_unknown_macros() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (name, code) in [
    ("Box0", "quote $ defstruct Box0 (:value 'Number)"),
    ("BoxContains", "quote $ deftrait BoxContains (.contains? :fn)"),
    (
      "BoxContainsImpl",
      "quote $ defimpl BoxContainsImpl BoxContains\n  .contains? $ fn (box index) true",
    ),
    ("Box", "quote $ def Box $ impl-traits Box0 BoxContainsImpl"),
    ("read-box", "quote $ defn read-box (box) box .contains? 0"),
    ("read-open", "quote $ defn read-open (value) value .contains? 0"),
    ("pass-form", "quote $ defmacro pass-form (body) body"),
    (
      "read-list-macro",
      "quote $ defn read-list-macro () $ pass-form $ ([] 10) .contains? 0",
    ),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "def",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          code,
        ],
      ),
      "install predicate negative boundary",
    );
  }
  for (name, schema) in [
    ("read-box", "quote $ :: 'Fn $ {} (:args $ [] 'fix-command.main/Box) (:return 'Bool)"),
    ("read-open", "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Bool)"),
    ("read-list-macro", "quote $ :: 'Fn $ {} (:args $ []) (:return 'Bool)"),
  ] {
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "edit",
          "schema",
          &format!("fix-command.main/{name}"),
          "--input-format",
          "cirru",
          "--code",
          schema,
        ],
      ),
      "declare predicate negative boundary",
    );
  }
  let custom = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-predicate-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-box",
      "--format",
      "json",
    ],
  );
  assert_success(&custom, "custom predicate method preview");
  assert_eq!(parse_stdout(&custom)["data"]["suggestions"], serde_json::json!([]));

  let open = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-predicate-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-open",
      "--format",
      "json",
    ],
  );
  assert_success(&open, "open predicate method preview");
  let open_report = parse_stdout(&open);
  assert_eq!(open_report["data"]["suggestions"], serde_json::json!([]), "{open_report}");

  let macro_call = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-predicate-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "read-list-macro",
      "--format",
      "json",
    ],
  );
  assert_success(&macro_call, "macro-wrapped predicate method preview");
  let report = parse_stdout(&macro_call);
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(1), "{report}");
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review");
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
}

#[test]
fn strict_result_success_consumption_rejects_dynamic_payload() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/test-types-inference.cirru", &snapshot).expect("copy type inference Snapshot");
  let target = "test-types-inference.main/consume-open-result";
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ defn consume-open-result (value) ((result:unwrap-or (%ok value) 7) .add 1)",
      ],
    ),
    "install open Result consumer",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Number)",
      ],
    ),
    "declare open Result consumer",
  );
  let checked = run_calcit(&snapshot, &["--check-only", "--init-fn", target]);
  assert!(
    !checked.status.success(),
    "Dynamic success payload must not become a statically callable Number"
  );
  let diagnostic = String::from_utf8_lossy(&checked.stderr);
  assert!(
    diagnostic.contains("E_DYNAMIC_POSTFIX_METHOD") && diagnostic.contains(".add"),
    "{diagnostic}"
  );
}

#[test]
fn core_result_method_rule_keeps_open_and_shadowed_receivers_for_review() {
  let directory = TestDirectory::create();
  for (name, code, schema) in [
    (
      "open-result",
      "quote $ defn open-result (value) (result:unwrap-or value 7)",
      "quote $ :: 'Fn $ {} (:args $ [] (:: 'Result 'Dynamic 'Number)) (:return 'Dynamic)",
    ),
    (
      "open-ok-result",
      "quote $ defn open-ok-result (value) (result:unwrap-or (%ok value) 7)",
      "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)",
    ),
  ] {
    let snapshot = directory.path().join(format!("{name}.cirru"));
    fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy fix fixture");
    let target = format!("fix-command.main/{name}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--code", code]),
      "install Result helper call",
    );
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", &target, "--code", schema]),
      "declare Result schema",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-result-method-v1",
        "--ns",
        "fix-command.main",
        "--def",
        name,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "open Result preview");
    let report = parse_stdout(&preview);
    assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review", "{report}");
  }

  let snapshot = directory.path().join("shadowed-result.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("copy shadowing fixture");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/Result",
        "--code",
        "quote $ defenum Result ([] 'T 'E) (:ok 'T) (:err 'E)",
      ],
    ),
    "install local Result enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "fix-command.main/Result", "--code", "quote $ :: 'EnumDef"],
    ),
    "declare local Result enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/shadowed-result",
        "--code",
        "quote $ defn shadowed-result () (result:unwrap-or (Result :err 3) 7)",
      ],
    ),
    "install core helper with local Result constructor",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/shadowed-result",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
    ),
    "declare shadowed helper schema",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-result-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "shadowed-result",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "shadowed Result preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review", "{report}");
}

#[test]
fn core_option_method_rule_requires_review_without_proven_receiver() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/open-option",
        "--code",
        "quote $ defn open-option (opt) option:unwrap-or opt 0",
      ],
    ),
    "install open Option helper call",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/open-option",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] (:: 'Option 'Dynamic)) (:return 'Dynamic)",
      ],
    ),
    "declare open Option receiver schema",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "open-option",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "open Option preview");
  let report = parse_stdout(&preview);
  assert_eq!(
    report["data"]["suggestions"][0]["applicability"], "requires-review",
    "report: {report}"
  );
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
  assert_eq!(report["data"]["validation"]["checked_operations"], 0);

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/option-helper-value",
        "--code",
        "quote $ defn option-helper-value () option:unwrap",
      ],
    ),
    "install Option helper as a function value",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/option-helper-value",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Dynamic)",
      ],
    ),
    "declare helper-value schema",
  );
  let value = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "option-helper-value",
      "--format",
      "json",
    ],
  );
  assert_success(&value, "Option helper function-value preview");
  let report = parse_stdout(&value);
  assert_eq!(
    report["data"]["suggestions"][0]["applicability"], "requires-review",
    "report: {report}"
  );
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
}

#[test]
fn core_option_method_rule_uses_direct_get_env_type_evidence() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/env-or",
        "--code",
        "quote $ defn env-or () $ option:unwrap-or (get-env |CALCIT_TEST_ENV) |missing",
      ],
    ),
    "install direct get-env helper call",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/env-or",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'String)",
      ],
    ),
    "declare env-or schema",
  );
  let args = [
    "--rule",
    "core-option-method-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "env-or",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "get-env Option method preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
  assert_eq!(report["data"]["validation"]["status"], "passed");
  let revision = report["revision"].as_str().expect("preview revision");
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "env-or",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      revision,
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "get-env Option method apply");
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("get-env |CALCIT_TEST_ENV"), "snapshot: {updated}");
  assert!(updated.contains(".unwrap-or |missing"), "snapshot: {updated}");
}

#[test]
fn core_option_method_rule_accepts_single_use_core_macro_wrappers() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (definition, code, return_type) in [
    (
      "asserted-option",
      "quote $ defn asserted-option (opt) $ assert= 2 $ do $ option:unwrap-or opt 0",
      "'Unit",
    ),
    (
      "closure-option",
      "quote $ defn closure-option (opt) $ let ((f $ fn () $ option:unwrap-or opt 0)) (f)",
      "'Number",
    ),
  ] {
    let target = format!("fix-command.main/{definition}");
    assert_success(
      &run_calcit(&snapshot, &["edit", "def", &target, "--code", code]),
      "install macro-wrapped helper call",
    );
    let schema = format!("quote $ :: 'Fn $ {{}} (:args $ [] (:: 'Option 'Number)) (:return {return_type})");
    assert_success(
      &run_calcit(&snapshot, &["edit", "schema", &target, "--code", &schema]),
      "declare Option receiver schema",
    );
    let behavior = if definition == "asserted-option" {
      "quote $ asserted-option $ Option :some 2"
    } else {
      "quote $ assert= 2 $ closure-option $ Option :some 2"
    };
    assert_success(
      &run_calcit(&snapshot, &["edit", "add-test", &target, "preserves-result", "--code", behavior]),
      "attach Calcit behavior test",
    );
    let preview = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-option-method-v1",
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--format",
        "json",
      ],
    );
    assert_success(&preview, "macro-wrapped Option method preview");
    let report = parse_stdout(&preview);
    assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
    assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
    let applied = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-option-method-v1",
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--apply",
        "--allow-no-vcs",
        "--expect-revision",
        report["revision"].as_str().expect("preview revision"),
        "--format",
        "json",
      ],
    );
    assert_success(&applied, "macro-wrapped Option method apply");
    let repeated = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-option-method-v1",
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--format",
        "json",
      ],
    );
    assert_success(&repeated, "macro-wrapped Option method idempotence");
    assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "Calcit behavior test after macro-wrapped migration",
    );
  }
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/gated-option",
        "--code",
        "quote $ defn gated-option (opt) $ if-not false (option:unwrap-or opt 0) 0",
      ],
    ),
    "install unapproved macro wrapper",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/gated-option",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] (:: 'Option 'Number)) (:return 'Number)",
      ],
    ),
    "declare gated Option schema",
  );
  let unknown_macro = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "gated-option",
      "--format",
      "json",
    ],
  );
  assert_success(&unknown_macro, "unapproved macro preview");
  let report = parse_stdout(&unknown_macro);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review", "{report}");
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
}

#[test]
fn core_option_method_rule_resolves_short_option_name_in_source_namespace() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/test-types-inference.cirru", &snapshot).expect("copy Calcit inference Snapshot");
  for definition in ["infer-raise-left", "infer-raise-right"] {
    let target = format!("test-types-inference.main/{definition}");
    assert_success(
      &run_calcit(
        &snapshot,
        &[
          "tree",
          "replace",
          &target,
          "--path",
          "@3.2",
          "--input-format",
          "cirru",
          "--expect",
          "quote $ value .unwrap",
          "--code",
          "quote $ option:unwrap value",
        ],
      ),
      "restore legacy source call in temporary Snapshot",
    );
    let args = [
      "--rule",
      "core-option-method-v1",
      "--ns",
      "test-types-inference.main",
      "--def",
      definition,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &args);
    assert_success(&preview, "short core Option preview");
    let report = parse_stdout(&preview);
    assert_eq!(report["data"]["suggestions"][0]["applicability"], "machine-applicable", "{report}");
    assert_eq!(report["data"]["validation"]["status"], "passed", "{report}");
    let applied = run_fix(
      &snapshot,
      &[
        "--rule",
        "core-option-method-v1",
        "--ns",
        "test-types-inference.main",
        "--def",
        definition,
        "--apply",
        "--allow-no-vcs",
        "--expect-revision",
        report["revision"].as_str().expect("preview revision"),
        "--format",
        "json",
      ],
    );
    assert_success(&applied, "short core Option apply");
    let repeated = run_fix(&snapshot, &args);
    assert_success(&repeated, "short core Option idempotence");
    assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
    assert_success(
      &run_calcit(&snapshot, &["test", &target, "--require-match"]),
      "Calcit Option/raise behavior test",
    );
  }
}

#[test]
fn scoped_fix_preserves_unselected_short_schema_surface() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("calcit/test-types-inference.cirru", &snapshot).expect("copy Calcit inference Snapshot");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "tree",
        "replace",
        "test-types-inference.main/infer-raise-left",
        "--path",
        "@3.2",
        "--input-format",
        "cirru",
        "--expect",
        "quote $ value .unwrap",
        "--code",
        "quote $ option:unwrap value",
      ],
    ),
    "restore legacy helper in a temporary Snapshot",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "test-types-inference.main/dynamic-last-compat",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'Dynamic) (:return $ :: 'Option 'Dynamic)",
      ],
    ),
    "restore the unrelated short Option schema",
  );
  let before = fs::read_to_string(&snapshot).expect("read baseline Snapshot");
  assert!(before.contains(":return $ :: 'Option 'Dynamic"));
  assert_eq!(before.matches("option:unwrap value").count(), 1);

  let args = [
    "--rule",
    "core-option-method-v1",
    "--ns",
    "test-types-inference.main",
    "--def",
    "infer-raise-left",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &args);
  assert_success(&preview, "preview scoped Option migration");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(1));
  assert_eq!(fs::read_to_string(&snapshot).expect("read after preview"), before);
  let applied = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "test-types-inference.main",
      "--def",
      "infer-raise-left",
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().expect("preview revision"),
      "--format",
      "json",
    ],
  );
  assert_success(&applied, "apply scoped Option migration");
  assert_eq!(parse_stdout(&applied)["data"]["new_revision"], report["data"]["new_revision"]);
  let after = fs::read_to_string(&snapshot).expect("read applied Snapshot");
  assert_eq!(after, before.replacen("option:unwrap value", "value .unwrap", 1));
  let repeated = run_fix(&snapshot, &args);
  assert_success(&repeated, "repeat scoped Option migration");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn core_option_method_rule_does_not_assume_shadowed_short_option_is_core() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/Option",
        "--code",
        "quote $ defenum Option ([] 'T) (:some 'T) (:none)",
      ],
    ),
    "install local Option enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "fix-command.main/Option", "--code", "quote $ :: 'EnumDef"],
    ),
    "declare local Option enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/shadowed-option",
        "--code",
        "quote $ defn shadowed-option () $ option:unwrap $ Option :some 2",
      ],
    ),
    "install core helper call with local Option constructor",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/shadowed-option",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return 'Number)",
      ],
    ),
    "declare helper schema",
  );
  let preview = run_fix(
    &snapshot,
    &[
      "--rule",
      "core-option-method-v1",
      "--ns",
      "fix-command.main",
      "--def",
      "shadowed-option",
      "--format",
      "json",
    ],
  );
  assert_success(&preview, "shadowed short Option preview");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["suggestions"][0]["applicability"], "requires-review", "{report}");
  assert_eq!(report["data"]["suggestions"][0]["replacement"], serde_json::Value::Null);
}

#[test]
fn imported_callable_return_keeps_its_declaration_namespace() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  assert_success(
    &run_calcit(&snapshot, &["edit", "add-ns", "fix-command.foreign"]),
    "add foreign namespace",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.foreign/Option",
        "--code",
        "quote $ defenum Option ([] 'T) (:some 'T) (:none)",
      ],
    ),
    "install foreign Option enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "fix-command.foreign/Option", "--code", "quote $ :: 'EnumDef"],
    ),
    "declare foreign Option enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.foreign/make-option",
        "--code",
        "quote $ defn make-option () $ Option :some 2",
      ],
    ),
    "install foreign Option constructor",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.foreign/make-option",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'Option 'Number)",
      ],
    ),
    "declare short foreign Option return",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/foreign-value",
        "--code",
        "quote $ defn foreign-value () $ fix-command.foreign/make-option",
      ],
    ),
    "install cross-namespace caller",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.main/foreign-value",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ []) (:return $ :: 'fix-command.foreign/Option 'Number)",
      ],
    ),
    "declare qualified consumer return",
  );
  let query = run_calcit(
    &snapshot,
    &[
      "query",
      "type-at",
      "fix-command.main/foreign-value",
      "--path",
      "@3",
      "--format",
      "json",
    ],
  );
  assert_success(&query, "query imported callable return type");
  let report = parse_stdout(&query);
  assert_eq!(
    report["data"]["inferred_type"], ":: 'fix-command.foreign/Option 'Number",
    "{report}"
  );
  assert!(
    report["data"]["static_methods"].as_array().is_none_or(|methods| !methods
      .iter()
      .any(|method| method["name"] == ".unwrap" && method["status"] == "proven")),
    "foreign Option must not inherit core methods: {report}"
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.foreign/make-option-hinted",
        "--code",
        "quote $ defn make-option-hinted () (hint-fn $ {} (:args $ []) (:return $ :: 'Option 'Number)) (Option :some 3)",
      ],
    ),
    "install hint-only foreign callable",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/foreign-hinted-value",
        "--code",
        "quote $ defn foreign-hinted-value () $ fix-command.foreign/make-option-hinted",
      ],
    ),
    "install caller of hint-only foreign callable",
  );
  let hinted = run_calcit(
    &snapshot,
    &[
      "query",
      "type-at",
      "fix-command.main/foreign-hinted-value",
      "--path",
      "@3",
      "--format",
      "json",
    ],
  );
  assert_success(&hinted, "query hint-only imported return type");
  let report = parse_stdout(&hinted);
  assert_eq!(
    report["data"]["inferred_type"], ":: 'fix-command.foreign/Option 'Number",
    "{report}"
  );

  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.foreign/pass-through",
        "--code",
        "quote $ defn pass-through (value) value",
      ],
    ),
    "install cross-namespace generic helper",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "schema",
        "fix-command.foreign/pass-through",
        "--code",
        "quote $ :: 'Fn $ {} (:generics $ [] 'T) (:args $ [] 'T) (:return 'T)",
      ],
    ),
    "declare generic identity contract",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/Option",
        "--code",
        "quote $ defenum Option ([] 'T) (:some 'T) (:none)",
      ],
    ),
    "install caller-owned Option enum",
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &["edit", "schema", "fix-command.main/Option", "--code", "quote $ :: 'EnumDef"],
    ),
    "declare caller-owned Option enum",
  );
  let foreign_again = run_calcit(
    &snapshot,
    &[
      "query",
      "type-at",
      "fix-command.main/foreign-value",
      "--path",
      "@3",
      "--format",
      "json",
    ],
  );
  assert_success(&foreign_again, "query foreign return after caller shadowing");
  let foreign_again_report = parse_stdout(&foreign_again);
  assert_eq!(
    foreign_again_report["data"]["inferred_type"], ":: 'fix-command.foreign/Option 'Number",
    "{foreign_again_report}"
  );
  assert_success(
    &run_calcit(
      &snapshot,
      &[
        "edit",
        "def",
        "fix-command.main/passed-option",
        "--code",
        "quote $ defn passed-option () $ fix-command.foreign/pass-through $ assert-type (fix-command.main/Option :some 4) $ :: 'fix-command.main/Option 'Number",
      ],
    ),
    "install generic caller",
  );
  let passed = run_calcit(
    &snapshot,
    &[
      "query",
      "type-at",
      "fix-command.main/passed-option",
      "--path",
      "@3",
      "--format",
      "json",
    ],
  );
  assert_success(&passed, "query generic caller-owned return type");
  let report = parse_stdout(&passed);
  assert_eq!(report["data"]["inferred_type"], ":: 'fix-command.main/Option 'Number", "{report}");
}

#[test]
fn surface_latest_preset_migrates_named_enum_and_struct_constructors() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let enum_preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-enum",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(
    enum_preview.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&enum_preview.stderr)
  );
  let report = parse_stdout(&enum_preview);
  assert_eq!(report["data"]["filters"]["preset_id"], "surface-latest-v1");
  assert_eq!(report["data"]["filters"]["expanded_rule_ids"].as_array().map(Vec::len), Some(4));
  assert_eq!(report["data"]["filters"]["expanded_rules"].as_array().map(Vec::len), Some(4));
  assert!(
    report["data"]["filters"]["expanded_rules"]
      .as_array()
      .expect("expanded rule metadata should be an array")
      .iter()
      .all(|rule| rule["lifecycle"] == "current-semantics" && rule["source_version_required"] == false)
  );
  assert_eq!(
    report["data"]["filters"]["expanded_rules"][0]["evidence_source"],
    "current-diagnostic"
  );
  assert_eq!(report["data"]["suggestions"][0]["rule_id"], "named-enum-constructor-v1");

  let enum_applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-enum",
      "--preset",
      "surface-latest-v1",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    enum_applied.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&enum_applied.stderr)
  );

  let struct_preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-struct",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(
    struct_preview.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&struct_preview.stderr)
  );
  assert_eq!(
    parse_stdout(&struct_preview)["data"]["suggestions"][0]["rule_id"],
    "named-struct-constructor-v1"
  );
  let struct_applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-struct",
      "--preset",
      "surface-latest-v1",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    struct_applied.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&struct_applied.stderr)
  );

  let nested_applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-nested",
      "--preset",
      "surface-latest-v1",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(
    nested_applied.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&nested_applied.stderr)
  );
  for definition in ["legacy-enum-overlap", "legacy-struct-overlap"] {
    let applied = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--preset",
        "surface-latest-v1",
        "--apply",
        "--allow-no-vcs",
        "--format",
        "json",
      ],
    );
    assert!(
      applied.status.success(),
      "overlap migration failed for {definition}:\n{}",
      String::from_utf8_lossy(&applied.stderr)
    );
  }
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("defn legacy-enum () (FixPersonChoice :none)"));
  assert!(updated.contains("defn legacy-struct () (FixPerson :name |Ada :age 1)"));
  assert!(updated.contains("FixPersonChoice :person $ FixPerson :name |Ada :age 1"));

  for definition in [
    "legacy-enum",
    "legacy-struct",
    "legacy-nested",
    "legacy-enum-overlap",
    "legacy-struct-overlap",
  ] {
    let target = format!("fix-command.main/{definition}");
    let calcit_test = run_calcit(&snapshot, &["test", target.as_str(), "--require-match"]);
    assert!(
      calcit_test.status.success(),
      "Calcit definition test failed for {definition}:\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&calcit_test.stdout),
      String::from_utf8_lossy(&calcit_test.stderr)
    );
  }

  for definition in [
    "legacy-enum",
    "legacy-struct",
    "legacy-nested",
    "legacy-enum-overlap",
    "legacy-struct-overlap",
  ] {
    let repeated = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--preset",
        "surface-latest-v1",
        "--format",
        "json",
      ],
    );
    assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
    assert_eq!(parse_stdout(&repeated)["data"]["changed"], false);
  }
}

#[test]
fn surface_latest_v2_unwraps_single_expression_do_without_changing_v1() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");

  let v1_preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(
    v1_preview.status.success(),
    "stderr:\n{}",
    String::from_utf8_lossy(&v1_preview.stderr)
  );
  let v1_report = parse_stdout(&v1_preview);
  assert_eq!(v1_report["data"]["changed"], false);
  assert_eq!(
    v1_report["data"]["filters"]["expanded_rule_ids"],
    serde_json::json!([
      "removed-data-api-v1",
      "named-enum-constructor-v1",
      "named-struct-constructor-v1",
      "redundant-do-v1"
    ])
  );

  let preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v2",
      "--format",
      "json",
    ],
  );
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["filters"]["preset_id"], "surface-latest-v2");
  assert_eq!(
    report["data"]["filters"]["expanded_rule_ids"],
    serde_json::json!([
      "removed-data-api-v1",
      "named-enum-constructor-v1",
      "named-struct-constructor-v1",
      "redundant-do-v1",
      "single-expression-do-v1"
    ])
  );
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(4));
  assert!(
    report["data"]["suggestions"]
      .as_array()
      .expect("suggestions should be an array")
      .iter()
      .all(|suggestion| suggestion["rule_id"] == "single-expression-do-v1")
  );

  let applied = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v2",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(applied.status.success(), "stderr:\n{}", String::from_utf8_lossy(&applied.stderr));
  let updated = fs::read_to_string(&snapshot).expect("updated fixture should read");
  assert!(updated.contains("'single-do-positions"));
  assert!(updated.contains("chosen value"));
  assert!(updated.contains("if true (+ chosen 1) 0"));

  let calcit_test = run_calcit(&snapshot, &["test", "fix-command.main/single-do-positions", "--require-match"]);
  assert!(
    calcit_test.status.success(),
    "Calcit definition test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_test.stdout),
    String::from_utf8_lossy(&calcit_test.stderr)
  );

  let repeated = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "single-do-positions",
      "--preset",
      "surface-latest-v2",
      "--format",
      "json",
    ],
  );
  assert!(repeated.status.success(), "stderr:\n{}", String::from_utf8_lossy(&repeated.stderr));
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"].as_array().map(Vec::len), Some(0));

  let overlap = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "legacy-enum-overlap",
      "--preset",
      "surface-latest-v2",
      "--apply",
      "--allow-no-vcs",
      "--format",
      "json",
    ],
  );
  assert!(overlap.status.success(), "stderr:\n{}", String::from_utf8_lossy(&overlap.stderr));
  let overlap_test = run_calcit(&snapshot, &["test", "fix-command.main/legacy-enum-overlap", "--require-match"]);
  assert!(
    overlap_test.status.success(),
    "Calcit overlap test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&overlap_test.stdout),
    String::from_utf8_lossy(&overlap_test.stderr)
  );

  for definition in ["quoted-single-do", "single-do-macro"] {
    let preview = run_fix(
      &snapshot,
      &[
        "--ns",
        "fix-command.main",
        "--def",
        definition,
        "--preset",
        "surface-latest-v2",
        "--format",
        "json",
      ],
    );
    assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
    assert_eq!(parse_stdout(&preview)["data"]["suggestions"].as_array().map(Vec::len), Some(0));
    let target = format!("fix-command.main/{definition}");
    let calcit_test = run_calcit(&snapshot, &["test", target.as_str(), "--require-match"]);
    assert!(
      calcit_test.status.success(),
      "Calcit definition test failed for {definition}:\nstdout:\n{}\nstderr:\n{}",
      String::from_utf8_lossy(&calcit_test.stdout),
      String::from_utf8_lossy(&calcit_test.stderr)
    );
  }
}

#[test]
fn core_api_028_preset_composes_nested_leaf_renames_and_is_idempotent() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let target = "fix-command.main/api-alias-summary";
  for args in [
    vec![
      "edit",
      "def",
      "fix-command.main/api-summary-label",
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn api-summary-label (text)\n  , text",
    ],
    vec![
      "edit",
      "schema",
      "fix-command.main/api-summary-label",
      "--input-format",
      "cirru",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] 'String) (:return 'String)",
    ],
    vec![
      "edit",
      "def",
      target,
      "--input-format",
      "cirru",
      "--code",
      "quote $ defn api-alias-summary (xs)\n  api-summary-label $ &str:concat (turn-string $ .count $ .add xs 3) $ &str:concat |: $ turn-string $ round? 2",
    ],
    vec![
      "edit",
      "schema",
      target,
      "--input-format",
      "cirru",
      "--code",
      "quote $ :: 'Fn $ {} (:args $ [] $ :: 'List 'Number) (:return 'String)",
    ],
    vec![
      "edit",
      "add-test",
      target,
      "nested-alias-semantics",
      "--tags",
      "unit",
      "--input-format",
      "cirru",
      "--code",
      "quote $ do\n  assert= |4:true $ api-alias-summary ([] 1 2 3)\n  assert= |1:true $ api-alias-summary ([])",
    ],
  ] {
    assert_success(&run_calcit(&snapshot, &args), "install nested API fixture through Calcit CLI");
  }
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "original Calcit semantics",
  );
  let original = fs::read(&snapshot).expect("original snapshot");
  let old = run_fix(
    &snapshot,
    &[
      "--preset",
      "surface-latest-v2",
      "--ns",
      "fix-command.main",
      "--def",
      "api-alias-summary",
      "--format",
      "json",
    ],
  );
  assert_success(&old, "frozen surface preset");
  assert_eq!(parse_stdout(&old)["data"]["suggestions"], serde_json::json!([]));

  let selectors = [
    "--preset",
    "core-api-0.28-v1",
    "--ns",
    "fix-command.main",
    "--def",
    "api-alias-summary",
    "--format",
    "json",
  ];
  let preview = run_fix(&snapshot, &selectors);
  assert_success(&preview, "composed API preview");
  assert_eq!(fs::read(&snapshot).unwrap(), original, "preview must preserve source");
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["filters"]["preset_id"], "core-api-0.28-v1");
  assert_eq!(
    report["data"]["filters"]["source_coverage"]["scanned_regions"],
    serde_json::json!(["code"])
  );
  assert_eq!(
    report["data"]["filters"]["source_coverage"]["manual_review_regions"],
    serde_json::json!(["tests", "examples"])
  );
  assert_eq!(report["data"]["filters"]["expanded_rule_ids"].as_array().unwrap().len(), 15);
  let suggestions = report["data"]["suggestions"].as_array().expect("suggestions");
  assert_eq!(suggestions.len(), 5, "nested aliases must all compose: {report}");
  assert!(
    suggestions
      .iter()
      .all(|suggestion| suggestion["applicability"] == "machine-applicable"),
    "{report}"
  );
  let mut apply = selectors.to_vec();
  apply.extend([
    "--apply",
    "--allow-no-vcs",
    "--expect-revision",
    report["revision"].as_str().unwrap(),
  ]);
  assert_success(&run_fix(&snapshot, &apply), "guarded API apply");
  assert_success(
    &run_calcit(&snapshot, &["test", target, "--require-match"]),
    "migrated Calcit semantics",
  );
  let repeated = run_fix(&snapshot, &selectors);
  assert_success(&repeated, "repeated API preview");
  assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
}

#[test]
fn preset_and_rule_selection_conflict() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let output = run_fix(
    &snapshot,
    &["--preset", "surface-latest-v1", "--rule", "redundant-do-v1", "--format", "json"],
  );
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("conflicts with `--preset`"));
}

#[test]
fn rename_module_definitions_to_public_file_names_preserves_attached_tests() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  for (old, new) in [("legacy-file-reader", "read-file"), ("legacy-dir-reader", "read-dir")] {
    let target = format!("fix-command.main/{old}");
    let code = format!("quote $ defn {old} (path) $ &str:concat path |!");
    let test = format!("quote $ if (&= |example! $ {old} |example) &unit $ raise |module-reader-contract-failed");
    for args in [
      vec!["edit", "def", &target, "--input-format", "cirru", "--code", &code],
      vec![
        "edit",
        "schema",
        &target,
        "--input-format",
        "cirru",
        "--code",
        "quote $ :: 'Fn $ {} (:args $ [] 'String) (:return 'String)",
      ],
      vec![
        "edit",
        "add-test",
        &target,
        "preserves-module-reader",
        "--input-format",
        "cirru",
        "--code",
        &test,
      ],
    ] {
      assert_success(&run_calcit(&snapshot, &args), "install module definition through Calcit CLI");
    }
    let selectors = [
      "--rule",
      "rename-definition-v1",
      "--ns",
      "fix-command.main",
      "--def",
      old,
      "--to",
      new,
      "--format",
      "json",
    ];
    let preview = run_fix(&snapshot, &selectors);
    assert_success(&preview, "public file name rename preview");
    let report = parse_stdout(&preview);
    let mut apply = selectors.to_vec();
    apply.extend([
      "--apply",
      "--allow-no-vcs",
      "--expect-revision",
      report["revision"].as_str().unwrap(),
    ]);
    assert_success(&run_fix(&snapshot, &apply), "public file name rename apply");
    let renamed = format!("fix-command.main/{new}");
    assert_success(
      &run_calcit(&snapshot, &["test", &renamed, "--require-match"]),
      "renamed module definition attached semantics",
    );
    let repeated = run_fix(&snapshot, &selectors);
    assert_success(&repeated, "rename repeat");
    assert_eq!(parse_stdout(&repeated)["data"]["suggestions"], serde_json::json!([]));
  }
}

#[test]
fn named_constructor_preset_preserves_quoted_data() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::copy("tests/fixtures/fix-command.cirru", &snapshot).expect("fixture should copy");
  let preview = run_fix(
    &snapshot,
    &[
      "--ns",
      "fix-command.main",
      "--def",
      "quoted-legacy-constructor",
      "--preset",
      "surface-latest-v1",
      "--format",
      "json",
    ],
  );
  assert!(preview.status.success(), "stderr:\n{}", String::from_utf8_lossy(&preview.stderr));
  let report = parse_stdout(&preview);
  assert_eq!(report["data"]["changed"], false);
  assert_eq!(report["data"]["suggestions"].as_array().map(Vec::len), Some(0));

  let target = "fix-command.main/quoted-legacy-constructor";
  let calcit_test = run_calcit(&snapshot, &["test", target, "--require-match"]);
  assert!(
    calcit_test.status.success(),
    "Calcit definition test failed:\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&calcit_test.stdout),
    String::from_utf8_lossy(&calcit_test.stderr)
  );
}

const FFI_BOUNDARY_FIXTURE: &str = r#"
{}
  :package |ffi-boundary
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'ffi-boundary.main/main!) (:mode :native) (:reload-fn 'ffi-boundary.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'ffi-boundary.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'raw-read! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn raw-read! (element) (.-length element)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'JsObject
            :features $ #{} :js-ffi
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns ffi-boundary.main
"#;

#[test]
fn strict_workflow_surfaces_ffi_boundary_defexternal_skeletons() {
  let directory = TestDirectory::create();
  let snapshot = directory.path().join("calcit.cirru");
  fs::write(&snapshot, FFI_BOUNDARY_FIXTURE).expect("ffi boundary fixture should write");

  let preview = run_fix(&snapshot, &["--workflow", "strict", "--format", "json"]);
  assert!(!preview.status.success(), "untyped field access must remain a compiler error");
  let report = parse_stdout(&preview);
  assert!(
    report["diagnostics"]
      .as_array()
      .unwrap()
      .iter()
      .any(|item| item["code"] == "E_UNTYPED_JS_OBJECT_ACCESS")
  );
  let boundaries = report["data"]["workflow"]["review_required"]["ffi_boundaries"]
    .as_array()
    .expect("ffi boundaries should be an array");
  let candidate = boundaries
    .iter()
    .find(|boundary| boundary["definition"] == "ffi-boundary.main/raw-read!")
    .and_then(|boundary| boundary["trait_candidates"].as_array())
    .and_then(|candidates| candidates.iter().find(|candidate| candidate["suggested_name"] == "RawReadHost"))
    .expect("a trait candidate should be reported for the bare field read");
  assert_eq!(candidate["contract_status"], "review-required");
  assert_eq!(candidate["defexternal_skeleton"], "defexternal RawReadHost\n  :length 'Dynamic");
}

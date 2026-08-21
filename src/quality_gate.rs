//! Native static-quality budgets for `calcit analyze quality`.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use calcit::cli_args::{CheckTypesCommand, DeprecatedCommand, QualityCommand, WeakTypesCommand};
use calcit::snapshot;
use serde::{Deserialize, Serialize};

use crate::deprecated_api;
use crate::type_coverage::{self, CoverageLevel, WeakTypeIntent, WeakTypeKind};

/// Version 2 adds the explicit unsafe-host-boundary budget. Version 1 native
/// baselines remain valid and are read with a zero budget for that new metric,
/// so existing projects can migrate deliberately instead of silently dropping
/// the new check.
const QUALITY_BASELINE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityMetrics {
  pub type_none: usize,
  pub type_not_full: usize,
  pub schema_dynamic: usize,
  pub code_dynamic: usize,
  pub code_nil: usize,
  pub unresolved: usize,
  pub declared_optional: usize,
  pub deprecated_calls: usize,
  #[serde(default)]
  pub unsafe_coerce: usize,
}

impl QualityMetrics {
  fn values(&self) -> [(&'static str, usize); 9] {
    [
      ("typeNone", self.type_none),
      ("typeNotFull", self.type_not_full),
      ("schemaDynamic", self.schema_dynamic),
      ("codeDynamic", self.code_dynamic),
      ("codeNil", self.code_nil),
      ("unresolved", self.unresolved),
      ("declaredOptional", self.declared_optional),
      ("deprecatedCalls", self.deprecated_calls),
      ("unsafeCoerce", self.unsafe_coerce),
    ]
  }

  fn value(&self, metric: &str) -> usize {
    match metric {
      "typeNone" => self.type_none,
      "typeNotFull" => self.type_not_full,
      "schemaDynamic" => self.schema_dynamic,
      "codeDynamic" => self.code_dynamic,
      "codeNil" => self.code_nil,
      "unresolved" => self.unresolved,
      "declaredOptional" => self.declared_optional,
      "deprecatedCalls" => self.deprecated_calls,
      "unsafeCoerce" => self.unsafe_coerce,
      _ => unreachable!("unknown quality metric: {metric}"),
    }
  }

  fn add_assign(&mut self, other: &Self) {
    self.type_none += other.type_none;
    self.type_not_full += other.type_not_full;
    self.schema_dynamic += other.schema_dynamic;
    self.code_dynamic += other.code_dynamic;
    self.code_nil += other.code_nil;
    self.unresolved += other.unresolved;
    self.declared_optional += other.declared_optional;
    self.deprecated_calls += other.deprecated_calls;
    self.unsafe_coerce += other.unsafe_coerce;
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualityScope {
  pub namespace: Option<String>,
  pub namespace_prefix: Option<String>,
  pub include_dependencies: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QualityBaseline {
  schema_version: u32,
  scope: QualityScope,
  metrics: QualityMetrics,
  definitions: BTreeMap<String, QualityMetrics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityViolation {
  pub definition: Option<String>,
  pub metric: String,
  pub actual: usize,
  pub limit: usize,
  pub delta: usize,
}

#[derive(Debug, Clone)]
struct QualitySnapshot {
  revision: String,
  metrics: QualityMetrics,
  definitions: BTreeMap<String, QualityMetrics>,
}

#[derive(Debug, Clone)]
pub struct QualityOutcome {
  pub revision: String,
  pub scope: QualityScope,
  pub mode: String,
  pub baseline_path: Option<String>,
  pub metrics: QualityMetrics,
  pub limits: Option<QualityMetrics>,
  pub deltas: BTreeMap<String, i64>,
  pub violations: Vec<QualityViolation>,
  pub passed: bool,
}

fn quality_scope(options: &QualityCommand) -> QualityScope {
  QualityScope {
    namespace: options.ns.clone(),
    namespace_prefix: options.ns_prefix.clone(),
    include_dependencies: options.deps,
  }
}

fn collect_quality_snapshot(options: &QualityCommand, snapshot: &snapshot::Snapshot) -> Result<QualitySnapshot, String> {
  let check_options = CheckTypesCommand {
    ns: options.ns.clone(),
    ns_prefix: options.ns_prefix.clone(),
    only: None,
    format: "json".to_owned(),
    deps: options.deps,
    summary_only: false,
  };
  let weak_options = WeakTypesCommand {
    ns: options.ns.clone(),
    ns_prefix: options.ns_prefix.clone(),
    only: Some("schema-dynamic,unresolved-type-slot,code-dynamic,code-nil,unsafe-coerce".to_owned()),
    intent: Some("unresolved,declared-optional,explicit-unsafe".to_owned()),
    format: "json".to_owned(),
    deps: options.deps,
    summary_only: false,
  };
  let deprecated_options = DeprecatedCommand {
    ns: options.ns.clone(),
    ns_prefix: options.ns_prefix.clone(),
    format: "json".to_owned(),
    deps: options.deps,
    summary_only: false,
  };

  let coverage_rows = type_coverage::collect_type_coverage_rows(&check_options, snapshot)?;
  let weak_rows = type_coverage::collect_weak_type_rows(&weak_options, snapshot)?;
  let deprecated_rows = deprecated_api::collect_deprecated_api_rows(&deprecated_options, snapshot)?;
  let revision_ids = coverage_rows
    .iter()
    .map(|row| (row.ns.clone(), row.def.clone()))
    .collect::<Vec<_>>();
  let revision = type_coverage::analysis_revision(snapshot, &revision_ids)?;
  let mut definitions = BTreeMap::<String, QualityMetrics>::new();

  for row in coverage_rows {
    let metrics = definitions.entry(format!("{}/{}", row.ns, row.def)).or_default();
    match row.level {
      CoverageLevel::None => {
        metrics.type_none = 1;
        metrics.type_not_full = 1;
      }
      CoverageLevel::Partial => metrics.type_not_full = 1,
      CoverageLevel::Full => {}
    }
  }

  for row in weak_rows {
    let metrics = definitions.entry(format!("{}/{}", row.ns, row.def)).or_default();
    for occurrence in row.occurrences {
      match occurrence.kind {
        WeakTypeKind::SchemaDynamic => metrics.schema_dynamic += 1,
        WeakTypeKind::UnresolvedTypeSlot => {}
        WeakTypeKind::CodeDynamic => metrics.code_dynamic += 1,
        WeakTypeKind::CodeNil => metrics.code_nil += 1,
        // Unlike Dynamic debt, this is an explicit boundary. It has its own
        // budget so intentional adapters remain visible without being folded
        // into unresolved inference failures.
        WeakTypeKind::UnsafeCoerce => metrics.unsafe_coerce += 1,
      }
      match occurrence.intent {
        WeakTypeIntent::Unresolved => metrics.unresolved += 1,
        WeakTypeIntent::DeclaredOptional => metrics.declared_optional += 1,
        WeakTypeIntent::IntentionalJsFfi
        | WeakTypeIntent::IntentionalTypeSlotDynamic
        | WeakTypeIntent::ExplicitUnsafe
        | WeakTypeIntent::DeclaredUnit => {}
      }
    }
  }

  for row in deprecated_rows {
    definitions
      .entry(format!("{}/{}", row.namespace, row.definition))
      .or_default()
      .deprecated_calls += row.uses.len();
  }

  let metrics = sum_metrics(definitions.values());
  Ok(QualitySnapshot {
    revision,
    metrics,
    definitions,
  })
}

fn sum_metrics<'a>(items: impl IntoIterator<Item = &'a QualityMetrics>) -> QualityMetrics {
  let mut total = QualityMetrics::default();
  for item in items {
    total.add_assign(item);
  }
  total
}

fn compare_metrics(
  actual: &QualityMetrics,
  limit: &QualityMetrics,
  definition: Option<&str>,
  include_unsafe_coerce: bool,
) -> Vec<QualityViolation> {
  actual
    .values()
    .into_iter()
    .filter(|(metric, _)| include_unsafe_coerce || *metric != "unsafeCoerce")
    .filter_map(|(metric, actual)| {
      let limit = limit.value(metric);
      (actual > limit).then(|| QualityViolation {
        definition: definition.map(str::to_owned),
        metric: metric.to_owned(),
        actual,
        limit,
        delta: actual - limit,
      })
    })
    .collect()
}

fn compare_detailed_baseline(current: &QualitySnapshot, baseline: &QualityBaseline) -> Vec<QualityViolation> {
  let mut violations = vec![];
  let include_unsafe_coerce = baseline.schema_version >= 2;
  for (definition, actual) in &current.definitions {
    let limit = baseline.definitions.get(definition).cloned().unwrap_or_default();
    violations.extend(compare_metrics(actual, &limit, Some(definition), include_unsafe_coerce));
  }
  violations.sort_by(|left, right| left.definition.cmp(&right.definition).then(left.metric.cmp(&right.metric)));
  violations
}

fn reported_baseline_limits(current: &QualitySnapshot, baseline: &QualityBaseline) -> QualityMetrics {
  let mut limits = baseline.metrics.clone();
  // v1 predates the explicit unsafe metric. Preserve its original eight-metric
  // gate and avoid reporting an unenforced positive delta as a regression.
  if baseline.schema_version == 1 {
    limits.unsafe_coerce = current.metrics.unsafe_coerce;
  }
  limits
}

fn metric_deltas(actual: &QualityMetrics, limit: &QualityMetrics) -> BTreeMap<String, i64> {
  actual
    .values()
    .into_iter()
    .map(|(metric, actual)| (metric.to_owned(), actual as i64 - limit.value(metric) as i64))
    .collect()
}

fn read_baseline(path: &Path) -> Result<Result<QualityBaseline, QualityMetrics>, String> {
  let content = fs::read_to_string(path).map_err(|error| format!("Failed to read quality baseline '{}': {error}", path.display()))?;
  let value: serde_json::Value =
    serde_json::from_str(&content).map_err(|error| format!("Failed to parse quality baseline '{}': {error}", path.display()))?;
  if value.get("schemaVersion").is_some() {
    let baseline: QualityBaseline =
      serde_json::from_value(value).map_err(|error| format!("Invalid native quality baseline '{}': {error}", path.display()))?;
    if !matches!(baseline.schema_version, 1 | QUALITY_BASELINE_SCHEMA_VERSION) {
      return Err(format!(
        "Unsupported quality baseline schemaVersion {} in '{}'; expected 1 or {}.",
        baseline.schema_version,
        path.display(),
        QUALITY_BASELINE_SCHEMA_VERSION
      ));
    }
    let summed = sum_metrics(baseline.definitions.values());
    if summed != baseline.metrics {
      return Err(format!(
        "Invalid native quality baseline '{}': top-level metrics do not equal the per-definition totals.",
        path.display()
      ));
    }
    Ok(Ok(baseline))
  } else {
    let metrics: QualityMetrics =
      serde_json::from_value(value).map_err(|error| format!("Invalid legacy quality baseline '{}': {error}", path.display()))?;
    Ok(Err(metrics))
  }
}

fn write_baseline(path: &Path, scope: &QualityScope, current: &QualitySnapshot) -> Result<(), String> {
  let zero = QualityMetrics::default();
  let baseline = QualityBaseline {
    schema_version: QUALITY_BASELINE_SCHEMA_VERSION,
    scope: scope.clone(),
    metrics: current.metrics.clone(),
    definitions: current
      .definitions
      .iter()
      .filter(|(_, metrics)| *metrics != &zero)
      .map(|(definition, metrics)| (definition.clone(), metrics.clone()))
      .collect(),
  };
  let mut content = serde_json::to_string_pretty(&baseline)
    .map_err(|error| format!("Failed to encode quality baseline '{}': {error}", path.display()))?;
  content.push('\n');
  let staged = crate::cli_handlers::stage_atomic_file(path, content.as_bytes(), "quality baseline")?;
  staged.commit()
}

pub fn analyze_quality(options: &QualityCommand, snapshot: &snapshot::Snapshot) -> Result<QualityOutcome, String> {
  if options.baseline.is_some() && options.write_baseline.is_some() {
    return Err("`--baseline` and `--write-baseline` cannot be used together.".to_owned());
  }

  let scope = quality_scope(options);
  let current = collect_quality_snapshot(options, snapshot)?;

  if let Some(path) = &options.write_baseline {
    write_baseline(Path::new(path), &scope, &current)?;
    return Ok(QualityOutcome {
      revision: current.revision,
      scope,
      mode: "write-baseline".to_owned(),
      baseline_path: Some(path.clone()),
      metrics: current.metrics,
      limits: None,
      deltas: BTreeMap::new(),
      violations: vec![],
      passed: true,
    });
  }

  let (mode, baseline_path, limits, violations) = if let Some(path) = &options.baseline {
    match read_baseline(Path::new(path))? {
      Ok(baseline) => {
        if baseline.scope != scope {
          return Err(format!(
            "Quality baseline scope does not match this command. Baseline: {:?}; current: {:?}. Use the same --ns/--ns-prefix/--deps flags or regenerate it.",
            baseline.scope, scope
          ));
        }
        let violations = compare_detailed_baseline(&current, &baseline);
        let limits = reported_baseline_limits(&current, &baseline);
        let mode = if baseline.schema_version == 1 {
          "native-baseline-v1"
        } else {
          "native-baseline"
        };
        (mode.to_owned(), Some(path.clone()), limits, violations)
      }
      Err(legacy_limits) => {
        let violations = compare_metrics(&current.metrics, &legacy_limits, None, false);
        ("legacy-baseline".to_owned(), Some(path.clone()), legacy_limits, violations)
      }
    }
  } else {
    let limits = QualityMetrics::default();
    let violations = compare_metrics(&current.metrics, &limits, None, true);
    ("strict-zero".to_owned(), None, limits, violations)
  };
  let deltas = metric_deltas(&current.metrics, &limits);
  let passed = violations.is_empty();

  Ok(QualityOutcome {
    revision: current.revision,
    scope,
    mode,
    baseline_path,
    metrics: current.metrics,
    limits: Some(limits),
    deltas,
    violations,
    passed,
  })
}

fn format_scope(scope: &QualityScope) -> String {
  if let Some(namespace) = &scope.namespace {
    format!("namespace={namespace}")
  } else if let Some(prefix) = &scope.namespace_prefix {
    format!("namespace-prefix={prefix}")
  } else if scope.include_dependencies {
    "project+dependencies".to_owned()
  } else {
    "project".to_owned()
  }
}

pub fn format_quality_report(outcome: &QualityOutcome) -> String {
  let mut out = String::new();
  if outcome.mode == "write-baseline" {
    let _ = writeln!(out, "Static quality baseline written");
  } else {
    let _ = writeln!(out, "Static quality gate");
  }
  let _ = writeln!(out, "- mode: {}", outcome.mode);
  let _ = writeln!(out, "- scope: {}", format_scope(&outcome.scope));
  let _ = writeln!(out, "- revision: {}", outcome.revision);
  if let Some(path) = &outcome.baseline_path {
    let _ = writeln!(out, "- baseline: {path}");
  }
  let _ = writeln!(out, "- result: {}", if outcome.passed { "PASS" } else { "FAIL" });
  let _ = writeln!(out, "- metrics:");
  for (metric, actual) in outcome.metrics.values() {
    if let Some(limits) = &outcome.limits {
      let limit = limits.value(metric);
      let delta = actual as i64 - limit as i64;
      let _ = writeln!(out, "  - {metric}: {actual} (limit {limit}, delta {delta:+})");
    } else {
      let _ = writeln!(out, "  - {metric}: {actual}");
    }
  }
  if !outcome.violations.is_empty() {
    let _ = writeln!(out, "- regressions:");
    for violation in &outcome.violations {
      let target = violation.definition.as_deref().unwrap_or("project total");
      let _ = writeln!(
        out,
        "  - {target}: {} {} > {} (+{})",
        violation.metric, violation.actual, violation.limit, violation.delta
      );
    }
  }
  out
}

pub fn format_quality_json(outcome: &QualityOutcome) -> Result<String, String> {
  let diagnostics = if outcome.passed {
    vec![]
  } else {
    vec![serde_json::json!({
      "code": "E_STATIC_QUALITY_REGRESSION",
      "phase": "analysis",
      "severity": "error",
      "message": format!("Static quality gate found {} regression(s).", outcome.violations.len()),
      "suggestion": "Fix the reported definitions. Update a reviewed baseline only when the remaining debt is intentional and documented.",
    })]
  };
  let envelope = serde_json::json!({
    "schema_version": 2,
    "command": "analyze.quality",
    "revision": outcome.revision,
    "data": {
      "scope": outcome.scope,
      "mode": outcome.mode,
      "baseline": outcome.baseline_path,
      "passed": outcome.passed,
      "metrics": outcome.metrics,
      "limits": outcome.limits,
      "deltas": outcome.deltas,
      "violations": outcome.violations,
    },
    "diagnostics": diagnostics,
  });
  serde_json::to_string_pretty(&envelope).map_err(|error| format!("Failed to encode quality JSON: {error}"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn metrics(schema_dynamic: usize) -> QualityMetrics {
    QualityMetrics {
      schema_dynamic,
      ..QualityMetrics::default()
    }
  }

  #[test]
  fn legacy_baseline_accepts_the_business_project_shape() {
    let source = r#"{
      "typeNone": 68,
      "typeNotFull": 100,
      "schemaDynamic": 101,
      "codeDynamic": 0,
      "codeNil": 35,
      "unresolved": 136,
      "declaredOptional": 0,
      "deprecatedCalls": 0
    }"#;
    let parsed: QualityMetrics = serde_json::from_str(source).expect("legacy baseline should parse");
    assert_eq!(parsed.type_none, 68);
    assert_eq!(parsed.schema_dynamic, 101);
    assert_eq!(parsed.unresolved, 136);
    assert_eq!(parsed.unsafe_coerce, 0);
  }

  #[test]
  fn v1_native_baseline_migrates_with_zero_unsafe_budget() {
    let source = r#"{
      "schemaVersion": 1,
      "scope": {"namespace": null, "namespacePrefix": null, "includeDependencies": false},
      "metrics": {"typeNone": 0, "typeNotFull": 0, "schemaDynamic": 0, "codeDynamic": 0, "codeNil": 0, "unresolved": 0, "declaredOptional": 0, "deprecatedCalls": 0},
      "definitions": {}
    }"#;
    let path = std::env::temp_dir().join(format!("calcit-quality-v1-{}.json", std::process::id()));
    fs::write(&path, source).expect("write v1 baseline");
    let baseline = read_baseline(&path).expect("v1 baseline should parse").expect("native baseline");
    fs::remove_file(&path).expect("remove v1 baseline");
    assert_eq!(baseline.schema_version, 1);
    assert_eq!(baseline.metrics.unsafe_coerce, 0);
  }

  #[test]
  fn v1_baseline_does_not_enforce_the_new_unsafe_metric() {
    let baseline = QualityBaseline {
      schema_version: 1,
      scope: QualityScope {
        namespace: None,
        namespace_prefix: None,
        include_dependencies: false,
      },
      metrics: QualityMetrics::default(),
      definitions: BTreeMap::from([("app/adapter".to_owned(), QualityMetrics::default())]),
    };
    let current = QualitySnapshot {
      revision: "md5:test".to_owned(),
      metrics: QualityMetrics {
        unsafe_coerce: 1,
        ..QualityMetrics::default()
      },
      definitions: BTreeMap::from([(
        "app/adapter".to_owned(),
        QualityMetrics {
          unsafe_coerce: 1,
          ..QualityMetrics::default()
        },
      )]),
    };
    assert!(compare_detailed_baseline(&current, &baseline).is_empty());
    assert_eq!(reported_baseline_limits(&current, &baseline).unsafe_coerce, 1);
  }

  #[test]
  fn detailed_baseline_catches_debt_moved_between_definitions() {
    let baseline = QualityBaseline {
      schema_version: QUALITY_BASELINE_SCHEMA_VERSION,
      scope: QualityScope {
        namespace: None,
        namespace_prefix: None,
        include_dependencies: false,
      },
      metrics: metrics(1),
      definitions: BTreeMap::from([("app/a".to_owned(), metrics(1)), ("app/b".to_owned(), metrics(0))]),
    };
    let current = QualitySnapshot {
      revision: "md5:test".to_owned(),
      metrics: metrics(1),
      definitions: BTreeMap::from([("app/a".to_owned(), metrics(0)), ("app/b".to_owned(), metrics(1))]),
    };

    let violations = compare_detailed_baseline(&current, &baseline);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].definition.as_deref(), Some("app/b"));
    assert_eq!(violations[0].metric, "schemaDynamic");
  }

  #[test]
  fn metric_comparison_reports_only_regressions() {
    let actual = QualityMetrics {
      type_none: 1,
      code_nil: 2,
      ..QualityMetrics::default()
    };
    let limit = QualityMetrics {
      type_none: 1,
      code_nil: 1,
      schema_dynamic: 3,
      ..QualityMetrics::default()
    };

    let violations = compare_metrics(&actual, &limit, None, true);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].metric, "codeNil");
    assert_eq!(violations[0].delta, 1);
  }
}

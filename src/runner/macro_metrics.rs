use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::calcit::MacroSignature;

static ENABLED: AtomicBool = AtomicBool::new(false);
static METRICS: LazyLock<Mutex<BTreeMap<String, MacroExpansionMetric>>> = LazyLock::new(|| Mutex::new(BTreeMap::new()));

thread_local! {
  static ACTIVE_PHASES: RefCell<Vec<ActivePhase>> = const { RefCell::new(vec![]) };
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroExpansionMetric {
  pub expansions: u64,
  pub evaluator_nanos: u64,
  pub post_preprocess_nanos: u64,
  pub general_evaluator_fallbacks: u64,
  pub cache_hits: u64,
  pub cache_misses: u64,
  pub cache_miss_reasons: BTreeMap<String, u64>,
  pub cache_bypasses: BTreeMap<String, u64>,
  pub cache_invalidations: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct MacroMetricTotals {
  expansions: u64,
  evaluator_nanos: u64,
  post_preprocess_nanos: u64,
  general_evaluator_fallbacks: u64,
  cache_hits: u64,
  cache_misses: u64,
  cache_miss_reasons: BTreeMap<String, u64>,
  cache_bypasses: BTreeMap<String, u64>,
  cache_invalidations: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MacroMetricsReport {
  schema_version: u8,
  unit: &'static str,
  totals: MacroMetricTotals,
  macros: BTreeMap<String, MacroExpansionMetric>,
}

fn add_reason(target: &mut BTreeMap<String, u64>, reason: &str, count: u64) {
  *target.entry(reason.to_owned()).or_default() += count;
}

fn remove_reason(target: &mut BTreeMap<String, u64>, reason: &str) -> bool {
  let Some(count) = target.get_mut(reason) else {
    return false;
  };
  *count = count.saturating_sub(1);
  if *count == 0 {
    target.remove(reason);
  }
  true
}

fn totals(metrics: &BTreeMap<String, MacroExpansionMetric>) -> MacroMetricTotals {
  let mut totals = MacroMetricTotals::default();
  for metric in metrics.values() {
    totals.expansions += metric.expansions;
    totals.evaluator_nanos += metric.evaluator_nanos;
    totals.post_preprocess_nanos += metric.post_preprocess_nanos;
    totals.general_evaluator_fallbacks += metric.general_evaluator_fallbacks;
    totals.cache_hits += metric.cache_hits;
    totals.cache_misses += metric.cache_misses;
    for (reason, count) in &metric.cache_miss_reasons {
      add_reason(&mut totals.cache_miss_reasons, reason, *count);
    }
    for (reason, count) in &metric.cache_bypasses {
      add_reason(&mut totals.cache_bypasses, reason, *count);
    }
    for (reason, count) in &metric.cache_invalidations {
      add_reason(&mut totals.cache_invalidations, reason, *count);
    }
  }
  totals
}

fn update(name: &str, f: impl FnOnce(&mut MacroExpansionMetric)) {
  if !ENABLED.load(Ordering::Relaxed) {
    return;
  }
  let mut metrics = METRICS.lock().expect("lock macro expansion metrics");
  f(metrics.entry(name.to_owned()).or_default());
}

/// Record one macro call before evaluator execution. The cache counters are
/// intentionally explicit even before a cache exists: strict pure candidates
/// count as misses (`cache-not-implemented`), while effectful and legacy calls
/// are bypassed with a stable reason.
pub fn record_expansion(name: &str, signature: &MacroSignature) {
  update(name, |metric| record_expansion_metric(metric, signature));
}

pub fn record_cache_hit(name: &str) {
  update(name, record_cache_hit_metric);
}

fn record_cache_hit_metric(metric: &mut MacroExpansionMetric) {
  if remove_reason(&mut metric.cache_miss_reasons, "cache-not-implemented") {
    metric.cache_misses = metric.cache_misses.saturating_sub(1);
  }
  metric.general_evaluator_fallbacks = metric.general_evaluator_fallbacks.saturating_sub(1);
  metric.cache_hits += 1;
}

pub fn record_cache_miss(name: &str, reason: &'static str, invalidated: bool) {
  update(name, |metric| record_cache_miss_metric(metric, reason, invalidated));
}

fn record_cache_miss_metric(metric: &mut MacroExpansionMetric, reason: &'static str, invalidated: bool) {
  if remove_reason(&mut metric.cache_miss_reasons, "cache-not-implemented") {
    add_reason(&mut metric.cache_miss_reasons, reason, 1);
  }
  if invalidated {
    add_reason(&mut metric.cache_invalidations, reason, 1);
  }
}

pub fn record_cache_bypass(name: &str, reason: &'static str) {
  update(name, |metric| record_cache_bypass_metric(metric, reason));
}

/// Reclassify a recorded macro expansion that used a compiler-native lowering
/// instead of entering the general macro evaluator.
pub fn record_native_fast_path(name: &str) {
  update(name, record_native_fast_path_metric);
}

fn record_native_fast_path_metric(metric: &mut MacroExpansionMetric) {
  metric.general_evaluator_fallbacks = metric.general_evaluator_fallbacks.saturating_sub(1);
  if remove_reason(&mut metric.cache_miss_reasons, "cache-not-implemented") {
    metric.cache_misses = metric.cache_misses.saturating_sub(1);
    add_reason(&mut metric.cache_bypasses, "native-fast-path", 1);
  }
}

fn record_cache_bypass_metric(metric: &mut MacroExpansionMetric, reason: &'static str) {
  if remove_reason(&mut metric.cache_miss_reasons, "cache-not-implemented") {
    metric.cache_misses = metric.cache_misses.saturating_sub(1);
    add_reason(&mut metric.cache_bypasses, reason, 1);
  }
}

fn record_expansion_metric(metric: &mut MacroExpansionMetric, signature: &MacroSignature) {
  metric.expansions += 1;
  metric.general_evaluator_fallbacks += 1;
  if !signature.capabilities.is_empty() {
    add_reason(&mut metric.cache_bypasses, "declared-capabilities", 1);
  } else {
    metric.cache_misses += 1;
    add_reason(&mut metric.cache_miss_reasons, "cache-not-implemented", 1);
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroMetricPhase {
  Evaluator,
  PostPreprocess,
}

struct ActivePhase {
  name: String,
  phase: MacroMetricPhase,
  resumed_at: Instant,
}

pub struct PhaseTimer {
  name: String,
  phase: MacroMetricPhase,
}

impl PhaseTimer {
  pub fn start(name: &str, phase: MacroMetricPhase) -> Option<Self> {
    if !ENABLED.load(Ordering::Relaxed) {
      return None;
    }
    let now = Instant::now();
    let paused_parent = ACTIVE_PHASES.with(|phases| {
      let mut phases = phases.borrow_mut();
      let paused = phases
        .last()
        .map(|parent| (parent.name.clone(), parent.phase, now.duration_since(parent.resumed_at)));
      phases.push(ActivePhase {
        name: name.to_owned(),
        phase,
        resumed_at: now,
      });
      paused
    });
    if let Some((parent_name, parent_phase, elapsed)) = paused_parent {
      record_phase(&parent_name, parent_phase, elapsed);
    }
    Some(Self {
      name: name.to_owned(),
      phase,
    })
  }
}

fn duration_nanos(duration: Duration) -> u64 {
  duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn record_phase(name: &str, phase: MacroMetricPhase, elapsed: Duration) {
  let elapsed = duration_nanos(elapsed);
  update(name, |metric| match phase {
    MacroMetricPhase::Evaluator => metric.evaluator_nanos += elapsed,
    MacroMetricPhase::PostPreprocess => metric.post_preprocess_nanos += elapsed,
  });
}

impl Drop for PhaseTimer {
  fn drop(&mut self) {
    let now = Instant::now();
    let elapsed = ACTIVE_PHASES.with(|phases| {
      let mut phases = phases.borrow_mut();
      let active = phases.pop().expect("macro metric phase stack must be balanced");
      debug_assert_eq!(active.name, self.name);
      debug_assert_eq!(active.phase, self.phase);
      if let Some(parent) = phases.last_mut() {
        parent.resumed_at = now;
      }
      now.duration_since(active.resumed_at)
    });
    record_phase(&self.name, self.phase, elapsed);
  }
}

pub fn reset(enabled: bool) {
  ENABLED.store(enabled, Ordering::SeqCst);
  METRICS.lock().expect("reset macro expansion metrics").clear();
  ACTIVE_PHASES.with(|phases| phases.borrow_mut().clear());
}

pub fn report_json() -> Result<String, String> {
  let metrics = METRICS.lock().expect("read macro expansion metrics").clone();
  report_json_for(metrics)
}

fn report_json_for(metrics: BTreeMap<String, MacroExpansionMetric>) -> Result<String, String> {
  serde_json::to_string(&MacroMetricsReport {
    schema_version: 1,
    unit: "nanoseconds",
    totals: totals(&metrics),
    macros: metrics,
  })
  .map_err(|error| format!("failed to serialize macro expansion metrics: {error}"))
}

pub struct ReportOnDrop {
  enabled: bool,
}

impl ReportOnDrop {
  pub fn new(enabled: bool) -> Self {
    reset(enabled);
    Self { enabled }
  }
}

impl Drop for ReportOnDrop {
  fn drop(&mut self) {
    if self.enabled {
      match report_json() {
        Ok(report) => eprintln!("macro-expansion-metrics: {report}"),
        Err(error) => eprintln!("[Warn] {error}"),
      }
      ENABLED.store(false, Ordering::SeqCst);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::{MacroCapability, MacroExpansionType};
  use std::collections::HashSet;
  use std::sync::Arc;

  fn pure_signature() -> MacroSignature {
    MacroSignature {
      generics: Arc::new(vec![]),
      where_bounds: Arc::new(vec![]),
      required_inputs: Arc::new(vec![]),
      optional_inputs: Arc::new(vec![]),
      rest_input: None,
      expansion: MacroExpansionType::Dynamic,
      capabilities: Arc::new(HashSet::new()),
      features: Arc::new(HashSet::new()),
    }
  }

  #[test]
  fn report_separates_counts_timings_and_cache_reasons() {
    let mut metrics = BTreeMap::new();
    let pure = metrics.entry("tests/pure".to_owned()).or_default();
    record_expansion_metric(pure, &pure_signature());
    pure.evaluator_nanos = 1;
    pure.post_preprocess_nanos = 2;
    let mut effectful = pure_signature();
    effectful.capabilities = Arc::new(HashSet::from([MacroCapability::EnvRead]));
    record_expansion_metric(metrics.entry("tests/effectful".to_owned()).or_default(), &effectful);
    let report: serde_json::Value = serde_json::from_str(&report_json_for(metrics).expect("metrics JSON")).expect("valid JSON");
    assert_eq!(report["totals"]["expansions"], 2);
    assert_eq!(report["totals"]["generalEvaluatorFallbacks"], 2);
    assert_eq!(report["totals"]["cacheMisses"], 1);
    assert_eq!(report["totals"]["cacheMissReasons"]["cache-not-implemented"], 1);
    assert_eq!(report["totals"]["cacheBypasses"]["declared-capabilities"], 1);
    assert_eq!(report["totals"]["cacheInvalidations"], serde_json::json!({}));
    assert_eq!(report["macros"]["tests/pure"]["evaluatorNanos"], 1);
    assert_eq!(report["macros"]["tests/pure"]["postPreprocessNanos"], 2);
  }

  #[test]
  fn cache_outcomes_replace_the_placeholder_candidate_miss() {
    let mut hit = MacroExpansionMetric::default();
    record_expansion_metric(&mut hit, &pure_signature());
    record_cache_hit_metric(&mut hit);
    assert_eq!(hit.cache_hits, 1);
    assert_eq!(hit.cache_misses, 0);
    assert_eq!(hit.general_evaluator_fallbacks, 0);

    let mut miss = MacroExpansionMetric::default();
    record_expansion_metric(&mut miss, &pure_signature());
    record_cache_miss_metric(&mut miss, "input-syntax", true);
    assert_eq!(miss.cache_misses, 1);
    assert_eq!(miss.cache_miss_reasons.get("input-syntax"), Some(&1));
    assert_eq!(miss.cache_invalidations.get("input-syntax"), Some(&1));

    let mut bypass = MacroExpansionMetric::default();
    record_expansion_metric(&mut bypass, &pure_signature());
    record_cache_bypass_metric(&mut bypass, "cache-disabled");
    assert_eq!(bypass.cache_misses, 0);
    assert_eq!(bypass.cache_bypasses.get("cache-disabled"), Some(&1));
  }

  #[test]
  fn native_fast_path_is_not_reported_as_a_general_evaluator_fallback() {
    let mut metric = MacroExpansionMetric::default();
    record_expansion_metric(&mut metric, &pure_signature());
    record_native_fast_path_metric(&mut metric);
    assert_eq!(metric.general_evaluator_fallbacks, 0);
    assert_eq!(metric.cache_misses, 0);
    assert_eq!(metric.cache_bypasses.get("native-fast-path"), Some(&1));
  }
}

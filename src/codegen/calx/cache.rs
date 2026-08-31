//! Embedding-owned, revision-safe cache for immutable Calx artifacts.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::program::CompiledProgram;

use super::lowering::{
  CalxCompiledArtifact, CalxKernelCompileError, CalxKernelCompileTimings, CalxPreparedKernel,
  compile_calx_artifact_with_imports_measured, prepare_calx_artifact,
};
use super::{CALX_KERNEL_ABI_EDITION, CalxDefinitionRef, CalxHostImports, CalxImportContract, import_contract, lookup_compiled_def};

/// Stable reason assigned to one source-derived cache miss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalxCacheMissReason {
  Empty,
  EntryChanged,
  CalleeChanged,
  SchemaChanged,
  AbiChanged,
  ImportContractChanged,
  DependencyMissing,
  Evicted,
}

impl CalxCacheMissReason {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Empty => "empty",
      Self::EntryChanged => "entry-changed",
      Self::CalleeChanged => "callee-changed",
      Self::SchemaChanged => "schema-changed",
      Self::AbiChanged => "abi-changed",
      Self::ImportContractChanged => "import-contract-changed",
      Self::DependencyMissing => "dependency-missing",
      Self::Evicted => "evicted",
    }
  }
}

/// Aggregate counters and current bounded-cache gauges.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CalxCompileCacheStats {
  pub hits: u64,
  pub misses: u64,
  pub misses_by_reason: BTreeMap<CalxCacheMissReason, u64>,
  pub evictions: u64,
  pub clears: u64,
  pub entry_count: usize,
  pub recently_evicted_count: usize,
  pub reachable_function_count: usize,
  pub syntax_instruction_count: usize,
  pub lowered_instruction_count: usize,
  pub estimated_bytes: usize,
}

impl CalxCompileCacheStats {
  pub fn miss_count(&self, reason: CalxCacheMissReason) -> u64 {
    self.misses_by_reason.get(&reason).copied().unwrap_or(0)
  }
}

/// Per-request evidence showing which stages were skipped or executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalxCachePrepareReport {
  pub cache_hit: bool,
  pub miss_reason: Option<CalxCacheMissReason>,
  pub skipped_eligibility: bool,
  pub skipped_planning: bool,
  pub skipped_program_construction: bool,
  pub skipped_validation_lowering: bool,
  pub revision_validation: Duration,
  pub binding_attachment: Duration,
  pub compilation: Option<CalxKernelCompileTimings>,
}

/// A freshly prepared kernel and the cache decision that produced it.
#[derive(Debug)]
pub struct CalxCachePreparation {
  kernel: CalxPreparedKernel,
  report: CalxCachePrepareReport,
}

impl CalxCachePreparation {
  pub fn kernel(&self) -> &CalxPreparedKernel {
    &self.kernel
  }

  pub fn report(&self) -> &CalxCachePrepareReport {
    &self.report
  }

  pub fn into_kernel(self) -> CalxPreparedKernel {
    self.kernel
  }

  pub fn into_parts(self) -> (CalxPreparedKernel, CalxCachePrepareReport) {
    (self.kernel, self.report)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CalxCacheSlotKey {
  entry: CalxDefinitionRef,
  abi_edition: Arc<str>,
  imports: Vec<CalxImportContract>,
}

impl CalxCacheSlotKey {
  fn current(entry: CalxDefinitionRef, imports: &CalxHostImports) -> Self {
    Self {
      entry,
      abi_edition: Arc::from(CALX_KERNEL_ABI_EDITION),
      imports: import_contract(imports),
    }
  }
}

#[derive(Debug, Clone)]
struct CalxCacheEntry {
  artifact: Rc<CalxCompiledArtifact>,
  last_used: u64,
}

#[derive(Debug, Default)]
struct CalxCacheCounters {
  hits: u64,
  misses: u64,
  misses_by_reason: BTreeMap<CalxCacheMissReason, u64>,
  evictions: u64,
  clears: u64,
}

/// Explicit, capacity-bounded LRU cache owned by one embedding.
///
/// The cache stores only immutable source-derived artifacts. Host bindings are
/// checked and reattached on every prepare, including cache hits.
#[derive(Debug)]
pub struct CalxCompileCache {
  capacity: usize,
  clock: u64,
  entries: BTreeMap<CalxCacheSlotKey, CalxCacheEntry>,
  recently_evicted: BTreeMap<CalxCacheSlotKey, u64>,
  counters: CalxCacheCounters,
}

impl CalxCompileCache {
  /// Create a bounded cache. Capacity zero is a supported always-miss mode.
  pub fn new(capacity: usize) -> Self {
    Self {
      capacity,
      clock: 0,
      entries: BTreeMap::new(),
      recently_evicted: BTreeMap::new(),
      counters: CalxCacheCounters::default(),
    }
  }

  pub fn capacity(&self) -> usize {
    self.capacity
  }

  /// Validate dependencies, compile on miss, and attach this request's hosts.
  pub fn prepare(
    &mut self,
    program: &CompiledProgram,
    namespace: impl Into<Arc<str>>,
    definition: impl Into<Arc<str>>,
    imports: &CalxHostImports,
  ) -> Result<CalxCachePreparation, CalxKernelCompileError> {
    let entry = CalxDefinitionRef::new(namespace, definition);
    let key = CalxCacheSlotKey::current(entry, imports);
    let revision_started = Instant::now();
    let candidate = self.entries.get(&key).map(|entry| entry.artifact.clone());
    let miss_reason = match candidate.as_ref() {
      Some(artifact) => validate_reachable_stamps(program, artifact),
      None => Some(self.classify_absent_slot(&key)),
    };
    let revision_validation = revision_started.elapsed();

    if let (Some(artifact), None) = (candidate, miss_reason) {
      let binding_started = Instant::now();
      let kernel = prepare_calx_artifact(artifact, imports)?;
      let binding_attachment = binding_started.elapsed();
      let tick = self.next_tick();
      if let Some(entry) = self.entries.get_mut(&key) {
        entry.last_used = tick;
      }
      self.counters.hits += 1;
      return Ok(CalxCachePreparation {
        kernel,
        report: CalxCachePrepareReport {
          cache_hit: true,
          miss_reason: None,
          skipped_eligibility: true,
          skipped_planning: true,
          skipped_program_construction: true,
          skipped_validation_lowering: true,
          revision_validation,
          binding_attachment,
          compilation: None,
        },
      });
    }

    let reason = miss_reason.unwrap_or(CalxCacheMissReason::Empty);
    self.record_miss(reason);
    let (artifact, compilation) =
      compile_calx_artifact_with_imports_measured(program, key.entry.namespace.clone(), key.entry.definition.clone(), imports)?;
    let binding_started = Instant::now();
    let kernel = prepare_calx_artifact(artifact.clone(), imports)?;
    let binding_attachment = binding_started.elapsed();
    self.insert(key, artifact);
    Ok(CalxCachePreparation {
      kernel,
      report: CalxCachePrepareReport {
        cache_hit: false,
        miss_reason: Some(reason),
        skipped_eligibility: false,
        skipped_planning: false,
        skipped_program_construction: false,
        skipped_validation_lowering: false,
        revision_validation,
        binding_attachment,
        compilation: Some(compilation),
      },
    })
  }

  /// Remove artifacts and eviction provenance while retaining counters.
  pub fn clear(&mut self) {
    self.entries.clear();
    self.recently_evicted.clear();
    self.counters.clears += 1;
  }

  pub fn stats(&self) -> CalxCompileCacheStats {
    let mut stats = CalxCompileCacheStats {
      hits: self.counters.hits,
      misses: self.counters.misses,
      misses_by_reason: self.counters.misses_by_reason.clone(),
      evictions: self.counters.evictions,
      clears: self.counters.clears,
      entry_count: self.entries.len(),
      recently_evicted_count: self.recently_evicted.len(),
      ..CalxCompileCacheStats::default()
    };
    for entry in self.entries.values() {
      stats.reachable_function_count += entry.artifact.reachable_definition_count();
      stats.syntax_instruction_count += entry.artifact.syntax_instruction_count();
      stats.lowered_instruction_count += entry.artifact.lowered_instruction_count();
      stats.estimated_bytes += entry.artifact.estimated_bytes();
    }
    stats
  }

  fn classify_absent_slot(&self, key: &CalxCacheSlotKey) -> CalxCacheMissReason {
    if self.recently_evicted.contains_key(key) {
      return CalxCacheMissReason::Evicted;
    }
    for active in self.entries.keys().filter(|active| active.entry == key.entry) {
      if active.abi_edition != key.abi_edition {
        return CalxCacheMissReason::AbiChanged;
      }
      if active.imports != key.imports {
        return CalxCacheMissReason::ImportContractChanged;
      }
    }
    CalxCacheMissReason::Empty
  }

  fn record_miss(&mut self, reason: CalxCacheMissReason) {
    self.counters.misses += 1;
    *self.counters.misses_by_reason.entry(reason).or_insert(0) += 1;
  }

  fn insert(&mut self, key: CalxCacheSlotKey, artifact: Rc<CalxCompiledArtifact>) {
    self.recently_evicted.remove(&key);
    if self.capacity == 0 {
      return;
    }
    if !self.entries.contains_key(&key)
      && self.entries.len() >= self.capacity
      && let Some(evicted) = least_recent_key(&self.entries)
    {
      self.entries.remove(&evicted);
      self.counters.evictions += 1;
      self.record_evicted(evicted);
    }
    let tick = self.next_tick();
    self.entries.insert(key, CalxCacheEntry { artifact, last_used: tick });
  }

  fn record_evicted(&mut self, key: CalxCacheSlotKey) {
    if self.capacity == 0 {
      return;
    }
    let tick = self.next_tick();
    self.recently_evicted.insert(key, tick);
    while self.recently_evicted.len() > self.capacity {
      let Some(oldest) = least_recent_tombstone(&self.recently_evicted) else {
        break;
      };
      self.recently_evicted.remove(&oldest);
    }
  }

  fn next_tick(&mut self) -> u64 {
    self.clock = self.clock.wrapping_add(1);
    self.clock
  }
}

fn validate_reachable_stamps(program: &CompiledProgram, artifact: &CalxCompiledArtifact) -> Option<CalxCacheMissReason> {
  for stamp in &artifact.reachable_stamps {
    let Some(current) = lookup_compiled_def(program, &stamp.definition) else {
      return Some(CalxCacheMissReason::DependencyMissing);
    };
    if current.schema != stamp.schema {
      return Some(CalxCacheMissReason::SchemaChanged);
    }
    if current.def_id != stamp.def_id || current.preprocessed_code != stamp.preprocessed_code {
      return Some(if stamp.definition == artifact.graph().entry {
        CalxCacheMissReason::EntryChanged
      } else {
        CalxCacheMissReason::CalleeChanged
      });
    }
  }
  for stamp in &artifact.import_schema_stamps {
    let Some(current) = lookup_compiled_def(program, &stamp.definition) else {
      return Some(CalxCacheMissReason::DependencyMissing);
    };
    if current.schema != stamp.schema {
      return Some(CalxCacheMissReason::SchemaChanged);
    }
  }
  None
}

fn least_recent_key(entries: &BTreeMap<CalxCacheSlotKey, CalxCacheEntry>) -> Option<CalxCacheSlotKey> {
  entries
    .iter()
    .min_by(|(left_key, left), (right_key, right)| left.last_used.cmp(&right.last_used).then_with(|| left_key.cmp(right_key)))
    .map(|(key, _)| key.clone())
}

fn least_recent_tombstone(entries: &BTreeMap<CalxCacheSlotKey, u64>) -> Option<CalxCacheSlotKey> {
  entries
    .iter()
    .min_by(|(left_key, left), (right_key, right)| left.cmp(right).then_with(|| left_key.cmp(right_key)))
    .map(|(key, _)| key.clone())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn absent_slot_classification_distinguishes_abi_and_import_contract() {
    let entry = CalxDefinitionRef::new("app.kernel", "main");
    let current = CalxCacheSlotKey {
      entry: entry.clone(),
      abi_edition: Arc::from("calcit-calx-kernel/2"),
      imports: vec![],
    };
    let mut cache = CalxCompileCache::new(2);
    let old_abi = CalxCacheSlotKey {
      entry: entry.clone(),
      abi_edition: Arc::from("calcit-calx-kernel/1"),
      imports: vec![],
    };
    cache.entries.insert(
      old_abi,
      CalxCacheEntry {
        artifact: test_artifact(),
        last_used: 1,
      },
    );
    assert_eq!(cache.classify_absent_slot(&current), CalxCacheMissReason::AbiChanged);

    cache.entries.clear();
    let different_import = CalxCacheSlotKey {
      entry,
      abi_edition: Arc::from("calcit-calx-kernel/2"),
      imports: vec![CalxImportContract {
        definition: CalxDefinitionRef::new("app.host", "read"),
        export_name: Arc::from("host.read"),
        params: vec![],
        result: None,
      }],
    };
    cache.entries.insert(
      different_import,
      CalxCacheEntry {
        artifact: test_artifact(),
        last_used: 2,
      },
    );
    assert_eq!(cache.classify_absent_slot(&current), CalxCacheMissReason::ImportContractChanged);
  }

  fn test_artifact() -> Rc<CalxCompiledArtifact> {
    use calx_vm::{CalxFunc, CalxProgram, CalxSyntax, CalxType, ValidatedProgram};

    let function = CalxFunc::new(
      "main",
      vec![],
      vec![CalxType::F64],
      vec![CalxSyntax::Const(super::super::CalxValue::F64(0.0))],
    );
    let program = CalxProgram::try_new(vec![function], vec![], vec![]).expect("test program");
    let program = ValidatedProgram::try_from_program(program).expect("validated test program");
    Rc::new(CalxCompiledArtifact {
      graph: super::super::CalxEligibleCallGraph {
        abi_edition: Arc::from(CALX_KERNEL_ABI_EDITION),
        entry: CalxDefinitionRef::new("app.kernel", "main"),
        functions: vec![],
      },
      params: vec![],
      result: Some(super::super::CalxScalarType::F64),
      program,
      reachable_stamps: vec![],
      import_schema_stamps: vec![],
      import_contract: vec![],
    })
  }
}

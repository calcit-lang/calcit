use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use crate::builtins::meta;
use crate::calcit::{Calcit, MacroSignature, NodeLocation};

const MAX_ENTRIES: usize = 8_192;

static ENABLED: AtomicBool = AtomicBool::new(false);
static CACHE: LazyLock<Mutex<HashMap<MacroCallSite, CachedExpansion>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MacroCallSite {
  macro_name: Arc<str>,
  expansion_ns: Arc<str>,
  compiling_def: Arc<str>,
  caller_ns: Arc<str>,
  caller_def: Arc<str>,
  coord: Arc<Vec<u16>>,
}

#[derive(Debug, Clone)]
struct CachedExpansion {
  macro_id: Arc<str>,
  signature_hash: u64,
  inputs: Vec<Calcit>,
  output: Calcit,
  gensym_start: usize,
  gensym_delta: usize,
}

#[derive(Debug)]
pub struct CacheMiss {
  call_site: MacroCallSite,
  macro_id: Arc<str>,
  signature_hash: u64,
  inputs: Vec<Calcit>,
  gensym_start: usize,
}

#[derive(Debug)]
pub enum CacheLookup {
  Hit(Calcit),
  Miss { token: CacheMiss, reason: &'static str },
  Bypass(&'static str),
}

fn signature_hash(signature: &MacroSignature) -> u64 {
  let mut hasher = DefaultHasher::new();
  signature.hash(&mut hasher);
  hasher.finish()
}

fn same_source_syntax(a: &Calcit, b: &Calcit) -> bool {
  if a != b || a.get_location() != b.get_location() {
    return false;
  }
  match (a, b) {
    (Calcit::List(xs), Calcit::List(ys)) => xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(x, y)| same_source_syntax(x, y)),
    (Calcit::Recur(xs), Calcit::Recur(ys)) => xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(x, y)| same_source_syntax(x, y)),
    _ => true,
  }
}

fn same_inputs(a: &[Calcit], b: &[Calcit]) -> bool {
  a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_source_syntax(x, y))
}

pub fn is_enabled() -> bool {
  ENABLED.load(Ordering::Relaxed)
}

pub fn reset(enabled: bool) {
  ENABLED.store(enabled, Ordering::SeqCst);
  CACHE.lock().expect("reset macro expansion cache").clear();
}

pub fn lookup(
  macro_name: &str,
  macro_id: &Arc<str>,
  signature: &MacroSignature,
  inputs: &[Calcit],
  call_location: Option<&NodeLocation>,
  file_ns: &str,
) -> CacheLookup {
  if !is_enabled() {
    return CacheLookup::Bypass("cache-disabled");
  }
  if !signature.capabilities.is_empty() {
    return CacheLookup::Bypass("declared-capabilities");
  }
  let Some(location) = call_location else {
    return CacheLookup::Bypass("unstable-call-site");
  };

  let call_site = MacroCallSite {
    macro_name: Arc::from(macro_name),
    expansion_ns: Arc::from(file_ns),
    compiling_def: meta::current_compiling_key(file_ns),
    caller_ns: location.ns.clone(),
    caller_def: location.def.clone(),
    coord: location.coord.clone(),
  };
  let signature_hash = signature_hash(signature);
  let gensym_start = meta::current_gensym_index(file_ns);
  let mut cache = CACHE.lock().expect("read macro expansion cache");

  if let Some(entry) = cache.get(&call_site) {
    let invalidation = if entry.macro_id != *macro_id {
      Some("macro-definition")
    } else if entry.signature_hash != signature_hash {
      Some("macro-signature")
    } else if !same_inputs(&entry.inputs, inputs) {
      Some("input-syntax")
    } else if entry.gensym_delta > 0 && entry.gensym_start != gensym_start {
      Some("gensym-sequence")
    } else {
      None
    };

    if let Some(reason) = invalidation {
      cache.remove(&call_site);
      return CacheLookup::Miss {
        token: CacheMiss {
          call_site,
          macro_id: macro_id.clone(),
          signature_hash,
          inputs: inputs.to_vec(),
          gensym_start,
        },
        reason,
      };
    }

    let output = entry.output.clone();
    let gensym_delta = entry.gensym_delta;
    drop(cache);
    if gensym_delta > 0 {
      meta::advance_gensym_index(file_ns, gensym_delta);
    }
    return CacheLookup::Hit(output);
  }

  CacheLookup::Miss {
    token: CacheMiss {
      call_site,
      macro_id: macro_id.clone(),
      signature_hash,
      inputs: inputs.to_vec(),
      gensym_start,
    },
    reason: "cold-call-site",
  }
}

/// Store an expansion with the gensym position captured immediately after the
/// macro evaluator returned, before recursively preprocessing its output.
pub fn store(token: CacheMiss, output: &Calcit, evaluator_gensym_end: usize) {
  if !is_enabled() {
    return;
  }
  let entry = CachedExpansion {
    macro_id: token.macro_id,
    signature_hash: token.signature_hash,
    inputs: token.inputs,
    output: output.clone(),
    gensym_start: token.gensym_start,
    gensym_delta: evaluator_gensym_end.saturating_sub(token.gensym_start),
  };
  let mut cache = CACHE.lock().expect("write macro expansion cache");
  if cache.len() >= MAX_ENTRIES && !cache.contains_key(&token.call_site) {
    cache.clear();
  }
  cache.insert(token.call_site, entry);
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::calcit::MacroExpansionType;
  use std::collections::HashSet;

  static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

  fn location(coord: &[u16]) -> NodeLocation {
    NodeLocation::new(Arc::from("app.main"), Arc::from("main!"), Arc::new(coord.to_vec()))
  }

  #[test]
  fn reuses_only_same_pure_call_site_and_inputs() {
    let _guard = TEST_LOCK.lock().expect("serialize macro cache tests");
    reset(true);
    let signature = pure_signature();
    let macro_id = Arc::from("macro-1");
    let inputs = vec![Calcit::Number(1.0)];
    let CacheLookup::Miss { token, reason } = lookup(
      "calcit.core/id",
      &macro_id,
      &signature,
      &inputs,
      Some(&location(&[1, 2])),
      "app.main",
    ) else {
      panic!("first lookup should miss");
    };
    assert_eq!(reason, "cold-call-site");
    store(token, &Calcit::Number(2.0), meta::current_gensym_index("app.main"));

    assert!(matches!(
      lookup(
        "calcit.core/id",
        &macro_id,
        &signature,
        &inputs,
        Some(&location(&[1, 2])),
        "app.main"
      ),
      CacheLookup::Hit(Calcit::Number(2.0))
    ));

    let CacheLookup::Miss { reason, .. } = lookup(
      "calcit.core/id",
      &macro_id,
      &signature,
      &[Calcit::Number(3.0)],
      Some(&location(&[1, 2])),
      "app.main",
    ) else {
      panic!("changed syntax should miss");
    };
    assert_eq!(reason, "input-syntax");
    reset(false);
  }

  #[test]
  fn invalidates_when_macro_identity_changes() {
    let _guard = TEST_LOCK.lock().expect("serialize macro cache tests");
    reset(true);
    let signature = pure_signature();
    let old_id = Arc::from("macro-1");
    let call_location = location(&[3]);
    let CacheLookup::Miss { token, .. } = lookup("calcit.core/id", &old_id, &signature, &[], Some(&call_location), "app.main") else {
      panic!("first lookup should miss");
    };
    store(token, &Calcit::Unit, meta::current_gensym_index("app.main"));

    let CacheLookup::Miss { reason, .. } = lookup(
      "calcit.core/id",
      &Arc::from("macro-2"),
      &signature,
      &[],
      Some(&call_location),
      "app.main",
    ) else {
      panic!("changed macro should miss");
    };
    assert_eq!(reason, "macro-definition");
    reset(false);
  }

  #[test]
  fn replays_gensym_progress_only_from_the_same_sequence_position() {
    let _guard = TEST_LOCK.lock().expect("serialize macro cache tests");
    reset(true);
    let signature = pure_signature();
    let macro_id = Arc::from("macro-gensym");
    let call_location = location(&[5]);

    meta::with_compiling_def("app.main", "main!", || {
      let CacheLookup::Miss { token, .. } = lookup(
        "calcit.core/with-gensym",
        &macro_id,
        &signature,
        &[],
        Some(&call_location),
        "app.main",
      ) else {
        panic!("first lookup should miss");
      };
      assert_eq!(meta::current_gensym_index("app.main"), 1);
      meta::advance_gensym_index("app.main", 2);
      store(token, &Calcit::new_str("generated"), meta::current_gensym_index("app.main"));
      Ok::<(), ()>(())
    })
    .expect("store generated expansion");

    meta::with_compiling_def("app.main", "main!", || {
      assert!(matches!(
        lookup(
          "calcit.core/with-gensym",
          &macro_id,
          &signature,
          &[],
          Some(&call_location),
          "app.main"
        ),
        CacheLookup::Hit(_)
      ));
      assert_eq!(meta::current_gensym_index("app.main"), 3);
      Ok::<(), ()>(())
    })
    .expect("replay generated expansion");

    meta::with_compiling_def("app.main", "main!", || {
      meta::advance_gensym_index("app.main", 1);
      let CacheLookup::Miss { reason, .. } = lookup(
        "calcit.core/with-gensym",
        &macro_id,
        &signature,
        &[],
        Some(&call_location),
        "app.main",
      ) else {
        panic!("shifted gensym sequence should miss");
      };
      assert_eq!(reason, "gensym-sequence");
      Ok::<(), ()>(())
    })
    .expect("reject shifted sequence");
    reset(false);
  }

  #[test]
  fn outer_cache_replays_only_evaluator_gensyms_before_nested_macro_expansion() {
    let _guard = TEST_LOCK.lock().expect("serialize macro cache tests");
    reset(true);
    let signature = pure_signature();
    let macro_id = Arc::from("outer-macro");
    let call_location = location(&[8]);

    let miss_following_gensym = meta::with_compiling_def("app.main", "main!", || {
      let CacheLookup::Miss { token, .. } =
        lookup("app.main/outer-macro", &macro_id, &signature, &[], Some(&call_location), "app.main")
      else {
        panic!("outer macro should initially miss");
      };

      // The outer evaluator emits one gensym, then returns code invoking an
      // inner macro. The inner macro emits two more while that code is
      // recursively preprocessed.
      meta::advance_gensym_index("app.main", 1);
      let evaluator_gensym_end = meta::current_gensym_index("app.main");
      meta::advance_gensym_index("app.main", 2);
      store(token, &Calcit::new_str("inner-macro-call"), evaluator_gensym_end);
      Ok::<usize, ()>(meta::current_gensym_index("app.main"))
    })
    .expect("store outer expansion");

    let hit_following_gensym = meta::with_compiling_def("app.main", "main!", || {
      assert!(matches!(
        lookup("app.main/outer-macro", &macro_id, &signature, &[], Some(&call_location), "app.main"),
        CacheLookup::Hit(_)
      ));
      // Recursive preprocessing still expands the inner macro on a cache hit.
      meta::advance_gensym_index("app.main", 2);
      Ok::<usize, ()>(meta::current_gensym_index("app.main"))
    })
    .expect("replay outer expansion");

    assert_eq!(
      hit_following_gensym, miss_following_gensym,
      "a cached outer expansion must preserve the gensym position after its nested macro"
    );
    reset(false);
  }
}

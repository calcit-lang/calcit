//! Definition-level revisions and three-way merges for one Snapshot.
//!
//! A Snapshot is split into independent units: each definition, each
//! namespace declaration, each entry, and the project fields. Transactions can
//! guard only the units they touch, and the Git merge driver merges per unit,
//! so edits to different definitions no longer invalidate or conflict with
//! each other. The stored Snapshot format is unchanged.

use std::collections::{BTreeMap, BTreeSet};

use calcit::snapshot::{self, Snapshot};
use md5::{Digest, Md5};
use serde::Serialize;

const SCOPE_PREFIX: &str = "scope:";
const ABSENT: &str = "absent";

/// Fingerprint of every unit in a Snapshot, keyed by a stable unit name such as
/// `def:app.main/main!`, `ns:app.main`, `entry:default` or `project`.
pub(crate) fn snapshot_units(snapshot: &Snapshot) -> Result<BTreeMap<String, String>, String> {
  let mut units = BTreeMap::new();
  units.insert("project".to_owned(), project_fingerprint(snapshot)?);
  for (name, entry) in &snapshot.entries {
    units.insert(format!("entry:{name}"), json_fingerprint(entry)?);
  }
  for (namespace, file) in &snapshot.files {
    // The loader synthesizes `<package>.$meta` from the file path; it is not stored source.
    if namespace.ends_with(".$meta") {
      continue;
    }
    units.insert(format!("ns:{namespace}"), json_fingerprint(&file.ns)?);
    for (definition, entry) in &file.defs {
      units.insert(format!("def:{namespace}/{definition}"), snapshot::definition_revision(entry)?);
    }
  }
  Ok(units)
}

fn json_fingerprint<T: Serialize>(value: &T) -> Result<String, String> {
  let encoded = serde_json::to_string(value).map_err(|error| format!("Failed to fingerprint Snapshot unit: {error}"))?;
  let mut hasher = Md5::new();
  hasher.update(encoded.as_bytes());
  Ok(format!("md5:{}", hex::encode(hasher.finalize())))
}

fn project_fingerprint(snapshot: &Snapshot) -> Result<String, String> {
  json_fingerprint(&(&snapshot.package, &snapshot.about, &snapshot.version, &snapshot.verification))
}

/// Units whose fingerprint differs between two Snapshots, including added and removed units.
pub(crate) fn changed_units(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Vec<String> {
  before
    .keys()
    .chain(after.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|unit| before.get(*unit) != after.get(*unit))
    .cloned()
    .collect()
}

/// Encode the pre-edit fingerprints of the touched units as a scoped revision:
/// `scope:def:app.main/f@md5:…,ns:app.main@md5:…`; a unit created by the edit is `@absent`.
pub(crate) fn scoped_revision(original: &BTreeMap<String, String>, touched: &[String]) -> String {
  let parts = touched
    .iter()
    .map(|unit| format!("{unit}@{}", original.get(unit).map(String::as_str).unwrap_or(ABSENT)))
    .collect::<Vec<_>>();
  format!("{SCOPE_PREFIX}{}", parts.join(","))
}

pub(crate) fn is_scoped_revision(revision: &str) -> bool {
  revision.starts_with(SCOPE_PREFIX)
}

fn parse_scoped_revision(revision: &str) -> Result<BTreeMap<String, Option<String>>, String> {
  let body = revision
    .strip_prefix(SCOPE_PREFIX)
    .ok_or_else(|| format!("Scoped revision must start with `{SCOPE_PREFIX}`"))?;
  let mut units = BTreeMap::new();
  for part in body.split(',').filter(|part| !part.is_empty()) {
    let (unit, fingerprint) = part
      .rsplit_once('@')
      .ok_or_else(|| format!("Scoped revision part `{part}` must be `<unit>@<fingerprint>`"))?;
    units.insert(unit.to_owned(), (fingerprint != ABSENT).then(|| fingerprint.to_owned()));
  }
  Ok(units)
}

/// Check a scoped revision against the current Snapshot units and return the guarded unit names.
pub(crate) fn check_scoped_revision(revision: &str, current: &BTreeMap<String, String>) -> Result<BTreeSet<String>, String> {
  let expected = parse_scoped_revision(revision)?;
  let conflicts = expected
    .iter()
    .filter(|(unit, fingerprint)| current.get(*unit) != fingerprint.as_ref())
    .map(|(unit, _)| unit.as_str())
    .collect::<Vec<_>>();
  if !conflicts.is_empty() {
    return Err(format!(
      "Definition conflict: {} changed since the transaction was previewed. Re-run the dry-run and review the new plan.",
      conflicts.join(", ")
    ));
  }
  Ok(expected.into_keys().collect())
}

/// Outcome of merging one unit: the chosen side, or a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
  Ours,
  Theirs,
  Conflict,
}

fn pick<T: PartialEq>(base: Option<&T>, ours: Option<&T>, theirs: Option<&T>) -> Pick {
  if ours == theirs || base == theirs {
    Pick::Ours
  } else if base == ours {
    Pick::Theirs
  } else {
    Pick::Conflict
  }
}

/// Result of a three-way Snapshot merge.
pub(crate) struct SnapshotMerge {
  pub merged: Snapshot,
  /// Units changed differently on both sides; `merged` keeps our version of each.
  pub conflicts: Vec<String>,
}

/// Merge `theirs` into `ours` relative to `base`, one unit at a time.
pub(crate) fn merge_snapshots(base: &Snapshot, ours: &Snapshot, theirs: &Snapshot) -> Result<SnapshotMerge, String> {
  let base_units = snapshot_units(base)?;
  let our_units = snapshot_units(ours)?;
  let their_units = snapshot_units(theirs)?;
  let mut merged = ours.clone();
  let mut conflicts = Vec::new();

  let all_units = base_units
    .keys()
    .chain(our_units.keys())
    .chain(their_units.keys())
    .cloned()
    .collect::<BTreeSet<_>>();
  for unit in all_units {
    match pick(base_units.get(&unit), our_units.get(&unit), their_units.get(&unit)) {
      Pick::Ours => {}
      Pick::Conflict => conflicts.push(unit),
      Pick::Theirs => {
        if !take_theirs(&mut merged, theirs, &unit)? {
          conflicts.push(unit);
        }
      }
    }
  }
  // Units sort `def:` before `ns:`, so a namespace removed by them is dropped
  // only after its definitions are merged.
  Ok(SnapshotMerge { merged, conflicts })
}

/// Apply their unit, returning false when surviving definitions prevent namespace deletion.
fn take_theirs(merged: &mut Snapshot, theirs: &Snapshot, unit: &str) -> Result<bool, String> {
  if unit == "project" {
    merged.package = theirs.package.clone();
    merged.about = theirs.about.clone();
    merged.version = theirs.version.clone();
    merged.verification = theirs.verification.clone();
  } else if let Some(name) = unit.strip_prefix("entry:") {
    match theirs.entries.get(name) {
      Some(entry) => {
        merged.entries.insert(name.to_owned(), entry.clone());
      }
      None => {
        merged.entries.remove(name);
      }
    }
  } else if let Some(namespace) = unit.strip_prefix("ns:") {
    match theirs.files.get(namespace) {
      Some(file) => {
        merged
          .files
          .entry(namespace.to_owned())
          .or_insert_with(|| snapshot::FileInSnapShot {
            ns: file.ns.clone(),
            defs: Default::default(),
          })
          .ns = file.ns.clone();
      }
      None => {
        if merged.files.get(namespace).is_some_and(|file| !file.defs.is_empty()) {
          return Ok(false);
        }
        merged.files.remove(namespace);
      }
    }
  } else if let Some(path) = unit.strip_prefix("def:") {
    let (namespace, definition) = path.split_once('/').ok_or_else(|| format!("Malformed definition unit `{unit}`"))?;
    match theirs.files.get(namespace).and_then(|file| file.defs.get(definition)) {
      Some(entry) => {
        let ns_entry = theirs.files[namespace].ns.clone();
        merged
          .files
          .entry(namespace.to_owned())
          .or_insert_with(|| snapshot::FileInSnapShot {
            ns: ns_entry,
            defs: Default::default(),
          })
          .defs
          .insert(definition.to_owned(), entry.clone());
      }
      None => {
        if let Some(file) = merged.files.get_mut(namespace) {
          file.defs.remove(definition);
        }
      }
    }
  } else {
    return Err(format!("Unknown Snapshot unit `{unit}`"));
  }
  Ok(true)
}

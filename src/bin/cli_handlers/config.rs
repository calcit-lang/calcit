//! Config command handler — top-level `calcit config` shortcut
//!
//! Consolidates project/entry display and safe mutations for modules, versions, and type slots.

use calcit::cli_args::{
  ConfigAddModuleCommand, ConfigCommand, ConfigModulesCommand, ConfigRmModuleCommand, ConfigRmTypeSlotCommand, ConfigSetCommand,
  ConfigSetTypeSlotCommand, ConfigShowCommand, ConfigSubcommand, ConfigTypeSlotsCommand, ConfigVersionCommand,
};
use calcit::snapshot;
use calcit::util::string::strip_shebang;
use cirru_edn::{Edn, EdnMapView};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::common::deps_path_for_snapshot;
use super::edit::{load_snapshot, save_snapshot};

fn load_snapshot_for_display(input_path: &str) -> Result<snapshot::Snapshot, String> {
  if !Path::new(input_path).exists() {
    return Err(format!("{input_path} does not exist"));
  }
  let mut content = fs::read_to_string(input_path).map_err(|e| format!("Failed to read file: {e}"))?;
  strip_shebang(&mut content);
  let data = cirru_edn::parse(&content).map_err(|e| {
    eprintln!("\nFailed to parse file '{input_path}':");
    eprintln!("{e}");
    format!("Failed to parse file '{input_path}'")
  })?;
  snapshot::load_snapshot_data(&data, input_path)
}

pub fn handle_config_command(cmd: &ConfigCommand, snapshot_file: &str) -> Result<(), String> {
  match &cmd.subcommand {
    ConfigSubcommand::Show(opts) => handle_show(opts, snapshot_file),
    ConfigSubcommand::Modules(opts) => handle_modules(opts, snapshot_file),
    ConfigSubcommand::TypeSlots(opts) => handle_type_slots(opts, snapshot_file),
    ConfigSubcommand::Version(opts) => handle_version(opts, snapshot_file),
    ConfigSubcommand::Set(opts) => handle_set(opts, snapshot_file),
    ConfigSubcommand::AddModule(opts) => handle_add_module(opts, snapshot_file),
    ConfigSubcommand::RmModule(opts) => handle_rm_module(opts, snapshot_file),
    ConfigSubcommand::SetTypeSlot(opts) => handle_set_type_slot(opts, snapshot_file),
    ConfigSubcommand::RmTypeSlot(opts) => handle_rm_type_slot(opts, snapshot_file),
  }
}

fn format_type_slots(type_slots: &std::collections::HashMap<String, String>) -> String {
  let mut pairs: Vec<String> = type_slots
    .iter()
    .map(|(slot, type_path)| format!(":{slot} -> {type_path}"))
    .collect();
  pairs.sort();
  format!("{{{}}}", pairs.join(", "))
}

fn handle_show(opts: &ConfigShowCommand, input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_display(input_path)?;

  if let Some(name) = &opts.entry {
    let entry = snapshot.entries.get(name).ok_or_else(|| {
      format!(
        "Entry '{name}' not found. Available: {}",
        snapshot.entries.keys().cloned().collect::<Vec<_>>().join(", ")
      )
    })?;
    println!("{}", format!("Entry '{name}':").bold());
    println!("  {}: {}", "mode".cyan(), entry.mode);
    println!("  {}: {}", "init_fn".cyan(), entry.init_fn);
    println!("  {}: {}", "reload_fn".cyan(), entry.reload_fn);
    println!("  {}: {}", "description".cyan(), entry.description);
    println!("  {}: {:?}", "modules".cyan(), entry.modules);
    println!("  {}: {}", "type_slots".cyan(), format_type_slots(&entry.type_slots));
    return Ok(());
  }

  println!("{}", "Project Config:".bold());
  let deps_path = deps_path_for_snapshot(input_path);
  println!("  {}: managed in deps.cirru (use `caps version get {deps_path}`)", "version".cyan());
  println!("\n{}", "Snapshot Entries:".bold());

  let mut names: Vec<&String> = snapshot.entries.keys().collect();
  names.sort();

  for name in names {
    let entry = snapshot
      .entries
      .get(name)
      .ok_or_else(|| format!("Missing entry config for '{name}'"))?;

    println!("  {}", name.cyan());
    println!("    {}: {}", "mode".cyan(), entry.mode);
    println!("    {}: {}", "init_fn".cyan(), entry.init_fn);
    println!("    {}: {}", "reload_fn".cyan(), entry.reload_fn);
    println!("    {}: {}", "description".cyan(), entry.description);
    println!("    {}: {:?}", "modules".cyan(), entry.modules);
    println!("    {}: {}", "type_slots".cyan(), format_type_slots(&entry.type_slots));
  }

  Ok(())
}

fn handle_type_slots(opts: &ConfigTypeSlotsCommand, input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_display(input_path)?;
  let name = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  let (label, type_slots) = {
    let entry = snapshot.entries.get(name).ok_or_else(|| {
      format!(
        "Entry '{name}' not found. Available: {}",
        snapshot.entries.keys().cloned().collect::<Vec<_>>().join(", ")
      )
    })?;
    (format!("Type slots in entry '{name}':"), &entry.type_slots)
  };

  println!("{}", label.bold());
  if type_slots.is_empty() {
    println!("  {}", "(none)".dimmed());
  } else {
    let mut slots: Vec<(&String, &String)> = type_slots.iter().collect();
    slots.sort_by_key(|(slot, _)| *slot);
    for (slot, type_path) in slots {
      println!("  {} -> {}", format!(":{slot}").cyan(), type_path);
    }
  }
  Ok(())
}

fn handle_modules(opts: &ConfigModulesCommand, input_path: &str) -> Result<(), String> {
  let snapshot = load_snapshot_for_display(input_path)?;

  let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
  let module_folder = calcit::project_module_folder(base_dir);

  let name = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  let (label, modules) = {
    let entry = snapshot.entries.get(name).ok_or_else(|| {
      format!(
        "Entry '{name}' not found. Available: {}",
        snapshot.entries.keys().cloned().collect::<Vec<_>>().join(", ")
      )
    })?;
    (format!("Modules in entry '{name}':"), entry.modules.clone())
  };

  println!("{}", label.bold());
  if opts.entry.is_none() {
    println!("  {} {}", snapshot.package.cyan(), "(main)".dimmed());
  }

  for module_path in &modules {
    match load_module_silent(module_path, base_dir, &module_folder) {
      Ok(module_snapshot) => {
        println!("  {} {}", module_snapshot.package.cyan(), format!("({module_path})").dimmed());
      }
      Err(_) => {
        println!("  {} {}", module_path.yellow(), "(failed)".red());
      }
    }
  }

  if opts.entry.is_none() && !snapshot.entries.is_empty() {
    println!("\n{}", "Entries:".bold());
    for name in snapshot.entries.keys() {
      println!("  {}", name.cyan());
    }
  }

  Ok(())
}

fn load_module_silent(module_path: &str, base_dir: &Path, module_folder: &Path) -> Result<snapshot::Snapshot, String> {
  let mut last_error = None;
  for (_, candidate, _) in calcit::resolve_module_snapshot_candidates(module_path, base_dir, module_folder) {
    if candidate.exists() {
      let mut content = match fs::read_to_string(&candidate) {
        Ok(content) => content,
        Err(error) => {
          last_error = Some(format!("Failed to read {}: {error}", candidate.display()));
          continue;
        }
      };
      strip_shebang(&mut content);
      let data = match cirru_edn::parse(&content) {
        Ok(data) => data,
        Err(error) => {
          last_error = Some(format!("Failed to parse {}: {error}", candidate.display()));
          continue;
        }
      };
      match snapshot::load_snapshot_data(&data, &candidate.to_string_lossy()) {
        Ok(snapshot) => return Ok(snapshot),
        Err(error) => last_error = Some(format!("Failed to load {}: {error}", candidate.display())),
      }
    }
  }

  Err(last_error.unwrap_or_else(|| format!("Module not found: {module_path}")))
}

fn handle_version(opts: &ConfigVersionCommand, snapshot_file: &str) -> Result<(), String> {
  let replacement = version_command_replacement(opts.value.as_deref(), snapshot_file);
  eprintln!(
    "[Deprecated] `calcit config version` no longer writes `calcit.cirru :version`; use {replacement}, which manages `deps.cirru :version`"
  );
  let _ = snapshot_file;
  Err(format!("Project version is stored in deps.cirru; run {replacement}"))
}

fn handle_set(opts: &ConfigSetCommand, snapshot_file: &str) -> Result<(), String> {
  if opts.key == "version" {
    let replacement = version_command_replacement(Some(opts.value.as_str()), snapshot_file);
    eprintln!(
      "[Deprecated] `calcit config set version` no longer writes `calcit.cirru :version`; use {replacement} to manage `deps.cirru :version`"
    );
    return Err(format!("Project version is stored in deps.cirru; run {replacement}"));
  }

  let mut snapshot = load_snapshot(snapshot_file)?;

  let entry_label = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  let entry = select_entry_mut(&mut snapshot, opts.entry.as_deref())?;

  let message = match opts.key.as_str() {
    "mode" => {
      entry.mode = match opts.value.trim_start_matches(':') {
        "native" => snapshot::SnapshotRunMode::Native,
        "js" => snapshot::SnapshotRunMode::Js,
        _ => return Err(format!("Unknown run mode '{}'. Valid modes: native, js", opts.value)),
      };
      format!("{} Set [{entry_label}] mode = '{}'", "✓".green(), entry.mode)
    }
    "init-fn" | "init_fn" => {
      entry.init_fn = opts.value.clone();
      format!("{} Set [{entry_label}] '{}' = '{}'", "✓".green(), opts.key.cyan(), opts.value)
    }
    "reload-fn" | "reload_fn" => {
      entry.reload_fn = opts.value.clone();
      format!("{} Set [{entry_label}] '{}' = '{}'", "✓".green(), opts.key.cyan(), opts.value)
    }
    "description" => {
      entry.description = opts.value.clone();
      format!("{} Set [{entry_label}] description = '{}'", "✓".green(), opts.value)
    }
    _ => {
      return Err(format!(
        "Unknown config key '{}'. Valid keys: mode, init-fn, reload-fn, description, version (accepts semver string or patch|minor|major)",
        opts.key
      ));
    }
  };

  save_snapshot(&snapshot, snapshot_file)?;
  println!("{message}");
  Ok(())
}

fn version_command_replacement(value: Option<&str>, snapshot_file: &str) -> String {
  let deps_path = deps_path_for_snapshot(snapshot_file);
  match value {
    None => format!("`caps version get {deps_path}`"),
    Some(v @ ("patch" | "minor" | "major")) => format!("`caps version bump {v} {deps_path}`"),
    Some(v) => format!("`caps version set {v} {deps_path}`"),
  }
}

fn handle_add_module(opts: &ConfigAddModuleCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  if let Some(name) = &opts.entry
    && !snapshot.entries.contains_key(name)
  {
    let available: Vec<_> = snapshot.entries.keys().cloned().collect();
    return Err(format!("Entry '{name}' not found. Available: {}", available.join(", ")));
  }

  let configs = select_entry_mut(&mut snapshot, opts.entry.as_deref())?;

  if configs.modules.contains(&opts.module_path) {
    return Err(format!("Module '{}' already exists", opts.module_path));
  }

  configs.modules.push(opts.module_path.clone());
  save_snapshot(&snapshot, snapshot_file)?;

  let scope = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  println!("{} Added module '{}' to [{scope}]", "✓".green(), opts.module_path.cyan());
  Ok(())
}

fn handle_rm_module(opts: &ConfigRmModuleCommand, snapshot_file: &str) -> Result<(), String> {
  let mut snapshot = load_snapshot(snapshot_file)?;

  if let Some(name) = &opts.entry
    && !snapshot.entries.contains_key(name)
  {
    let available: Vec<_> = snapshot.entries.keys().cloned().collect();
    return Err(format!("Entry '{name}' not found. Available: {}", available.join(", ")));
  }

  let configs = select_entry_mut(&mut snapshot, opts.entry.as_deref())?;

  let original_len = configs.modules.len();
  configs.modules.retain(|m| m != &opts.module_path);

  if configs.modules.len() == original_len {
    return Err(format!("Module '{}' not found", opts.module_path));
  }

  save_snapshot(&snapshot, snapshot_file)?;
  let scope = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  println!("{} Removed module '{}' from [{scope}]", "✓".green(), opts.module_path.cyan());
  Ok(())
}

fn normalize_type_slot_name(raw: &str) -> Result<String, String> {
  let slot = raw.trim().trim_start_matches(':');
  if slot.is_empty() {
    Err("Type slot name cannot be empty".to_owned())
  } else {
    Ok(slot.to_owned())
  }
}

fn select_entry_mut<'a>(snapshot: &'a mut snapshot::Snapshot, entry: Option<&str>) -> Result<&'a mut snapshot::SnapshotEntry, String> {
  let name = entry.unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  let available = snapshot.entries.keys().cloned().collect::<Vec<_>>().join(", ");
  snapshot
    .entries
    .get_mut(name)
    .ok_or_else(|| format!("Entry '{name}' not found. Available: {available}"))
}

fn find_map_value_mut<'a>(map: &'a mut EdnMapView, key: &str) -> Option<&'a mut Edn> {
  let tag_key = Edn::tag(key);
  if map.0.contains_key(&tag_key) {
    return map.0.get_mut(&tag_key);
  }
  map.0.get_mut(&Edn::str(key))
}

fn find_map_value<'a>(map: &'a EdnMapView, key: &str) -> Option<&'a Edn> {
  map.get(&Edn::tag(key)).or_else(|| map.get(&Edn::str(key)))
}

fn type_slots_as_edn(type_slots: &HashMap<String, String>) -> Edn {
  let mut slots = EdnMapView::default();
  for (slot, type_path) in type_slots {
    let value = if type_path == ":dynamic" {
      Edn::tag("dynamic")
    } else {
      Edn::str(type_path.as_str())
    };
    slots.insert_key(slot.as_str(), value);
  }
  slots.into()
}

/// Save only a type-slot map against the original EDN tree. Going through the
/// typed Snapshot renderer would also canonicalize unrelated legacy schemas.
fn save_type_slots_preserving_snapshot(
  snapshot_file: &str,
  entry: Option<&str>,
  type_slots: &HashMap<String, String>,
) -> Result<(), String> {
  let original = fs::read_to_string(snapshot_file).map_err(|e| format!("Failed to read {snapshot_file}: {e}"))?;
  let shebang = original.lines().next().filter(|line| line.starts_with("#!")).map(str::to_owned);
  let mut content = original;
  strip_shebang(&mut content);
  let mut data = cirru_edn::parse(&content).map_err(|e| format!("Failed to parse EDN: {e}"))?;
  let Edn::Map(root) = &mut data else {
    return Err("Snapshot root must be an EDN map".to_owned());
  };
  let has_default_entry = match find_map_value(root, "entries") {
    Some(Edn::Map(entries)) => find_map_value(entries, snapshot::DEFAULT_ENTRY_NAME).is_some(),
    _ => false,
  };

  let configs = if let Some(entry_name) = entry {
    let entries_value = find_map_value_mut(root, "entries").ok_or_else(|| "Snapshot is missing :entries".to_owned())?;
    let Edn::Map(entries) = entries_value else {
      return Err("Snapshot :entries must be an EDN map".to_owned());
    };
    let entry_value = find_map_value_mut(entries, entry_name).ok_or_else(|| format!("Entry '{entry_name}' not found"))?;
    let Edn::Map(entry_configs) = entry_value else {
      return Err(format!("Entry '{entry_name}' config must be an EDN map"));
    };
    entry_configs
  } else if has_default_entry {
    let entries_value = find_map_value_mut(root, "entries").ok_or_else(|| "Snapshot is missing :entries".to_owned())?;
    let Edn::Map(entries) = entries_value else {
      return Err("Snapshot :entries must be an EDN map".to_owned());
    };
    let entry_value =
      find_map_value_mut(entries, snapshot::DEFAULT_ENTRY_NAME).ok_or_else(|| "Snapshot is missing :entries.default".to_owned())?;
    let Edn::Map(entry_configs) = entry_value else {
      return Err("Entry 'default' config must be an EDN map".to_owned());
    };
    entry_configs
  } else {
    let configs_value = find_map_value_mut(root, "configs").ok_or_else(|| "Snapshot is missing :entries or :configs".to_owned())?;
    let Edn::Map(configs) = configs_value else {
      return Err("Snapshot :configs must be an EDN map".to_owned());
    };
    configs
  };

  configs.insert_key("type-slots", type_slots_as_edn(type_slots));
  let formatted = cirru_edn::format(&data, true).map_err(|e| format!("Failed to format snapshot EDN: {e}"))?;
  let output = match shebang {
    Some(line) => format!("{line}\n{formatted}"),
    None => formatted,
  };
  fs::write(snapshot_file, output).map_err(|e| format!("Failed to write {snapshot_file}: {e}"))
}

fn handle_set_type_slot(opts: &ConfigSetTypeSlotCommand, snapshot_file: &str) -> Result<(), String> {
  let slot = normalize_type_slot_name(&opts.slot)?;
  let type_path = opts.type_path.trim();
  if !matches!(type_path, ":dynamic" | "dynamic") {
    let Some((ns, def)) = type_path.rsplit_once('/') else {
      return Err(format!(
        "Type slot binding must use a full `namespace/definition` path or `:dynamic`, got `{type_path}`"
      ));
    };
    if ns.is_empty() || def.is_empty() {
      return Err(format!(
        "Type slot binding must use a full `namespace/definition` path or `:dynamic`, got `{type_path}`"
      ));
    }
  }

  let mut snapshot = load_snapshot(snapshot_file)?;
  let scope = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  let configs = select_entry_mut(&mut snapshot, opts.entry.as_deref())?;
  let normalized_type = if type_path == "dynamic" { ":dynamic" } else { type_path };
  let previous = configs.type_slots.insert(slot.clone(), normalized_type.to_owned());
  save_type_slots_preserving_snapshot(snapshot_file, opts.entry.as_deref(), &configs.type_slots)?;

  if let Some(previous) = previous {
    println!(
      "{} Updated type slot ':{}' in [{scope}]: {} -> {}",
      "✓".green(),
      slot.cyan(),
      previous.yellow(),
      normalized_type.green()
    );
  } else {
    println!(
      "{} Bound type slot ':{}' to '{}' in [{scope}]",
      "✓".green(),
      slot.cyan(),
      normalized_type.green()
    );
  }
  Ok(())
}

fn handle_rm_type_slot(opts: &ConfigRmTypeSlotCommand, snapshot_file: &str) -> Result<(), String> {
  let slot = normalize_type_slot_name(&opts.slot)?;
  let mut snapshot = load_snapshot(snapshot_file)?;
  let scope = opts.entry.as_deref().unwrap_or(snapshot::DEFAULT_ENTRY_NAME);
  let configs = select_entry_mut(&mut snapshot, opts.entry.as_deref())?;
  let Some(previous) = configs.type_slots.remove(&slot) else {
    return Err(format!("Type slot ':{slot}' is not bound in [{scope}]"));
  };
  save_type_slots_preserving_snapshot(snapshot_file, opts.entry.as_deref(), &configs.type_slots)?;
  println!(
    "{} Removed type slot ':{}' ({}) from [{scope}]",
    "✓".green(),
    slot.cyan(),
    previous.yellow()
  );
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn config_module_lookup_prefers_project_module_view() {
    let root = std::env::temp_dir().join(format!("calcit-config-module-view-{}", std::process::id()));
    let project = root.join("project");
    let global = root.join("global");
    fs::create_dir_all(project.join(".calcit/modules/demo")).unwrap();
    fs::create_dir_all(global.join("demo")).unwrap();
    let snapshot = |package: &str| {
      format!(
        "{{}} (:package |{package})\n  :configs $ {{}} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.1)\n    :modules $ []\n  :files $ {{}}\n"
      )
    };
    fs::write(project.join(".calcit/modules/demo/compact.cirru"), "invalid snapshot").unwrap();
    fs::write(project.join(".calcit/modules/demo/calcit.cirru"), snapshot("project-demo")).unwrap();
    fs::write(global.join("demo/calcit.cirru"), snapshot("global-demo")).unwrap();

    let loaded = load_module_silent("demo/", &project, &calcit::project_module_folder(&project)).unwrap();
    assert_eq!(loaded.package, "project-demo");
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn deprecated_version_set_does_not_load_snapshot_before_migration_error() {
    let opts = ConfigSetCommand {
      entry: Some("missing-entry".to_owned()),
      key: "version".to_owned(),
      value: "patch".to_owned(),
    };
    let err = handle_set(&opts, "/missing-project/calcit.cirru").expect_err("version migration should always fail explicitly");
    assert!(
      err.contains("caps version bump patch /missing-project/deps.cirru"),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn type_slot_edn_mutation_preserves_unrelated_snapshot_data() {
    let source = r#"{} (:package |demo)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.1)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |f $ %{} :CodeEntry (:schema $ :: :fn $ {} (:rest :any) (:return $ :: :list :any))
          :code $ quote (defn f (x) x)
      :ns $ %{} :CodeEntry (:code $ quote $ ns app.main)
"#;
    let before = cirru_edn::parse(source).expect("source snapshot");
    let temp_path = std::env::temp_dir().join(format!("calcit-type-slots-preserve-{}.cirru", std::process::id()));
    fs::write(&temp_path, source).expect("write fixture");

    let slots = HashMap::from([("dispatch-op".to_owned(), "app.schema/Op".to_owned())]);
    save_type_slots_preserving_snapshot(&temp_path.to_string_lossy(), None, &slots).expect("save type slots");
    let after_text = fs::read_to_string(&temp_path).expect("read output");
    let after = cirru_edn::parse(&after_text).expect("output snapshot");
    fs::remove_file(&temp_path).expect("remove fixture");

    let Edn::Map(before_root) = before else { panic!("before root") };
    let Edn::Map(after_root) = after else { panic!("after root") };
    let read_definition_field = |root: &EdnMapView, field: &str| {
      let Edn::Map(files) = root.get_or_nil("files") else {
        panic!("files")
      };
      let Edn::Struct(file) = files.get_or_nil("app.main") else {
        panic!("app.main")
      };
      let Edn::Map(defs) = file["defs"].clone() else { panic!("defs") };
      let Edn::Struct(definition) = defs.get_or_nil("f") else {
        panic!("f")
      };
      definition[field].clone()
    };
    assert_eq!(
      read_definition_field(&before_root, "code"),
      read_definition_field(&after_root, "code")
    );
    assert_eq!(
      read_definition_field(&before_root, "schema"),
      read_definition_field(&after_root, "schema")
    );
    let Edn::Map(configs) = after_root.get_or_nil("configs") else {
      panic!("configs")
    };
    let Edn::Map(saved_slots) = configs.get_or_nil("type-slots") else {
      panic!("type slots")
    };
    assert_eq!(saved_slots.get_or_nil("dispatch-op"), Edn::str("app.schema/Op"));
  }
}

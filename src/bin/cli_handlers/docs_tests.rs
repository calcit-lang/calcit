use super::{
  GuideDoc, GuideDocFrontmatter, GuideDocScope, collect_check_md_module_paths, collect_docs_for_query, collect_search_results,
  find_doc_by_query, load_agents_document, load_entry_snapshot_for_check_md, load_module_docs_from_dir, parse_doc_frontmatter,
  parse_doc_knowledge_metadata, score_doc_query, score_doc_shape, validate_doc_frontmatter,
};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
  let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  std::env::temp_dir().join(format!("calcit-docs-{label}-{nanos}"))
}

fn write_file(path: &Path, content: &str) {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).unwrap();
  }
  fs::write(path, content).unwrap();
}

#[test]
fn agents_docs_default_to_the_version_matched_embedded_guide() {
  let document = load_agents_document(false).expect("embedded Agents guide should load without network");

  assert!(!document.refreshed);
  assert!(document.display_path.starts_with("embedded:docs/CalcitAgent.md@"));
  assert!(document.content.contains("是 **Cirru EDN 树形 Snapshot**"));
  assert!(document.content.contains("x $ (f a)"));
  assert!(document.content.contains("symbol leaf:    quote new-name"));
  assert!(document.content.contains("cr cirru parse -e --validate"));
  assert!(document.content.contains("cr docs search 'cursor'"));
}

#[test]
fn collect_check_md_module_paths_merges_entry_modules_with_cli_deps() {
  let root = unique_temp_dir("check-md-modules");
  let entry = root.join("mini.cirru");
  let content = r#"{} (:package |mini)
  :configs $ {} (:init-fn |mini/main!) (:reload-fn |mini/main!) (:version |0.0.0)
    :modules $ [] |respo.calcit/ |memof/
  :entries $ {}
  :files $ {}
    |mini $ %{} :FileEntry
      :ns $ %{} :CodeEntry (:doc |) (:code $ quote (ns mini)) (:examples $ []) (:schema nil)
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn main! () nil)
          :examples $ []
          :schema nil
"#;
  write_file(&entry, content);

  let modules = collect_check_md_module_paths(
    entry.to_str().expect("entry path should be utf-8"),
    &[String::from("lilac/"), String::from("memof/")],
  )
  .expect("collect modules");

  assert_eq!(
    modules,
    vec![String::from("respo.calcit/"), String::from("memof/"), String::from("lilac/")]
  );

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn load_entry_snapshot_for_check_md_reads_respo_project() {
  let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../respo/respo/calcit.cirru");
  if !entry.exists() {
    return;
  }
  let snapshot = load_entry_snapshot_for_check_md(entry.to_str().expect("entry path utf-8")).expect("load respo entry");
  assert!(snapshot.files.contains_key("respo.core"));
}

#[test]
fn collect_docs_for_query_uses_guidebook_without_module() {
  match collect_docs_for_query(None) {
    Ok(docs) => {
      assert!(!docs.is_empty());
      assert!(
        docs
          .iter()
          .any(|doc| doc.filename == "CalcitAgent.md" || doc.path.ends_with("CalcitAgent.md"))
      );
    }
    Err(err) => {
      assert!(err.contains("Guidebook documentation directory not found") || err.contains("Failed to read directory"));
    }
  }
}

#[test]
fn load_module_docs_from_dir_reads_agents_and_docs() {
  let root = unique_temp_dir("modules-read");
  let modules_dir = root.join("modules");
  write_file(&modules_dir.join("respo.calcit/Agents.md"), "# Respo Agent\n");
  write_file(&modules_dir.join("respo.calcit/docs/api.md"), "# API\nrender!\n");
  write_file(&modules_dir.join("memof/Agents.md"), "# Memof Agent\n");

  let docs = load_module_docs_from_dir(&modules_dir, Some("respo.calcit")).unwrap();
  assert_eq!(docs.len(), 2);
  assert!(docs.iter().any(|doc| doc.path == "respo.calcit/Agents.md"));
  assert!(docs.iter().any(|doc| doc.path == "docs/api.md"));

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn load_module_docs_from_dir_errors_on_missing_module() {
  let root = unique_temp_dir("modules-missing");
  let modules_dir = root.join("modules");
  fs::create_dir_all(&modules_dir).unwrap();

  let err = load_module_docs_from_dir(&modules_dir, Some("missing.module")).unwrap_err();
  assert!(err.contains("Module 'missing.module' not found"));

  fs::remove_dir_all(root).unwrap();
}

#[test]
fn collect_search_results_prefers_heading_matches_over_example_mentions() {
  let docs = vec![
    GuideDoc {
      filename: "docs-indexing.md".to_string(),
      path: "docs-indexing.md".to_string(),
      content: "# Documentation Indexing\n\n```bash\ncr docs search polymorphism\n```\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter::default(),
    },
    GuideDoc {
      filename: "features.md".to_string(),
      path: "features.md".to_string(),
      content: "# Polymorphism\n\nPolymorphism allows generic behavior.\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter::default(),
    },
  ];

  let results = collect_search_results(&docs, "polymorphism", 2, None);
  assert_eq!(results.len(), 2);
  assert_eq!(docs[results[0].doc_index].filename, "features.md");
}

#[test]
fn collect_search_results_prefers_filename_matches() {
  let docs = vec![
    GuideDoc {
      filename: "traits.md".to_string(),
      path: "traits.md".to_string(),
      content: "# Notes\n\nTraits discussion appears here.\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter::default(),
    },
    GuideDoc {
      filename: "guide.md".to_string(),
      path: "guide.md".to_string(),
      content: "# Traits\n\nTraits overview.\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter::default(),
    },
  ];

  let results = collect_search_results(&docs, "traits", 2, None);
  assert_eq!(results.len(), 2);
  assert_eq!(docs[results[0].doc_index].filename, "traits.md");
}

#[test]
fn parse_doc_frontmatter_extracts_lists_and_strips_header() {
  let raw = "---\ntitle: \"Run Calcit\"\nkind: hub\ncategory: run\naliases:\n  - watch mode\n  - eval\nentry_for:\n  - cr eval\n---\n# Run Calcit\n\nBody text.\n";
  let (frontmatter, body) = parse_doc_frontmatter(raw);

  assert_eq!(frontmatter.title.as_deref(), Some("Run Calcit"));
  assert_eq!(frontmatter.scope.as_deref(), None);
  assert_eq!(frontmatter.kind.as_deref(), Some("hub"));
  assert_eq!(frontmatter.category.as_deref(), Some("run"));
  assert_eq!(frontmatter.aliases, vec!["watch mode", "eval"]);
  assert_eq!(frontmatter.entry_for, vec!["cr eval"]);
  assert!(body.starts_with("# Run Calcit"));
}

#[test]
fn parse_doc_knowledge_metadata_extracts_graph_fields() {
  let raw = "---\nid: api/list/nth\nparent: structure/list\nrelated:\n  - api/list/get\nrequires: concept/indexing\nleads_to:\n  - example/list-access\n---\n# List nth\n";
  let metadata = parse_doc_knowledge_metadata(raw);

  assert_eq!(metadata.id.as_deref(), Some("api/list/nth"));
  assert_eq!(metadata.parent.as_deref(), Some("structure/list"));
  assert_eq!(metadata.related, vec!["api/list/get"]);
  assert_eq!(metadata.requires, vec!["concept/indexing"]);
  assert_eq!(metadata.leads_to, vec!["example/list-access"]);
}

#[test]
fn parse_doc_knowledge_metadata_supports_scalar_and_repeated_edges() {
  let raw = "---\nid: concept/list\nrelated: structure/sequence\nrelated:\n  - api/list/map\n  - api/list/foldl\nrequires: concept/collection\n---\n# List\n";
  let metadata = parse_doc_knowledge_metadata(raw);

  assert_eq!(metadata.id.as_deref(), Some("concept/list"));
  assert_eq!(metadata.related, vec!["structure/sequence", "api/list/map", "api/list/foldl"]);
  assert_eq!(metadata.requires, vec!["concept/collection"]);
}

#[test]
fn parse_doc_knowledge_metadata_ignores_legacy_fields_and_requires_closing_marker() {
  let legacy = "---\ntitle: List\naliases:\n  - sequence\nentry_for:\n  - list access\n---\n# List\n";
  let legacy_metadata = parse_doc_knowledge_metadata(legacy);
  assert_eq!(legacy_metadata.title.as_deref(), Some("List"));
  assert!(legacy_metadata.id.is_none());
  assert!(legacy_metadata.related.is_empty());

  let unterminated = "---\nid: broken\nparent: structure/list\n";
  assert_eq!(parse_doc_knowledge_metadata(unterminated), Default::default());
}

#[test]
fn repository_sample_docs_form_a_navigable_learning_chain() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let sample_paths = [
    "docs/CalcitAgent.md",
    "docs/docs.md",
    "docs/docs-indexing.md",
    "docs/features.md",
    "docs/features/list.md",
    "docs/run.md",
    "docs/run/query.md",
    "docs/run/edit-tree.md",
  ];

  let mut metadata = Vec::new();
  for relative_path in sample_paths {
    let content = fs::read_to_string(root.join(relative_path)).expect("sample doc should be readable");
    metadata.push((relative_path, parse_doc_knowledge_metadata(&content)));
  }

  let find = |id: &str| {
    metadata
      .iter()
      .find(|(_, item)| item.id.as_deref() == Some(id))
      .map(|(_, item)| item)
  };
  assert!(find("core/agent").unwrap().leads_to.contains(&"core/run/quick-start".to_string()));
  assert!(find("core/docs").unwrap().leads_to.contains(&"core/docs/indexing".to_string()));
  assert_eq!(find("core/docs/indexing").unwrap().parent.as_deref(), Some("core/docs"));
  assert!(
    find("core/features/list")
      .unwrap()
      .related
      .contains(&"core/features/hashmap".to_string())
  );
  assert!(find("core/run/query").unwrap().leads_to.contains(&"core/run/edit-tree".to_string()));
  assert_eq!(find("core/run/query").unwrap().parent.as_deref(), Some("core/run"));
  assert_eq!(find("core/run/edit-tree").unwrap().requires, vec!["core/run/query"]);

  let list_content = fs::read_to_string(root.join("docs/features/list.md")).expect("list doc should be readable");
  let (list_frontmatter, _) = parse_doc_frontmatter(&list_content);
  assert!(list_frontmatter.entry_for.iter().any(|entry| entry == "calcit.core/nth"));
  assert!(list_frontmatter.entry_for.iter().any(|entry| entry == "calcit.core/append"));
}

#[test]
fn repository_query_doc_frontmatter_keeps_search_metadata_separate() {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/run/query.md");
  let content = fs::read_to_string(path).expect("query doc should be readable");
  let (frontmatter, _) = parse_doc_frontmatter(&content);
  let knowledge = parse_doc_knowledge_metadata(&content);

  assert_eq!(frontmatter.title.as_deref(), Some("Querying Definitions"));
  assert!(frontmatter.summary.as_deref().is_some_and(|summary| summary.contains("cr query")));
  assert_eq!(knowledge.id.as_deref(), Some("core/run/query"));
}

#[test]
fn collect_search_results_uses_alias_matches_without_body_hits() {
  let docs = vec![GuideDoc {
    filename: "edit-tree.md".to_string(),
    path: "run/edit-tree.md".to_string(),
    content: "# CLI Code Editing\n\nFocused chapter for tree editing.\n".to_string(),
    scope: GuideDocScope::Core,
    frontmatter: GuideDocFrontmatter {
      title: Some("CLI Code Editing".to_string()),
      summary: None,
      scope: Some("core".to_string()),
      kind: Some("guide".to_string()),
      category: Some("run".to_string()),
      aliases: vec!["search-replace".to_string(), "tree replace".to_string()],
      entry_for: vec!["cr tree search-replace".to_string()],
    },
  }];

  let results = collect_search_results(&docs, "search-replace", 2, None);
  assert_eq!(results.len(), 1);
  assert_eq!(docs[results[0].doc_index].filename, "edit-tree.md");
}

#[test]
fn collect_search_results_prefers_guide_over_spec_on_same_metadata_hit() {
  let docs = vec![
    GuideDoc {
      filename: "docs-indexing.md".to_string(),
      path: "docs-indexing.md".to_string(),
      content: "# Documentation Indexing Spec\n\nBody.\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter {
        title: Some("Documentation Indexing Spec".to_string()),
        summary: None,
        scope: Some("core".to_string()),
        kind: Some("spec".to_string()),
        category: Some("docs".to_string()),
        aliases: vec!["search-replace".to_string()],
        entry_for: vec![],
      },
    },
    GuideDoc {
      filename: "edit-tree.md".to_string(),
      path: "run/edit-tree.md".to_string(),
      content: "# CLI Code Editing\n\nBody.\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter {
        title: Some("CLI Code Editing".to_string()),
        summary: None,
        scope: Some("core".to_string()),
        kind: Some("guide".to_string()),
        category: Some("run".to_string()),
        aliases: vec!["search-replace".to_string()],
        entry_for: vec![],
      },
    },
  ];

  let results = collect_search_results(&docs, "search-replace", 2, None);
  assert_eq!(docs[results[0].doc_index].filename, "edit-tree.md");
}

#[test]
fn score_doc_shape_prefers_guides_to_specs() {
  let guide = GuideDoc {
    filename: "guide.md".to_string(),
    path: "guide.md".to_string(),
    content: "# Guide\n".to_string(),
    scope: GuideDocScope::Core,
    frontmatter: GuideDocFrontmatter {
      title: None,
      summary: None,
      scope: Some("core".to_string()),
      kind: Some("guide".to_string()),
      category: Some("run".to_string()),
      aliases: vec![],
      entry_for: vec![],
    },
  };
  let spec = GuideDoc {
    filename: "spec.md".to_string(),
    path: "spec.md".to_string(),
    content: "# Spec\n".to_string(),
    scope: GuideDocScope::Core,
    frontmatter: GuideDocFrontmatter {
      title: None,
      summary: None,
      scope: Some("core".to_string()),
      kind: Some("spec".to_string()),
      category: Some("docs".to_string()),
      aliases: vec![],
      entry_for: vec![],
    },
  };

  assert!(score_doc_shape(&guide) > score_doc_shape(&spec));
}

#[test]
fn parse_doc_frontmatter_reads_scope_field() {
  let raw = "---\ntitle: \"Module API\"\nscope: \"module\"\nkind: reference\ncategory: api\n---\n# Module API\n";
  let (frontmatter, body) = parse_doc_frontmatter(raw);
  assert_eq!(frontmatter.scope.as_deref(), Some("module"));
  assert_eq!(frontmatter.kind.as_deref(), Some("reference"));
  assert!(body.starts_with("# Module API"));
}

#[test]
fn find_doc_by_query_matches_aliases_and_titles() {
  let docs = vec![
    GuideDoc {
      filename: "edit-tree.md".to_string(),
      path: "run/edit-tree.md".to_string(),
      content: "# CLI Code Editing\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter {
        title: Some("CLI Code Editing".to_string()),
        summary: None,
        scope: Some("core".to_string()),
        kind: Some("guide".to_string()),
        category: Some("run".to_string()),
        aliases: vec!["search-replace".to_string()],
        entry_for: vec!["cr tree search-replace".to_string()],
      },
    },
    GuideDoc {
      filename: "docs-indexing.md".to_string(),
      path: "docs-indexing.md".to_string(),
      content: "# Documentation Indexing Spec\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter {
        title: Some("Documentation Indexing Spec".to_string()),
        summary: None,
        scope: Some("core".to_string()),
        kind: Some("spec".to_string()),
        category: Some("docs".to_string()),
        aliases: vec!["search indexing".to_string()],
        entry_for: vec![],
      },
    },
  ];

  assert_eq!(find_doc_by_query(&docs, "search-replace").unwrap().filename, "edit-tree.md");
  assert_eq!(find_doc_by_query(&docs, "CLI Code Editing").unwrap().filename, "edit-tree.md");
}

#[test]
fn score_doc_query_prefers_filename_exact_match() {
  let doc = GuideDoc {
    filename: "polymorphism.md".to_string(),
    path: "features/polymorphism.md".to_string(),
    content: "# Polymorphism\n".to_string(),
    scope: GuideDocScope::Core,
    frontmatter: GuideDocFrontmatter {
      title: Some("Polymorphism".to_string()),
      summary: None,
      scope: Some("core".to_string()),
      kind: Some("guide".to_string()),
      category: Some("features".to_string()),
      aliases: vec!["trait dispatch".to_string()],
      entry_for: vec![],
    },
  };

  assert!(score_doc_query(&doc, "polymorphism.md") > score_doc_query(&doc, "trait dispatch"));
}

#[test]
fn find_doc_by_query_can_resolve_module_agents_title() {
  let docs = vec![GuideDoc {
    filename: "Agents.md".to_string(),
    path: "respo.calcit/Agents.md".to_string(),
    content: "# Respo Development Guide for LLM Agents\n".to_string(),
    scope: GuideDocScope::Module("respo.calcit".to_string()),
    frontmatter: GuideDocFrontmatter {
      title: Some("Respo-Agent.md".to_string()),
      summary: None,
      scope: Some("module".to_string()),
      kind: Some("agent".to_string()),
      category: Some("docs".to_string()),
      aliases: vec!["Respo-Agent".to_string()],
      entry_for: vec!["render!".to_string()],
    },
  }];

  assert_eq!(find_doc_by_query(&docs, "Respo-Agent").unwrap().path, "respo.calcit/Agents.md");
}

#[test]
fn collect_search_results_prefers_module_style_guide_for_defstyle_query() {
  let docs = vec![
    GuideDoc {
      filename: "styles.md".to_string(),
      path: "docs/guide/styles.md".to_string(),
      content: "## Styles\n\nStatic style guide for Respo.\n".to_string(),
      scope: GuideDocScope::Module("respo.calcit".to_string()),
      frontmatter: GuideDocFrontmatter {
        title: Some("Styles".to_string()),
        summary: None,
        scope: Some("module".to_string()),
        kind: Some("guide".to_string()),
        category: Some("ecosystem".to_string()),
        aliases: vec!["defstyle".to_string(), ":class-name".to_string(), "style map".to_string()],
        entry_for: vec!["style extraction".to_string(), "static styles".to_string()],
      },
    },
    GuideDoc {
      filename: "api.md".to_string(),
      path: "docs/api.md".to_string(),
      content: "## Respo API\n\ndefcomp render! clear-cache!\n".to_string(),
      scope: GuideDocScope::Module("respo.calcit".to_string()),
      frontmatter: GuideDocFrontmatter {
        title: Some("Respo API".to_string()),
        summary: None,
        scope: Some("module".to_string()),
        kind: Some("overview".to_string()),
        category: Some("reference".to_string()),
        aliases: vec!["respo api".to_string()],
        entry_for: vec!["api reference".to_string()],
      },
    },
  ];

  let results = collect_search_results(&docs, "defstyle", 2, None);
  assert_eq!(results.len(), 1);
  assert_eq!(docs[results[0].doc_index].filename, "styles.md");
}

#[test]
fn find_doc_by_query_matches_module_entry_for_terms() {
  let docs = vec![GuideDoc {
    filename: "server-rendering.md".to_string(),
    path: "docs/guide/server-rendering.md".to_string(),
    content: "## Server Rendering\n\nSSR flow for Respo.\n".to_string(),
    scope: GuideDocScope::Module("respo.calcit".to_string()),
    frontmatter: GuideDocFrontmatter {
      title: Some("Server Rendering".to_string()),
      summary: None,
      scope: Some("module".to_string()),
      kind: Some("guide".to_string()),
      category: Some("ecosystem".to_string()),
      aliases: vec!["SSR".to_string(), "server side rendering".to_string()],
      entry_for: vec!["realize-ssr!".to_string(), "hydrate app".to_string()],
    },
  }];

  assert_eq!(
    find_doc_by_query(&docs, "server side rendering").unwrap().filename,
    "server-rendering.md"
  );
  assert_eq!(find_doc_by_query(&docs, "hydrate app").unwrap().filename, "server-rendering.md");
}

#[test]
fn find_doc_by_query_matches_symbol_aliases_for_module_docs() {
  let docs = vec![GuideDoc {
    filename: "pick-states.md".to_string(),
    path: "docs/apis/pick-states.md".to_string(),
    content: "## >>\n\nPick nested states.\n".to_string(),
    scope: GuideDocScope::Module("respo.calcit".to_string()),
    frontmatter: GuideDocFrontmatter {
      title: Some(">>".to_string()),
      summary: None,
      scope: Some("module".to_string()),
      kind: Some("reference".to_string()),
      category: Some("reference".to_string()),
      aliases: vec!["pick-states".to_string(), ">>".to_string(), "states cursor".to_string()],
      entry_for: vec!["state cursor".to_string(), "local states".to_string()],
    },
  }];

  assert_eq!(find_doc_by_query(&docs, ">>").unwrap().filename, "pick-states.md");
  assert_eq!(find_doc_by_query(&docs, "state cursor").unwrap().filename, "pick-states.md");
}

#[test]
fn validate_doc_frontmatter_accepts_registered_category() {
  let frontmatter = GuideDocFrontmatter {
    title: Some("Styles".to_string()),
    summary: None,
    scope: Some("module".to_string()),
    kind: Some("guide".to_string()),
    category: Some("ecosystem".to_string()),
    aliases: vec![],
    entry_for: vec![],
  };

  validate_doc_frontmatter("docs/guide/styles.md", &frontmatter).unwrap();
}

#[test]
fn validate_doc_frontmatter_rejects_unknown_category() {
  let frontmatter = GuideDocFrontmatter {
    title: Some("Broken".to_string()),
    summary: None,
    scope: Some("core".to_string()),
    kind: Some("guide".to_string()),
    category: Some("api".to_string()),
    aliases: vec![],
    entry_for: vec![],
  };

  let err = validate_doc_frontmatter("docs/broken.md", &frontmatter).unwrap_err();
  assert!(err.contains("Invalid frontmatter category 'api'"));
  assert!(err.contains("docs/docs-indexing.md"));
}

#[test]
fn load_module_docs_from_dir_rejects_invalid_category() {
  let root = unique_temp_dir("modules-invalid-category");
  let modules_dir = root.join("modules");
  write_file(
    &modules_dir.join("respo.calcit/docs/api.md"),
    "---\ntitle: \"API\"\nscope: \"module\"\nkind: reference\ncategory: api\n---\n# API\n",
  );

  let err = load_module_docs_from_dir(&modules_dir, Some("respo.calcit")).unwrap_err();
  assert!(err.contains("Invalid frontmatter category 'api'"));

  fs::remove_dir_all(root).unwrap();
}

use super::{
  GuideDoc, GuideDocFrontmatter, GuideDocScope, collect_docs_for_query, collect_search_results, find_doc_by_query,
  load_module_docs_from_dir, parse_doc_frontmatter, score_doc_query, score_doc_shape, validate_doc_frontmatter,
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
fn collect_docs_for_query_uses_guidebook_without_module() {
  let docs = collect_docs_for_query(None).expect("guidebook docs should load from the workspace");
  assert!(!docs.is_empty());
  assert!(docs.iter().any(|doc| doc.filename == "CalcitAgent.md" || doc.path.ends_with("CalcitAgent.md")));
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
fn collect_search_results_uses_alias_matches_without_body_hits() {
  let docs = vec![GuideDoc {
    filename: "edit-tree.md".to_string(),
    path: "run/edit-tree.md".to_string(),
    content: "# CLI Code Editing\n\nFocused chapter for tree editing.\n".to_string(),
    scope: GuideDocScope::Core,
    frontmatter: GuideDocFrontmatter {
      title: Some("CLI Code Editing".to_string()),
      scope: Some("core".to_string()),
      kind: Some("guide".to_string()),
      category: Some("run".to_string()),
      aliases: vec!["target-replace".to_string(), "tree replace".to_string()],
      entry_for: vec!["cr tree target-replace".to_string()],
    },
  }];

  let results = collect_search_results(&docs, "target-replace", 2, None);
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
        scope: Some("core".to_string()),
        kind: Some("spec".to_string()),
        category: Some("docs".to_string()),
        aliases: vec!["target-replace".to_string()],
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
        scope: Some("core".to_string()),
        kind: Some("guide".to_string()),
        category: Some("run".to_string()),
        aliases: vec!["target-replace".to_string()],
        entry_for: vec![],
      },
    },
  ];

  let results = collect_search_results(&docs, "target-replace", 2, None);
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
        scope: Some("core".to_string()),
        kind: Some("guide".to_string()),
        category: Some("run".to_string()),
        aliases: vec!["target-replace".to_string()],
        entry_for: vec!["cr tree target-replace".to_string()],
      },
    },
    GuideDoc {
      filename: "docs-indexing.md".to_string(),
      path: "docs-indexing.md".to_string(),
      content: "# Documentation Indexing Spec\n".to_string(),
      scope: GuideDocScope::Core,
      frontmatter: GuideDocFrontmatter {
        title: Some("Documentation Indexing Spec".to_string()),
        scope: Some("core".to_string()),
        kind: Some("spec".to_string()),
        category: Some("docs".to_string()),
        aliases: vec!["search indexing".to_string()],
        entry_for: vec![],
      },
    },
  ];

  assert_eq!(find_doc_by_query(&docs, "target-replace").unwrap().filename, "edit-tree.md");
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

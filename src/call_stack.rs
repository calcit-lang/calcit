use crate::calcit::Calcit;
use crate::calcit::NodeLocation;
use crate::data::cirru;
use crate::data::edn;
use cirru_edn::Edn;
use cirru_edn::EdnListView;
use std::fmt;
use std::fs;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

static TRACK_STACK: AtomicBool = AtomicBool::new(true);

/// control global stack usage
pub fn set_using_stack(b: bool) {
  TRACK_STACK.store(b, std::sync::atomic::Ordering::Relaxed);
}

/// defaults to `true``
pub fn using_stack() -> bool {
  TRACK_STACK.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, PartialEq, Clone, Eq, Ord, PartialOrd, Hash)]
pub struct CalcitStack {
  pub ns: Arc<str>,
  pub def: Arc<str>,
  pub code: Calcit, // built in functions may not contain code
  pub args: Vec<Calcit>,
  pub kind: StackKind,
}

impl fmt::Display for CalcitStack {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Stack {}/{} {}", self.ns, self.def, self.kind)
  }
}

#[derive(Debug, PartialEq, Clone, Eq, Ord, PartialOrd, Hash)]
pub enum StackKind {
  Fn,
  Proc,
  Method,
  Macro,
  /// tracks builtin syntax
  Syntax,
  /// track preprocessing, mainly used in js backend
  Codegen,
}

impl fmt::Display for StackKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self {
      Self::Fn => write!(f, "fn"),
      Self::Proc => write!(f, "proc"),
      Self::Method => write!(f, "method"),
      Self::Macro => write!(f, "macro"),
      Self::Syntax => write!(f, "syntax"),
      Self::Codegen => write!(f, "codegen"),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CallStackList(pub rpds::ListSync<CalcitStack>);

impl CallStackList {
  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn push_left(&self, v: CalcitStack) -> CallStackList {
    CallStackList(self.0.push_front(v))
  }

  /// create new entry to the tree
  pub fn extend(&self, ns: &str, def: &str, kind: StackKind, code: &Calcit, args: &[Calcit]) -> CallStackList {
    let b = TRACK_STACK.load(std::sync::atomic::Ordering::Relaxed);
    if b {
      self.push_left(CalcitStack {
        ns: Arc::from(ns),
        def: Arc::from(def),
        code: code.to_owned(),
        args: args.to_owned(),
        kind,
      })
    } else {
      self.to_owned()
    }
  }

  /// create new entry when code and args are already owned, avoiding an extra clone of args
  pub fn extend_owned(&self, ns: &str, def: &str, kind: StackKind, code: Calcit, args: Vec<Calcit>) -> CallStackList {
    let b = TRACK_STACK.load(std::sync::atomic::Ordering::Relaxed);
    if b {
      self.push_left(CalcitStack {
        ns: Arc::from(ns),
        def: Arc::from(def),
        code,
        args,
        kind,
      })
    } else {
      self.to_owned()
    }
  }
}

// show simplified version of stack
pub fn show_stack(stack: &CallStackList) {
  println!("\ncall stack:");
  for s in &stack.0 {
    let is_macro = s.kind == StackKind::Macro;
    println!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }
}

pub fn display_stack(failure: &str, stack: &CallStackList, location: Option<&Arc<NodeLocation>>) -> Result<(), String> {
  display_stack_with_docs(failure, stack, location, None)
}

pub fn display_stack_with_docs(
  failure: &str,
  stack: &CallStackList,
  location: Option<&Arc<NodeLocation>>,
  hint: Option<&str>,
) -> Result<(), String> {
  let fallback_location: Option<Arc<NodeLocation>> = location.cloned().or_else(|| {
    if let Some(loc) = find_preferred_macro_location(stack) {
      return Some(Arc::new(loc));
    }

    let mut candidates: Vec<Arc<NodeLocation>> = vec![];
    for s in &stack.0 {
      if let Some(loc) = find_location_in_calcit(&s.code).or_else(|| s.args.iter().find_map(find_location_in_calcit)) {
        candidates.push(Arc::new(loc));
      }
    }

    candidates
      .iter()
      .find(|loc| loc.ns.as_ref() != crate::calcit::CORE_NS)
      .cloned()
      .or_else(|| candidates.into_iter().next())
      .or_else(|| {
        stack
          .0
          .first()
          .map(|s| Arc::new(NodeLocation::new(s.ns.to_owned(), s.def.to_owned(), Arc::new(vec![]))))
      })
  });
  let mut stack_rows: Vec<(usize, &CalcitStack, Option<NodeLocation>)> = vec![];
  for (idx, s) in stack.0.iter().enumerate() {
    let stack_location = find_location_in_calcit(&s.code).or_else(|| s.args.iter().find_map(find_location_in_calcit));
    stack_rows.push((idx, s, stack_location));
  }

  let current_package = fallback_location
    .as_deref()
    .map(|l| root_ns(&l.ns).to_string())
    .or_else(|| {
      stack_rows.iter().find_map(|(_, _, loc)| {
        loc
          .as_ref()
          .and_then(|l| (!is_calcit_ns(&l.ns)).then(|| root_ns(&l.ns).to_string()))
      })
    })
    .or_else(|| {
      stack_rows
        .iter()
        .find_map(|(_, s, _)| (!is_calcit_ns(&s.ns)).then(|| root_ns(&s.ns).to_string()))
    })
    .unwrap_or_else(|| String::from(crate::calcit::CORE_NS));

  stack_rows.sort_by_key(|(idx, s, loc)| {
    let ns_for_priority = loc.as_ref().map(|l| l.ns.as_ref()).unwrap_or(s.ns.as_ref());
    let priority = if root_ns(ns_for_priority) == current_package {
      0
    } else if is_calcit_ns(ns_for_priority) {
      2
    } else {
      1
    };
    (priority, *idx)
  });

  eprintln!("\nStack:");
  for (_, s, stack_location) in &stack_rows {
    let is_macro = s.kind == StackKind::Macro;
    match stack_location {
      Some(l) => eprintln!("  {}/{}{} @ {l}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" }),
      None => eprintln!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" }),
    }
  }

  eprintln!("\nFailure: {failure}");
  if let Some(l) = fallback_location.as_deref() {
    eprintln!("  at {l}");
  }
  if let Some(h) = hint {
    eprintln!("\n{h}");
  } else if let Some((ns, def, examples)) = find_stack_examples(stack) {
    eprintln!("examples: cargo run --bin cr -- demos/calcit.cirru query examples {ns}/{def}");
    if !examples.is_empty() {
      eprintln!("sample examples:");
      for line in examples.iter().take(2) {
        eprintln!("  - {line}");
      }
    }
  }

  let mut stack_list = EdnListView::default();
  for (_, s, stack_location) in &stack_rows {
    let mut args = EdnListView::default();
    for v in s.args.iter() {
      let edn_val = edn::calcit_to_edn(v)?;
      args.push(edn::compact_edn_for_display(&edn_val, 0));
    }
    let mut info_map = vec![
      (Edn::tag("def"), format!("{}/{}", s.ns, s.def).into()),
      (
        Edn::tag("code"),
        Edn::str(edn::format_edn_display(&Edn::Quote(cirru::calcit_to_cirru(&s.code)?))),
      ),
      (Edn::tag("args"), args.into()),
      (Edn::tag("kind"), Edn::tag(s.kind.to_string())),
      (
        Edn::tag("location"),
        match stack_location {
          Some(l) => l.into(),
          None => Edn::Nil,
        },
      ),
    ];

    // Add documentation if available from program data
    if let Some(doc) = crate::program::lookup_def_doc(&s.ns, &s.def) {
      info_map.push((Edn::tag("doc"), doc.into()));
    }

    // Add examples if available from program data
    if let Some(examples) = crate::program::lookup_def_examples(&s.ns, &s.def) {
      let mut examples_list = EdnListView::default();
      for example in examples {
        examples_list.push(example.into());
      }
      info_map.push((Edn::tag("examples"), examples_list.into()));
    }

    let info = Edn::map_from_iter(info_map);
    stack_list.push(info);
  }

  let mut snapshot_fields = vec![(Edn::tag("message"), failure.into()), (Edn::tag("stack"), stack_list.into())];
  if let Some(h) = hint {
    snapshot_fields.push((Edn::tag("hint"), h.into()));
  }
  snapshot_fields.push((
    Edn::tag("location"),
    match fallback_location.as_deref() {
      Some(l) => l.into(),
      None => Edn::Nil,
    },
  ));

  let snapshot_edn = Edn::map_from_iter(snapshot_fields);
  let content = cirru_edn::format(&edn::compact_edn_for_display(&snapshot_edn, 0), true)?;
  let _ = fs::write(ERROR_SNAPSHOT, content);
  eprintln!("\nrun `cat {ERROR_SNAPSHOT}` to read stack details.");
  Ok(())
}

fn root_ns(ns: &str) -> &str {
  ns.split('.').next().unwrap_or(ns)
}

fn is_calcit_ns(ns: &str) -> bool {
  ns == crate::calcit::CORE_NS || ns.starts_with("calcit.")
}

const ERROR_SNAPSHOT: &str = ".calcit-error.cirru";

fn find_location_in_calcit(v: &Calcit) -> Option<NodeLocation> {
  match v {
    Calcit::List(items) => items.iter().find_map(find_location_in_calcit),
    Calcit::Recur(items) => items.iter().find_map(find_location_in_calcit),
    _ => v.get_location(),
  }
}

fn find_preferred_macro_location(stack: &CallStackList) -> Option<NodeLocation> {
  let mut macro_locations: Vec<NodeLocation> = vec![];
  for item in &stack.0 {
    if item.kind != StackKind::Macro {
      continue;
    }
    if let Some(loc) = find_location_in_calcit(&item.code).or_else(|| item.args.iter().find_map(find_location_in_calcit)) {
      macro_locations.push(loc);
    }
  }

  macro_locations
    .iter()
    .find(|loc| loc.ns.as_ref() != crate::calcit::CORE_NS)
    .cloned()
    .or_else(|| macro_locations.into_iter().next())
}

fn find_stack_examples(stack: &CallStackList) -> Option<(String, String, Vec<String>)> {
  let mut macro_fallback: Option<(String, String, Vec<String>)> = None;

  for item in &stack.0 {
    if item.kind != StackKind::Macro {
      continue;
    }
    if is_internal_macro_ns(&item.ns) {
      if macro_fallback.is_none() {
        macro_fallback = lookup_macro_examples_target(item);
      }
      continue;
    }

    if is_macro_called_from_user(item) {
      if let Some(target) = lookup_macro_examples_target(item) {
        return Some(target);
      }
    } else if macro_fallback.is_none() {
      macro_fallback = lookup_macro_examples_target(item);
    }
  }

  if macro_fallback.is_some() {
    return macro_fallback;
  }

  for item in &stack.0 {
    if let Some(examples) = crate::program::lookup_def_examples(&item.ns, &item.def) {
      if examples.is_empty() {
        continue;
      }
      let mut rendered = vec![];
      for example in examples.iter().take(2) {
        rendered.push(match cirru_parser::format_expr_one_liner(example) {
          Ok(v) => v.to_string(),
          Err(_) => example.to_string(),
        });
      }
      return Some((item.ns.to_string(), item.def.to_string(), rendered));
    }
  }
  None
}

fn lookup_macro_examples_target(item: &CalcitStack) -> Option<(String, String, Vec<String>)> {
  if let Some(examples) = crate::program::lookup_def_examples(&item.ns, &item.def) {
    if examples.is_empty() {
      return Some((item.ns.to_string(), item.def.to_string(), vec![]));
    }
    let mut rendered = vec![];
    for example in examples.iter().take(2) {
      rendered.push(match cirru_parser::format_expr_one_liner(example) {
        Ok(v) => v.to_string(),
        Err(_) => example.to_string(),
      });
    }
    return Some((item.ns.to_string(), item.def.to_string(), rendered));
  }
  if crate::program::has_def_code(&item.ns, &item.def) {
    return Some((item.ns.to_string(), item.def.to_string(), vec![]));
  }
  None
}

fn is_macro_called_from_user(item: &CalcitStack) -> bool {
  find_location_in_calcit(&item.code)
    .or_else(|| item.args.iter().find_map(find_location_in_calcit))
    .is_some_and(|loc| loc.ns.as_ref() != crate::calcit::CORE_NS)
}

fn is_internal_macro_ns(ns: &str) -> bool {
  ns == "calcit.internal" || ns.starts_with("calcit.internal.")
}

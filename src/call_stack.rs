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
    stack
      .0
      .first()
      .map(|s| Arc::new(NodeLocation::new(s.ns.to_owned(), s.def.to_owned(), Arc::new(vec![]))))
  });
  eprintln!("\nFailure: {failure}");
  if let Some(l) = fallback_location.as_deref() {
    eprintln!("  at {l}");
  }
  if let Some(h) = hint {
    eprintln!("\n{h}");
  }
  eprintln!("\ncall stack:");

  for s in &stack.0 {
    let is_macro = s.kind == StackKind::Macro;
    let stack_location = s.code.get_location().or_else(|| s.args.first().and_then(|arg| arg.get_location()));
    match stack_location {
      Some(l) => eprintln!("  {}/{}{} @ {l}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" }),
      None => eprintln!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" }),
    }
  }

  let mut stack_list = EdnListView::default();
  for s in &stack.0 {
    let mut args = EdnListView::default();
    for v in s.args.iter() {
      args.push(edn::calcit_to_edn(v)?);
    }
    let stack_location = s.code.get_location().or_else(|| s.args.first().and_then(|arg| arg.get_location()));
    let mut info_map = vec![
      (Edn::tag("def"), format!("{}/{}", s.ns, s.def).into()),
      (Edn::tag("code"), cirru::calcit_to_cirru(&s.code)?.into()),
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

  let content = cirru_edn::format(
    &Edn::map_from_iter([
      (Edn::tag("message"), failure.into()),
      (
        Edn::tag("hint"),
        match hint {
          Some(h) => h.into(),
          None => Edn::Nil,
        },
      ),
      (Edn::tag("stack"), stack_list.into()),
      (
        Edn::tag("location"),
        match fallback_location.as_deref() {
          Some(l) => l.into(),
          None => Edn::Nil,
        },
      ),
    ]),
    true,
  )?;
  let _ = fs::write(ERROR_SNAPSHOT, content);
  eprintln!("\nrun `cat {ERROR_SNAPSHOT}` to read stack details.");
  Ok(())
}

const ERROR_SNAPSHOT: &str = ".calcit-error.cirru";

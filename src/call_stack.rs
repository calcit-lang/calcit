use crate::calcit::Calcit;
use crate::calcit::NodeLocation;
use crate::data::cirru;
use crate::data::edn;
use crate::CalcitCompactList;
use cirru_edn::Edn;
use cirru_edn::EdnListView;
use im_ternary_tree::TernaryTreeList;
use std::fmt;
use std::fs;
use std::hash::Hash;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

lazy_static! {
  static ref TRACK_STACK: AtomicBool = AtomicBool::new(true);
}

/// control global stack usage
pub fn set_using_stack(b: bool) {
  TRACK_STACK.store(b, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Debug, PartialEq, Clone, Eq, Ord, PartialOrd, Hash)]
pub struct CalcitStack {
  pub ns: Arc<str>,
  pub def: Arc<str>,
  pub code: Calcit, // built in functions may not contain code
  pub args: StackArgsList,
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

pub type CallStackList = rpds::ListSync<CalcitStack>;

// TODO impl fmt

#[derive(Debug, PartialEq, Clone, Eq, Ord, PartialOrd, Hash)]
pub enum StackArgsList {
  List(TernaryTreeList<Arc<Calcit>>),
  Compact(CalcitCompactList),
}

impl Default for StackArgsList {
  fn default() -> StackArgsList {
    StackArgsList::Compact(TernaryTreeList::Empty)
  }
}

/// create new entry to the tree
pub fn extend_call_stack(
  stack: &CallStackList,
  ns: &str,
  def: &str,
  kind: StackKind,
  code: &Calcit,
  args: &StackArgsList,
) -> CallStackList {
  let b = TRACK_STACK.load(std::sync::atomic::Ordering::Relaxed);
  if b {
    stack.push_front(CalcitStack {
      ns: Arc::from(ns),
      def: Arc::from(def),
      code: code.to_owned(),
      args: args.to_owned(),
      kind,
    })
  } else {
    stack.to_owned()
  }
}

// show simplified version of stack
pub fn show_stack(stack: &CallStackList) {
  println!("\ncall stack:");
  for s in stack {
    let is_macro = s.kind == StackKind::Macro;
    println!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }
}

pub fn display_stack(failure: &str, stack: &CallStackList, location: Option<&Arc<NodeLocation>>) -> Result<(), String> {
  eprintln!("\nFailure: {failure}");
  eprintln!("\ncall stack:");

  for s in stack {
    let is_macro = s.kind == StackKind::Macro;
    eprintln!("  {}/{}{}", s.ns, s.def, if is_macro { "\t ~macro" } else { "" });
  }

  let mut stack_list = EdnListView::default();
  for s in stack {
    let mut args = EdnListView::default();
    match &s.args {
      StackArgsList::List(xs) => {
        for v in xs.iter() {
          args.push(edn::calcit_to_edn(v)?);
        }
      }
      StackArgsList::Compact(xs) => {
        for v in xs.iter() {
          args.push(edn::calcit_to_edn(v)?);
        }
      }
    }
    let info = Edn::map_from_iter([
      (Edn::tag("def"), format!("{}/{}", s.ns, s.def).into()),
      (Edn::tag("code"), cirru::calcit_to_cirru(&s.code)?.into()),
      (Edn::tag("args"), args.into()),
      (Edn::tag("kind"), Edn::tag(&s.kind.to_string())),
    ]);

    stack_list.push(info);
  }

  let content = cirru_edn::format(
    &Edn::map_from_iter([
      (Edn::tag("message"), failure.into()),
      (Edn::tag("stack"), stack_list.into()),
      (
        Edn::tag("location"),
        match location {
          Some(l) => (&**l).into(),
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

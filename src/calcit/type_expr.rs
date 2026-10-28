//! Compact quote type expressions.
//!
//! `quote $ Fn Number (-> String)` is a shorter source spelling of the
//! canonical type form `:: 'Fn $ {} (:args $ [] 'Number) (:return 'String)`.
//! The compact form is only surface syntax: it is desugared into the canonical
//! Cirru form before type parsing, so storage, the checker and every backend
//! keep seeing the canonical shape.
//!
//! Grammar (see issue #2433):
//!
//! ```text
//! type    := Name | Slot | (Name type+) | fn
//! fn      := (Fn param* [& type] (-> type) clause*)
//! clause  := (:generics Var+) | (:where (Var Trait+)+) | (:async) | (:features tag+)
//! ```
//!
//! Unqualified names and type variables start with an uppercase letter.
//! Qualified `ns/name` references keep any case. `*name` is a type slot.
//! Other lowercase leaves and symbol leaves are reserved for operators; `->`
//! and `&` are in use, `.`, `=` and `forall` are reserved.

use cirru_edn::{Edn, EdnMapView};
use cirru_parser::Cirru;

use crate::calcit::{CORE_NS, Calcit, CalcitImport, CalcitSyntax};

/// Operators reserved for future type-expression extensions.
const RESERVED_OPERATORS: [&str; 3] = [".", "=", "forall"];

/// Heads that mark an existing canonical or data form rather than a compact payload.
const CANONICAL_HEADS: [&str; 6] = ["::", "{}", "[]", "#{}", "quote", "%{}"];

/// Whether a quoted payload is written in the compact syntax. A compact payload
/// is a list whose head is a type name, such as `Fn`, `List` or `app/Box`.
pub(crate) fn is_compact_payload(payload: &Cirru) -> bool {
  let Cirru::List(items) = payload else {
    return false;
  };
  match items.first() {
    Some(Cirru::Leaf(head)) => !CANONICAL_HEADS.contains(&head.as_ref()) && is_type_name(head),
    _ => false,
  }
}

/// `hint-fn Async` marks a function async without stating its
/// signature; `(Async (:features …))` adds features. Returns the schema map, or
/// `None` when `payload` is not an `Async` marker.
pub(crate) fn desugar_async_hint(payload: &Cirru) -> Option<Result<Cirru, String>> {
  let clauses: &[Cirru] = match payload {
    Cirru::Leaf(name) if name.as_ref() == "Async" => &[],
    Cirru::List(items) if items.first() == Some(&Cirru::leaf("Async")) => &items[1..],
    _ => return None,
  };
  let mut map = vec![Cirru::leaf("{}"), Cirru::List(vec![Cirru::leaf(":async"), Cirru::leaf("true")])];
  let mut has_features = false;
  for clause in clauses {
    let Cirru::List(xs) = clause else {
      return Some(Err(format!(
        "`Async` clause `{}` must be written as a list such as `(:features :js-ffi)`",
        describe(clause)
      )));
    };
    if xs.first() != Some(&Cirru::leaf(":features")) {
      return Some(Err(format!(
        "unknown `Async` clause `{}`; only `(:features …)` is allowed",
        describe(clause)
      )));
    }
    if has_features {
      return Some(Err(String::from("duplicate `:features` clause in `Async`")));
    }
    has_features = true;
    if xs.len() < 2 {
      return Some(Err(String::from("`:features` needs at least one tag")));
    }
    let mut set = vec![Cirru::leaf("#{}")];
    for tag in &xs[1..] {
      match tag {
        Cirru::Leaf(name) if name.starts_with(':') && name.len() > 1 => set.push(tag.to_owned()),
        _ => return Some(Err(format!("feature `{}` must be a tag such as `:js-ffi`", describe(tag)))),
      }
    }
    map.push(Cirru::List(vec![Cirru::leaf(":features"), Cirru::List(set)]));
  }
  Some(Ok(Cirru::List(map)))
}

/// Desugar a compact payload (the item inside `quote`) into canonical Cirru.
pub(crate) fn desugar_type_expr(payload: &Cirru) -> Result<Cirru, String> {
  desugar_type(payload, &[])
}

/// Expand `(quote <compact payload>)` written in source code into the canonical
/// code form. Returns `None` when `form` is not a quoted compact payload, so
/// existing canonical forms and plain `'T` type variables keep their path.
pub(crate) fn desugar_quoted_form(form: &Calcit) -> Option<Result<Calcit, String>> {
  let Calcit::List(list) = form else {
    return None;
  };
  let is_quote_head = match list.first()? {
    Calcit::Syntax(CalcitSyntax::Quote, _) => true,
    Calcit::Symbol { sym, .. } => sym.as_ref() == "quote",
    Calcit::Import(CalcitImport { ns, def, .. }) => &**ns == CORE_NS && &**def == "quote",
    _ => false,
  };
  if !is_quote_head || list.len() != 2 {
    return None;
  }
  let payload = list.get(1)?;
  if !matches!(payload, Calcit::List(_)) {
    return None;
  }
  let cirru = crate::data::cirru::calcit_to_cirru(payload).ok()?;
  // A quoted list in a type position only means a compact expression, so a
  // misspelled head such as `list` is reported instead of parsed as a List value.
  let Cirru::List(items) = &cirru else {
    return None;
  };
  match items.first() {
    Some(Cirru::Leaf(head)) if !CANONICAL_HEADS.contains(&head.as_ref()) && !head.starts_with([':', '\'', '&']) => {}
    _ => return None,
  }
  let (ns, def, coord) = match first_symbol(payload) {
    Some(Calcit::Symbol { info, location, .. }) => (
      info.at_ns.to_string(),
      info.at_def.to_string(),
      location.as_deref().cloned().unwrap_or_default(),
    ),
    _ => (String::new(), String::new(), vec![]),
  };
  Some(desugar_type_expr(&cirru).and_then(|canonical| crate::data::cirru::code_to_calcit(&canonical, &ns, &def, coord)))
}

/// A type application written without its `quote`, such as `(List Number)`
/// in `assert-type xs (List Number)`. Code reads it as a call, so it would
/// otherwise reach the checker as an unknown form and widen to `Dynamic`.
/// Returns the compact payload text so the diagnostic can show the quoted form.
pub(crate) fn unquoted_compact_form(form: &Calcit) -> Option<String> {
  let Calcit::List(list) = form else {
    return None;
  };
  let Some(Calcit::Symbol { sym, .. }) = list.first() else {
    return None;
  };
  let name = sym.rsplit_once('/').map_or(sym.as_ref(), |(_, def)| def);
  if !is_uppercase_name(name) {
    return None;
  }
  let cirru = crate::data::cirru::calcit_to_cirru(form).ok()?;
  Some(describe(&cirru))
}

/// Location of the first located node inside `form`; a quoted list carries
/// no location of its own.
pub(crate) fn first_location(form: &Calcit) -> Option<crate::calcit::NodeLocation> {
  match form {
    Calcit::List(xs) => xs.iter().find_map(|x| x.get_location().or_else(|| first_location(x))),
    _ => form.get_location(),
  }
}

fn first_symbol(form: &Calcit) -> Option<&Calcit> {
  match form {
    Calcit::Symbol { .. } => Some(form),
    Calcit::List(xs) => xs.iter().find_map(first_symbol),
    _ => None,
  }
}

fn is_qualified_name(name: &str) -> bool {
  match name.split_once('/') {
    Some((ns, def)) => !ns.is_empty() && !def.is_empty() && !def.contains('/'),
    None => false,
  }
}

fn is_uppercase_name(name: &str) -> bool {
  name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
    && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '?' | '!'))
}

fn is_type_name(name: &str) -> bool {
  is_uppercase_name(name) || is_qualified_name(name)
}

fn quoted(name: &str) -> Cirru {
  Cirru::Leaf(format!("'{name}").into())
}

fn leaf_text(node: &Cirru) -> Option<&str> {
  match node {
    Cirru::Leaf(s) => Some(s.as_ref()),
    Cirru::List(_) => None,
  }
}

fn describe(node: &Cirru) -> String {
  match node {
    Cirru::Leaf(s) => s.to_string(),
    Cirru::List(_) => cirru_parser::format_expr_one_liner(node).unwrap_or_else(|_| format!("{node:?}")),
  }
}

fn desugar_leaf(name: &str) -> Result<Cirru, String> {
  // Type slots keep the `'*slot` spelling of the canonical form, which is
  // also what the Cirru EDN reader accepts inside a Snapshot schema.
  if let Some(slot) = name.strip_prefix('*')
    && !slot.is_empty()
  {
    return Ok(quoted(name));
  }
  if is_type_name(name) {
    return Ok(quoted(name));
  }
  if name.starts_with('\'') {
    return Err(format!(
      "`{name}` uses a quote prefix inside a compact type expression; write `{}` without `'`",
      name.trim_start_matches('\'')
    ));
  }
  if name.starts_with(':') {
    return Err(format!(
      "tag `{name}` is not a type in a compact type expression; use a type name that starts with an uppercase letter"
    ));
  }
  if name == "->" {
    return Err(String::from(
      "`->` is only valid as the return clause of `Fn`, written as `(-> ReturnType)`",
    ));
  }
  if name == "&" {
    return Err(String::from("`&` is only valid among `Fn` parameters, before the rest type"));
  }
  if RESERVED_OPERATORS.contains(&name) {
    return Err(format!("`{name}` is reserved for a future type operator"));
  }
  Err(format!(
    "`{name}` is not a type name; unqualified type names and type variables start with an uppercase letter, qualified names use `ns/name`"
  ))
}

fn desugar_type(node: &Cirru, scope: &[String]) -> Result<Cirru, String> {
  match node {
    Cirru::Leaf(name) => desugar_leaf(name),
    Cirru::List(items) => {
      let Some(head) = items.first() else {
        return Err(String::from("empty list is not a type expression"));
      };
      let Some(head) = leaf_text(head) else {
        return Err(format!("type application head must be a type name, got `{}`", describe(node)));
      };
      match head {
        "Fn" => desugar_fn(&items[1..], scope, Kind::Fn),
        "Macro" => desugar_fn(&items[1..], scope, Kind::Macro),
        _ if is_type_name(head) => {
          if items.len() == 1 {
            return Err(format!(
              "type application `({head})` has no arguments; write `{head}` without parentheses"
            ));
          }
          let mut out = vec![Cirru::leaf("::"), quoted(head)];
          for arg in &items[1..] {
            out.push(desugar_type(arg, scope)?);
          }
          Ok(Cirru::List(out))
        }
        _ => Err(
          desugar_leaf(head)
            .err()
            .unwrap_or_else(|| format!("`{head}` cannot head a type application")),
        ),
      }
    }
  }
}

/// `Fn` and `Macro` share one grammar: positional inputs, `& T`, one `(-> T)`
/// and tag clauses. They differ in the clauses they allow and the map they build.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
  Fn,
  Macro,
}

impl Kind {
  fn name(self) -> &'static str {
    match self {
      Kind::Fn => "Fn",
      Kind::Macro => "Macro",
    }
  }
}

#[derive(Default)]
struct FnParts {
  optional: Option<Vec<Cirru>>,
  capabilities: Option<Vec<Cirru>>,
  args: Vec<Cirru>,
  rest: Option<Cirru>,
  ret: Option<Cirru>,
  generics: Option<Vec<String>>,
  where_bounds: Option<Vec<(String, Vec<String>)>>,
  is_async: bool,
  features: Option<Vec<Cirru>>,
}

/// Clause heads come either from `->` or from a tag. Anything else is a parameter.
fn clause_head(node: &Cirru) -> Option<&str> {
  let Cirru::List(items) = node else {
    return None;
  };
  let head = leaf_text(items.first()?)?;
  if head == "->" || head.starts_with(':') { Some(head) } else { None }
}

fn desugar_fn(items: &[Cirru], outer_scope: &[String], kind: Kind) -> Result<Cirru, String> {
  let kind_name = kind.name();
  // Generics are collected first so parameters and the return type can see them.
  let mut parts = FnParts::default();
  for item in items {
    let Some(head) = clause_head(item) else { continue };
    let Cirru::List(clause) = item else { unreachable!() };
    let body = &clause[1..];
    match head {
      ":generics" => {
        if parts.generics.is_some() {
          return Err(format!("duplicate `:generics` clause in `{kind_name}`"));
        }
        if body.is_empty() {
          return Err(String::from("`:generics` needs at least one type variable"));
        }
        let mut names = vec![];
        for var in body {
          let name = match var {
            Cirru::Leaf(name) if is_uppercase_name(name) => name.to_string(),
            Cirru::List(_) => {
              return Err(format!(
                "`{}` in `:generics` is reserved for future generic options such as `:arity`; list plain type variables",
                describe(var)
              ));
            }
            _ => {
              return Err(format!(
                "type variable `{}` must be an unqualified name that starts with an uppercase letter",
                describe(var)
              ));
            }
          };
          if outer_scope.contains(&name) {
            return Err(format!("type variable `{name}` is already declared by an outer `Fn`"));
          }
          if names.contains(&name) {
            return Err(format!("type variable `{name}` is declared twice in `:generics`"));
          }
          names.push(name);
        }
        parts.generics = Some(names);
      }
      ":where" => {
        if parts.where_bounds.is_some() {
          return Err(format!("duplicate `:where` clause in `{kind_name}`"));
        }
        if body.is_empty() {
          return Err(String::from("`:where` needs at least one `(Var Trait…)` bound"));
        }
        let mut bounds = vec![];
        for bound in body {
          let Cirru::List(xs) = bound else {
            return Err(format!("`:where` bound `{}` must be written as `(Var Trait…)`", describe(bound)));
          };
          let subject = match xs.first() {
            Some(Cirru::Leaf(name)) if is_uppercase_name(name) => name.to_string(),
            _ => {
              return Err(format!(
                "`:where` bound `{}` must start with a type variable; other subjects are reserved",
                describe(bound)
              ));
            }
          };
          if xs.len() < 2 {
            return Err(format!("`:where` bound for `{subject}` lists no traits"));
          }
          let mut traits = vec![];
          for t in &xs[1..] {
            match t {
              Cirru::Leaf(name) if is_type_name(name) => traits.push(name.to_string()),
              Cirru::List(_) => {
                return Err(format!(
                  "parameterized trait bound `{}` is reserved for a future extension",
                  describe(t)
                ));
              }
              _ => return Err(format!("trait `{}` must be a trait name", describe(t))),
            }
          }
          bounds.push((subject, traits));
        }
        parts.where_bounds = Some(bounds);
      }
      ":async" if kind == Kind::Fn => {
        if parts.is_async {
          return Err(String::from("duplicate `:async` clause in `Fn`"));
        }
        if !body.is_empty() {
          return Err(String::from("`:async` takes no arguments"));
        }
        parts.is_async = true;
      }
      ":features" => {
        if parts.features.is_some() {
          return Err(format!("duplicate `:features` clause in `{kind_name}`"));
        }
        if body.is_empty() {
          return Err(String::from("`:features` needs at least one tag"));
        }
        let mut tags = vec![];
        for tag in body {
          match tag {
            Cirru::Leaf(name) if name.starts_with(':') && name.len() > 1 => tags.push(tag.to_owned()),
            _ => return Err(format!("feature `{}` must be a tag such as `:js-ffi`", describe(tag))),
          }
        }
        parts.features = Some(tags);
      }
      ":optional" if kind == Kind::Macro => {
        if parts.optional.is_some() {
          return Err(String::from("duplicate `:optional` clause in `Macro`"));
        }
        if body.is_empty() {
          return Err(String::from("`:optional` needs at least one input contract"));
        }
        // Resolved after generics are in scope, below.
        parts.optional = Some(body.to_vec());
      }
      ":capabilities" if kind == Kind::Macro => {
        if parts.capabilities.is_some() {
          return Err(String::from("duplicate `:capabilities` clause in `Macro`"));
        }
        let mut tags = vec![];
        for tag in body {
          match tag {
            Cirru::Leaf(name) if name.starts_with(':') && name.len() > 1 => tags.push(tag.to_owned()),
            _ => return Err(format!("capability `{}` must be a tag such as `:platform-read`", describe(tag))),
          }
        }
        parts.capabilities = Some(tags);
      }
      "->" => {}
      _ => {
        return Err(match kind {
          Kind::Fn => format!("unknown `Fn` clause `{head}`; expected `(-> T)`, `:generics`, `:where`, `:async` or `:features`"),
          Kind::Macro => format!(
            "unknown `Macro` clause `{head}`; expected `(-> T)`, `:optional`, `:capabilities`, `:generics`, `:where` or `:features`"
          ),
        });
      }
    }
  }

  let mut scope = outer_scope.to_vec();
  if let Some(names) = &parts.generics {
    scope.extend(names.iter().cloned());
  }
  if let Some(bounds) = &parts.where_bounds {
    let declared = parts.generics.as_deref().unwrap_or(&[]);
    for (subject, _) in bounds {
      if !declared.contains(subject) {
        return Err(format!(
          "`:where` constrains `{subject}`, which is not declared in this `{kind_name}`'s `:generics`"
        ));
      }
    }
  }

  let mut seen_clause = false;
  let mut idx = 0;
  while idx < items.len() {
    let item = &items[idx];
    if let Some(head) = clause_head(item) {
      seen_clause = true;
      if head == "->" {
        let Cirru::List(clause) = item else { unreachable!() };
        if parts.ret.is_some() {
          return Err(format!("`{kind_name}` has more than one `(-> T)` return clause"));
        }
        if clause.len() != 2 {
          return Err(format!("return clause `{}` must hold exactly one type", describe(item)));
        }
        parts.ret = Some(desugar_type(&clause[1], &scope)?);
      }
      idx += 1;
      continue;
    }
    if let Some(tag) = leaf_text(item).filter(|name| name.starts_with(':')) {
      return Err(format!(
        "`{kind_name}` clause `{tag}` must be written as a list, such as `({tag})` on its own line"
      ));
    }
    if seen_clause {
      return Err(format!(
        "`{kind_name}` parameter `{}` appears after a clause; write parameters first, then `(-> T)` and other clauses",
        describe(item)
      ));
    }
    if parts.rest.is_some() {
      return Err(format!("`{kind_name}` parameter `{}` appears after the rest type", describe(item)));
    }
    if leaf_text(item) == Some("&") {
      let Some(rest) = items.get(idx + 1).filter(|next| clause_head(next).is_none()) else {
        return Err(String::from("`&` must be followed by the rest type"));
      };
      parts.rest = Some(desugar_type(rest, &scope)?);
      idx += 2;
      continue;
    }
    if leaf_text(item) == Some("->") {
      return Err(String::from(
        "a bare `->` is not part of the compact syntax; write the return type as a clause `(-> T)`",
      ));
    }
    parts.args.push(desugar_type(item, &scope)?);
    idx += 1;
  }

  let Some(ret) = parts.ret else {
    return Err(match kind {
      Kind::Fn => String::from("`Fn` needs a return clause `(-> T)`; write `(-> Dynamic)` when the result is intentionally dynamic"),
      Kind::Macro => String::from("`Macro` needs an expansion clause `(-> T)`, such as `(-> (Expr Dynamic))`"),
    });
  };

  let mut args = vec![Cirru::leaf("[]")];
  args.extend(parts.args);
  let mut map = vec![Cirru::leaf("{}")];
  match kind {
    Kind::Fn => {
      map.push(Cirru::List(vec![Cirru::leaf(":args"), Cirru::List(args)]));
      map.push(Cirru::List(vec![Cirru::leaf(":return"), ret]));
    }
    Kind::Macro => {
      map.push(Cirru::List(vec![Cirru::leaf(":required"), Cirru::List(args)]));
      if let Some(optional) = &parts.optional {
        let mut list = vec![Cirru::leaf("[]")];
        for item in optional {
          list.push(desugar_type(item, &scope)?);
        }
        map.push(Cirru::List(vec![Cirru::leaf(":optional"), Cirru::List(list)]));
      }
      map.push(Cirru::List(vec![Cirru::leaf(":expansion"), ret]));
      let mut set = vec![Cirru::leaf("#{}")];
      set.extend(parts.capabilities.take().unwrap_or_default());
      map.push(Cirru::List(vec![Cirru::leaf(":capabilities"), Cirru::List(set)]));
    }
  }
  if let Some(names) = parts.generics {
    let mut list = vec![Cirru::leaf("[]")];
    list.extend(names.iter().map(|name| quoted(name)));
    map.push(Cirru::List(vec![Cirru::leaf(":generics"), Cirru::List(list)]));
  }
  if let Some(bounds) = parts.where_bounds {
    let mut bound_map = vec![Cirru::leaf("{}")];
    for (subject, traits) in bounds {
      let value = if traits.len() == 1 {
        quoted(&traits[0])
      } else {
        let mut list = vec![Cirru::leaf("[]")];
        list.extend(traits.iter().map(|name| quoted(name)));
        Cirru::List(list)
      };
      bound_map.push(Cirru::List(vec![quoted(&subject), value]));
    }
    map.push(Cirru::List(vec![Cirru::leaf(":where"), Cirru::List(bound_map)]));
  }
  if let Some(rest) = parts.rest {
    map.push(Cirru::List(vec![Cirru::leaf(":rest"), rest]));
  }
  if parts.is_async {
    map.push(Cirru::List(vec![Cirru::leaf(":async"), Cirru::leaf("true")]));
  }
  if let Some(tags) = parts.features {
    let mut set = vec![Cirru::leaf("#{}")];
    set.extend(tags);
    map.push(Cirru::List(vec![Cirru::leaf(":features"), Cirru::List(set)]));
  }
  Ok(Cirru::List(vec![Cirru::leaf("::"), quoted(kind_name), Cirru::List(map)]))
}

/// Print canonical type EDN in the compact syntax, the inverse of
/// `desugar_type_expr`. Shapes without a compact spelling return `Err`.
pub(crate) fn print_type_compact(edn: &Edn) -> Result<Cirru, String> {
  match edn {
    Edn::Symbol(name) => {
      let leaf = Cirru::Leaf(name.clone());
      desugar_type_expr(&leaf)?;
      Ok(leaf)
    }
    Edn::Enum(view) if view.type_name.is_none() && view.variant.as_ref() == "Fn" => match view.extra.as_slice() {
      [Edn::Map(map)] => print_fn(map),
      _ => Err(format!("unexpected Fn payload {edn}")),
    },
    Edn::Enum(view) if view.type_name.is_none() && view.variant.as_ref() == "Macro" => match view.extra.as_slice() {
      [Edn::Map(map)] => print_macro(map),
      _ => Err(format!("unexpected Macro payload {edn}")),
    },
    Edn::Enum(view) if view.type_name.is_none() => {
      let mut out = vec![Cirru::Leaf(view.variant.clone())];
      for arg in &view.extra {
        out.push(print_type_compact(arg)?);
      }
      Ok(Cirru::List(out))
    }
    Edn::List(xs) if matches!(xs.0.first(), Some(Edn::Symbol(head)) if head.as_ref() == "::") => {
      let mut out = vec![];
      for item in xs.0.iter().skip(1) {
        out.push(print_type_compact(item)?);
      }
      Ok(Cirru::List(out))
    }
    _ => Err(format!("no compact spelling for {edn}")),
  }
}

fn print_macro(map: &EdnMapView) -> Result<Cirru, String> {
  let get = |key: &str| map.0.get(&Edn::tag(key));
  let mut out = vec![Cirru::leaf("Macro")];
  if let Some(Edn::List(args)) = get("required") {
    for arg in &args.0 {
      out.push(print_type_compact(arg)?);
    }
  }
  if let Some(rest) = get("rest") {
    out.push(Cirru::leaf("&"));
    out.push(print_type_compact(rest)?);
  }
  let expansion = get("expansion").ok_or_else(|| String::from("macro without expansion"))?;
  out.push(Cirru::List(vec![Cirru::leaf("->"), print_type_compact(expansion)?]));
  if let Some(Edn::List(optional)) = get("optional") {
    let mut clause = vec![Cirru::leaf(":optional")];
    for item in &optional.0 {
      clause.push(print_type_compact(item)?);
    }
    out.push(Cirru::List(clause));
  }
  if let Some(Edn::Set(caps)) = get("capabilities")
    && !caps.0.is_empty()
  {
    let mut clause = vec![Cirru::leaf(":capabilities")];
    let mut tags: Vec<_> = caps.0.iter().map(|tag| Cirru::Leaf(tag.to_string().into())).collect();
    tags.sort();
    clause.extend(tags);
    out.push(Cirru::List(clause));
  }
  print_shared_clauses(&get, &mut out)?;
  Ok(Cirru::List(out))
}

fn print_fn(map: &EdnMapView) -> Result<Cirru, String> {
  let get = |key: &str| map.0.get(&Edn::tag(key));
  if get("kind").is_some() {
    return Err(String::from("macro schema"));
  }
  let mut out = vec![Cirru::leaf("Fn")];
  if let Some(Edn::List(args)) = get("args") {
    for arg in &args.0 {
      out.push(print_type_compact(arg)?);
    }
  }
  if let Some(rest) = get("rest") {
    out.push(Cirru::leaf("&"));
    out.push(print_type_compact(rest)?);
  }
  let ret = get("return")
    .map(print_type_compact)
    .transpose()?
    .unwrap_or_else(|| Cirru::leaf("Dynamic"));
  out.push(Cirru::List(vec![Cirru::leaf("->"), ret]));
  print_shared_clauses(&get, &mut out)?;
  if matches!(get("async"), Some(Edn::Bool(true))) {
    out.push(Cirru::List(vec![Cirru::leaf(":async")]));
  }
  Ok(Cirru::List(out))
}

fn print_shared_clauses<'a>(get: &dyn Fn(&str) -> Option<&'a Edn>, out: &mut Vec<Cirru>) -> Result<(), String> {
  if let Some(Edn::List(generics)) = get("generics") {
    let mut clause = vec![Cirru::leaf(":generics")];
    clause.extend(generics.0.iter().map(|g| match g {
      Edn::Symbol(name) => Cirru::Leaf(name.clone()),
      other => Cirru::Leaf(other.to_string().into()),
    }));
    out.push(Cirru::List(clause));
  }
  if let Some(Edn::Map(bounds)) = get("where") {
    let mut clause = vec![Cirru::leaf(":where")];
    let mut pairs: Vec<_> = bounds.0.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    for (subject, traits) in pairs {
      let Edn::Symbol(subject) = subject else {
        return Err(String::from("where subject"));
      };
      let mut bound = vec![Cirru::Leaf(subject.clone())];
      match traits {
        Edn::Symbol(name) => bound.push(Cirru::Leaf(name.clone())),
        Edn::List(names) => {
          for name in &names.0 {
            let Edn::Symbol(name) = name else {
              return Err(String::from("where trait"));
            };
            bound.push(Cirru::Leaf(name.clone()));
          }
        }
        _ => return Err(String::from("where traits")),
      }
      clause.push(Cirru::List(bound));
    }
    out.push(Cirru::List(clause));
  }
  if let Some(Edn::Set(features)) = get("features") {
    let mut clause = vec![Cirru::leaf(":features")];
    let mut tags: Vec<_> = features.0.iter().map(|tag| Cirru::Leaf(tag.to_string().into())).collect();
    tags.sort();
    clause.extend(tags);
    out.push(Cirru::List(clause));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_one(code: &str) -> Cirru {
    let mut nodes = cirru_parser::parse(code).expect("parse cirru");
    assert_eq!(nodes.len(), 1, "expected one expression in {code}");
    nodes.remove(0)
  }

  /// `code` is the payload as written after `quote $`, using Cirru indentation.
  fn desugar_text(code: &str) -> Result<String, String> {
    fn lisp(node: &Cirru) -> String {
      match node {
        Cirru::Leaf(s) => s.to_string(),
        Cirru::List(xs) => format!("({})", xs.iter().map(lisp).collect::<Vec<_>>().join(" ")),
      }
    }
    Ok(lisp(&desugar_type_expr(&parse_one(code))?))
  }

  #[test]
  fn desugars_applications_and_names() {
    assert_eq!(desugar_text("List Number").unwrap(), "(:: 'List 'Number)");
    assert_eq!(
      desugar_text("Map String (List app.schema/Item)").unwrap(),
      "(:: 'Map 'String (:: 'List 'app.schema/Item))"
    );
    assert_eq!(desugar_text("Option *item").unwrap(), "(:: 'Option '*item)");
    assert_eq!(
      desugar_text("recollect.schema/change-op Number").unwrap(),
      "(:: 'recollect.schema/change-op 'Number)"
    );
  }

  #[test]
  fn desugars_fn_with_clauses() {
    let code = "Fn C\n  Fn T $ -> U\n  -> Dynamic\n  :generics C T U\n  :where $ C Mappable";
    assert_eq!(
      desugar_text(code).unwrap(),
      "(:: 'Fn ({} (:args ([] 'C (:: 'Fn ({} (:args ([] 'T)) (:return 'U))))) (:return 'Dynamic) (:generics ([] 'C 'T 'U)) (:where ({} ('C 'Mappable)))))"
    );
    assert_eq!(
      desugar_text("Fn Number & Number (-> Number)").unwrap(),
      "(:: 'Fn ({} (:args ([] 'Number)) (:return 'Number) (:rest 'Number)))"
    );
    let code = "Fn String\n  -> $ Result String String\n  :async\n  :features :js-ffi\n  :where (T Sliceable Countable)\n  :generics T";
    assert_eq!(
      desugar_text(code).unwrap(),
      "(:: 'Fn ({} (:args ([] 'String)) (:return (:: 'Result 'String 'String)) (:generics ([] 'T)) (:where ({} ('T ([] 'Sliceable 'Countable)))) (:async true) (:features (#{} :js-ffi))))"
    );
  }

  #[test]
  fn desugars_macro_contracts() {
    assert_eq!(
      desugar_text("Macro (Expr Dynamic) & SyntaxList (-> (Expr Struct))").unwrap(),
      "(:: 'Macro ({} (:required ([] (:: 'Expr 'Dynamic))) (:expansion (:: 'Expr 'Struct)) (:capabilities (#{})) (:rest 'SyntaxList)))"
    );
    let code = "Macro (Expr T)\n  -> $ Expr T\n  :capabilities :platform-read\n  :optional (Expr Dynamic)\n  :generics T";
    assert_eq!(
      desugar_text(code).unwrap(),
      "(:: 'Macro ({} (:required ([] (:: 'Expr 'T))) (:optional ([] (:: 'Expr 'Dynamic))) (:expansion (:: 'Expr 'T)) (:capabilities (#{} :platform-read)) (:generics ([] 'T))))"
    );
  }

  #[test]
  fn rejects_invalid_shapes() {
    let cases = [
      ("Fn Number", "needs a return clause"),
      ("Fn Number -> Number", "bare `->`"),
      ("Fn (-> Number) Number", "after a clause"),
      ("Fn Number (-> Number) (-> String)", "more than one"),
      ("Fn & Number String (-> Unit)", "after the rest type"),
      ("Fn (-> Number) :async", "must be written as a list"),
      ("Fn (-> Number) (:effects :fs)", "unknown `Fn` clause `:effects`"),
      ("Fn T (-> T) (:generics t)", "uppercase"),
      ("Fn T (-> T) (:generics (F :arity 1))", "reserved"),
      ("Fn T (-> T) (:generics T) (:where (T (Into String)))", "parameterized trait"),
      ("Fn T (-> T) (:where (T Eq))", "not declared"),
      ("Fn (Fn T (-> T) (:generics T)) (-> Unit) (:generics T)", "outer `Fn`"),
      ("List number", "not a type name"),
      ("List 'Number", "quote prefix"),
      ("List :number", "tag `:number`"),
      ("List (. C Item)", "reserved"),
      ("List (Number)", "has no arguments"),
      ("Macro Syntax", "expansion clause"),
      ("Macro Syntax (-> Unit) (:async)", "unknown `Macro` clause `:async`"),
      ("Fn Syntax (-> Unit) (:capabilities :fs)", "unknown `Fn` clause `:capabilities`"),
      ("Macro (-> Unit) (:capabilities fs)", "must be a tag"),
    ];
    for (code, expected) in cases {
      let err = desugar_text(code).expect_err(code);
      assert!(err.contains(expected), "{code}: expected `{expected}` in `{err}`");
    }
  }

  #[test]
  fn detects_only_compact_payloads() {
    assert!(is_compact_payload(&parse_one("Fn Number (-> Number)")));
    assert!(is_compact_payload(&parse_one("app.schema/Box Number")));
    assert!(!is_compact_payload(&parse_one(":: 'List 'Number")));
    assert!(!is_compact_payload(&parse_one("{} (:args $ [])")));
    assert!(!is_compact_payload(&Cirru::leaf("Number")));
    assert!(!is_compact_payload(&parse_one("f x")));
  }

  #[test]
  fn core_schemas_round_trip_through_compact_syntax() {
    let core = crate::load_core_snapshot().expect("embedded core");
    let mut checked = 0;
    let mut skipped = vec![];
    let mut failures = vec![];
    for file in core.files.values() {
      for (def, entry) in &file.defs {
        if matches!(entry.schema.as_ref(), crate::calcit::CalcitTypeAnnotation::Dynamic) {
          continue;
        }
        let edn = entry.schema.to_type_edn();
        let compact = match print_type_compact(&edn) {
          Ok(compact) => compact,
          Err(reason) => {
            skipped.push(format!("{def}: {reason}"));
            continue;
          }
        };
        checked += 1;
        match crate::snapshot::parse_schema_annotation_for_write(&compact) {
          Ok(parsed) if parsed.to_type_edn() == edn => {}
          Ok(parsed) => failures.push(format!(
            "{def}: {} reparsed as {} (expected {edn})",
            cirru_parser::format_expr_one_liner(&compact).unwrap_or_default(),
            parsed.to_type_edn()
          )),
          Err(e) => failures.push(format!(
            "{def}: {} failed: {e}",
            cirru_parser::format_expr_one_liner(&compact).unwrap_or_default()
          )),
        }
      }
    }
    eprintln!("compact round trip: {checked} checked, {} skipped", skipped.len());
    assert!(checked > 500, "expected most core schemas to be checked, got {checked}");
    assert!(
      failures.is_empty(),
      "{} round-trip failures:\n{}",
      failures.len(),
      failures.join("\n")
    );
    // Only non-type leaves may lack a compact spelling.
    for reason in &skipped {
      assert!(reason.contains("no compact spelling"), "unexpected skip {reason}");
    }
  }
}

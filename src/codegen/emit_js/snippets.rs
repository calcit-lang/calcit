use crate::builtins::meta::js_gensym;

use super::runtime::get_proc_prefix;

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn tmpl_try(err_var: String, body: String, handler: String, return_code: &str) -> String {
  let message_var = js_gensym("errMessage");
  format!(
    "try {{
  {body}
}} catch ({err_var}) {{
  let {message_var} = {err_var} instanceof Error ? {err_var}.message : `${{{err_var}}}`;
  {return_code} ({handler})({message_var})
}}",
  )
}

pub fn tmpl_result_try(call_code: &str, ok_code: &str, err_code: &str) -> String {
  let err_var = js_gensym("err");
  let message_var = js_gensym("errMessage");
  format!(
    "(function __fn__(){{
  try {{
    return {ok_code}({call_code});
  }} catch ({err_var}) {{
    let {message_var} = {err_var} instanceof Error ? {err_var}.message : `${{{err_var}}}`;
    return {err_code}({message_var});
  }}
}})()"
  )
}

pub fn tmpl_fn_wrapper(body: String) -> String {
  format!(
    "(function __fn__(){{
  {body}
}})()"
  )
}

pub fn tmpl_args_fewer_than(name: &str, args_count: usize, at_ns: &str) -> String {
  if args_count == 0 {
    return String::new();
  }
  let proc_ns = get_proc_prefix(at_ns);
  let escaped_name = name.escape_default();
  format!("if (arguments.length < {args_count}) throw {proc_ns}_args_fewer_throw('{escaped_name}', {args_count}, arguments.length);")
}

pub fn tmpl_args_between(name: &str, a: usize, b: usize, at_ns: &str) -> String {
  let proc_ns = get_proc_prefix(at_ns);
  let escaped_name = name.escape_default();
  format!(
    "if (arguments.length < {a}) throw {proc_ns}_args_between_throw('{escaped_name}', {a}, {b}, arguments.length);
if (arguments.length > {b}) throw {proc_ns}_args_between_throw('{escaped_name}', {a}, {b}, arguments.length);"
  )
}

pub fn tmpl_args_exact(name: &str, args_count: usize, at_ns: &str) -> String {
  let proc_ns = get_proc_prefix(at_ns);
  let escaped_name = name.escape_default();
  format!("if (arguments.length !== {args_count}) throw {proc_ns}_args_throw('{escaped_name}', {args_count}, arguments.length);")
}

pub struct RecurPrefixes {
  pub var_prefix: String,
  pub async_prefix: String,
  pub return_mark: String,
}

pub fn tmpl_tail_recursion(
  name: String,
  args_code: String,
  check_args: String,
  spreading_code: String,
  recur_assign_code_template: String,
  body0: String,
  prefixes: RecurPrefixes,
) -> String {
  let var_prefix = prefixes.var_prefix;
  let return_mark = &prefixes.return_mark;
  let async_prefix = &prefixes.async_prefix;
  let ret_var = js_gensym("ret");
  let times_var = js_gensym("times");
  let body = body0.replace(return_mark, &ret_var); // dirty trick for injection
  let recur_assign_code = recur_assign_code_template.replace("{ret_var}", &ret_var);

  let check_recur_args = check_args.replace("arguments.length", &format!("{ret_var}.args.length"));

  format!(
    "{async_prefix}function {name}({args_code}) {{
  {check_args}
  {spreading_code}
  let {times_var} = 0;
  while(true) {{ /* Tail Recursion */
    let {ret_var} = null;
    if ((({times_var} & 1023) === 0) && {times_var} > 10000000) throw new Error('tail recursion not finished after 10M iterations');
    {body}
    if ({ret_var} instanceof {var_prefix}CalcitRecur) {{
      {check_recur_args}
      {recur_assign_code}
      {spreading_code}
      {times_var} += 1;
      continue;
    }} else {{
      return {ret_var};
    }}
  }}
}}
"
  )
}

pub fn tmpl_import_procs(name: String) -> String {
  format!(
    "
import {{init_tags, arrayToList, listToArray, CalcitSliceList, CalcitSymbol, CalcitRecur}} from {name};
import * as $procs from {name};
export * from {name};
",
  )
}

pub fn tmpl_classes_registering() -> String {
  format!(
    "
$procs.register_calcit_builtin_impls({{
  list: _$n_core_list_impls,
  map: _$n_core_map_impls,
  number: _$n_core_number_impls,
  set: _$n_core_set_impls,
  string: _$n_core_string_impls,
  fn: _$n_core_fn_impls,
  enum: _$n_core_enum_impls,
  struct: _$n_core_struct_impls,
  scalar: _$n_core_scalar_impls,
  ref: _$n_core_ref_impls,
}});

let runtimeVersion = $procs.calcit_version;
let cli_version = '{CALCIT_VERSION}';

if (runtimeVersion !== cli_version) {{
  console.warn(`[Warning] versions mismatch, CLI using: ${{cli_version}}, runtime using: ${{runtimeVersion}}`)
}}
"
  )
}

pub fn tmpl_tags_init(arr: &str, prefix: &str) -> String {
  format!("\nconst _t_ = {prefix}init_tags({arr});\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn import_procs_includes_init_tags() {
    let code = tmpl_import_procs("\"@calcit/procs\"".to_owned());
    assert!(code.contains("init_tags"));
  }

  #[test]
  fn tags_init_uses_runtime_helper() {
    let code = tmpl_tags_init("[\"a\",\"b\"]", "$clt.");
    assert!(code.contains("const _t_ = $clt.init_tags([\"a\",\"b\"]);"));
  }

  #[test]
  fn args_fewer_than_zero_arity_returns_empty() {
    let code = tmpl_args_fewer_than("f%", 0, "app.main");
    assert!(code.is_empty());
  }

  #[test]
  fn try_handler_receives_a_string_message() {
    let code = tmpl_try("caught".to_owned(), "return risky()".to_owned(), "handler".to_owned(), "return");
    assert!(code.contains("caught instanceof Error ? caught.message"));
    assert!(code.contains("(handler)(errMessage"));
    assert!(!code.contains("(handler)(caught)"));
  }

  #[test]
  fn result_try_wraps_success_and_normalizes_failure() {
    let code = tmpl_result_try("decode()", "ok", "err");
    assert!(code.contains("return ok(decode());"));
    assert!(code.contains("instanceof Error"));
    assert!(code.contains("return err(errMessage"));
  }

  #[test]
  fn tail_recursion_uses_periodic_watchdog_and_replaced_assign_template() {
    let code = tmpl_tail_recursion(
      "f".to_owned(),
      "a, b".to_owned(),
      "if (arguments.length !== 2) throw new Error('x');".to_owned(),
      "".to_owned(),
      "a = {ret_var}.args[0];\nb = {ret_var}.args[1];".to_owned(),
      "%%ret%% = 1;".to_owned(),
      RecurPrefixes {
        var_prefix: "$clt.".to_owned(),
        async_prefix: "".to_owned(),
        return_mark: "%%ret%%".to_owned(),
      },
    );

    assert!(code.contains("& 1023"));
    assert!(code.contains("args.length"));
    assert!(code.contains("args[0]"));
    assert!(!code.contains("{ret_var}"));
  }
}

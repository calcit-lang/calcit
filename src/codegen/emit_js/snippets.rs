use crate::{builtins::meta::js_gensym, codegen::emit_js::get_proc_prefix};

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn tmpl_try(err_var: String, body: String, handler: String, return_code: &str) -> String {
  format!(
    "try {{
  {body}
}} catch ({err_var}) {{
  {return_code} ({handler})({err_var})
}}",
  )
}

pub fn tmpl_fn_wrapper(body: String) -> String {
  format!(
    "(function __fn__(){{
  {body}
}})()"
  )
}

pub fn tmpl_args_fewer_than(args_count: usize) -> String {
  format!(
    "
if (arguments.length < {args_count}) throw new Error('too few arguments');"
  )
}

pub fn tmpl_args_between(a: usize, b: usize) -> String {
  format!(
    "
if (arguments.length < {a}) throw new Error('too few arguments');
if (arguments.length > {b}) throw new Error('too many arguments');"
  )
}

pub fn tmpl_args_exact(name: &str, args_count: usize, at_ns: &str) -> String {
  let proc_ns = get_proc_prefix(at_ns);
  format!(
    "
  if (arguments.length !== {args_count}) throw {proc_ns}_args_throw('{name}', {args_count}, arguments.length);"
  )
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
  body0: String,
  prefixes: RecurPrefixes,
) -> String {
  let var_prefix = prefixes.var_prefix;
  let return_mark = &prefixes.return_mark;
  let async_prefix = &prefixes.async_prefix;
  let ret_var = js_gensym("ret");
  let times_var = js_gensym("times");
  let body = body0.replace(return_mark, &ret_var); // dirty trick for injection

  let check_recur_args = check_args.replace("arguments.length", &format!("{ret_var}.args.length"));

  format!(
    "{async_prefix}function {name}({args_code}) {{
  {check_args}
  {spreading_code}
  let {times_var} = 0;
  while(true) {{ /* Tail Recursion */
    let {ret_var} = null;
    if ({times_var} > 10000000) throw new Error('tail recursion not finished after 10M iterations');
    {body}
    if ({ret_var} instanceof {var_prefix}CalcitRecur) {{
      {check_recur_args}
      [ {args_code} ] = {ret_var}.args;
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
import {{newTag, arrayToList, listToArray, CalcitSliceList, CalcitSymbol, CalcitRecur}} from {name};
import * as $procs from {name};
export * from {name};
",
  )
}

pub fn tmpl_classes_registering() -> String {
  format!(
    "
$procs.register_calcit_builtin_impls({{
  list: _$n_core_list_methods,
  map: _$n_core_map_methods,
  number: _$n_core_number_methods,
  set: _$n_core_set_methods,
  string: _$n_core_string_methods,
  nil: _$n_core_nil_methods,
  fn: _$n_core_fn_methods,
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
  format!(
    "
{arr}.forEach(x => {{
  _tag[x] = {prefix}newTag(x);
}});
"
  )
}

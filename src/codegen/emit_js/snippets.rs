use crate::builtins::meta::js_gensym;

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn tmpl_try(err_var: String, body: String, handler: String, return_code: &str) -> String {
  format!(
    "try {{
  {}
}} catch ({}) {{
  {} ({})({}.toString())
}}",
    body, err_var, return_code, handler, err_var,
  )
}

pub fn tmpl_fn_wrapper(body: String) -> String {
  format!(
    "(function __fn__(){{
  {}
}})()",
    body
  )
}

pub fn tmpl_args_fewer_than(args_count: usize) -> String {
  format!(
    "
if (arguments.length < {}) throw new Error('too few arguments');",
    args_count
  )
}

pub fn tmpl_args_between(a: usize, b: usize) -> String {
  format!(
    "
if (arguments.length < {}) throw new Error('too few arguments');
if (arguments.length > {}) throw new Error('too many arguments');",
    a, b
  )
}

pub fn tmpl_args_exact(name: &str, args_count: usize) -> String {
  format!(
    "
if (arguments.length !== {}) throw _calcit_args_mismatch('{}', {}, arguments.length);",
    args_count, name, args_count
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
",
    name = name,
    args_code = args_code,
    check_args = check_args,
    spreading_code = spreading_code,
    body = body,
    var_prefix = var_prefix,
    ret_var = ret_var,
    times_var = times_var,
    check_recur_args = check_recur_args,
    async_prefix = async_prefix
  )
}

pub fn tmpl_import_procs(name: String) -> String {
  format!(
    "
import {{kwd, arrayToList, listToArray, CalcitSliceList, CalcitSymbol, CalcitRecur}} from {};
import * as $calcit_procs from {};
export * from {};
",
    name, name, name,
  )
}

pub fn tmpl_classes_registering() -> String {
  format!(
    "
$calcit_procs.register_calcit_builtin_classes({{
  list: _$n_core_list_class,
  map: _$n_core_map_class,
  number: _$n_core_number_class,
  record: _$n_core_record_class,
  set: _$n_core_set_class,
  string: _$n_core_string_class,
  nil: _$n_core_nil_class,
  fn: _$n_core_fn_class,
}});

let runtimeVersion = $calcit_procs.calcit_version;
let cli_version = '{}';

if (runtimeVersion !== cli_version) {{
  console.warn(`[Warning] versions mismatch, CLI using: ${{cli_version}}, runtime using: ${{runtimeVersion}}`)
}}
",
    CALCIT_VERSION
  )
}

pub fn tmpl_keywords_init(arr: &str, prefix: &str) -> String {
  format!(
    "
{}.forEach(x => {{
  _kwd[x] = {}kwd(x);
}});
",
    arr, prefix
  )
}

pub fn tmpl_errors_init() -> String {
  String::from(
    "
let _calcit_args_mismatch = (name, expected, got) => {
  return new Error(`\\`${name}\\` expected ${expected} params, got ${got}`);
};
",
  )
}

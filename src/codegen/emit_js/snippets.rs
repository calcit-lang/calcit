use crate::builtins::meta::js_gensym;

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn tmpl_try(err_var: String, body: String, handler: String) -> String {
  format!(
    "try {{
  return {}
}} catch ({}) {{
  return ({})({}.toString())
}}",
    body, err_var, handler, err_var,
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
if (arguments.length < {}) throw new Error('too few arguments')",
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

pub fn tmpl_args_exact(args_count: usize) -> String {
  format!(
    "
if (arguments.length !== {}) throw new Error('argument sizes do not match');",
    args_count
  )
}

pub fn tmpl_tail_recursion(
  name: String,
  args_code: String,
  check_args: String,
  spreading_code: String,
  body0: String,
  var_prefix: String,
  return_mark: &str,
) -> String {
  let ret_var = js_gensym("ret");
  let times_var = js_gensym("times");
  let body = body0.replace(return_mark, &ret_var); // dirty trick for injection

  let check_recur_args = check_args.replace("arguments.length", &format!("{}.args.length", ret_var));

  format!(
    "function {name}({args_code}) {{
  {check_args}
  {spreading_code}
  let {ret_var} = null;
  let {times_var} = 0;
  while(true) {{ /* Tail Recursion */
    if ({times_var} > 10000) throw new Error('tail recursion not stopping');
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
    check_recur_args = check_recur_args
  )
}

pub fn tmpl_import_procs(name: String) -> String {
  format!(
    "
import {{kwd, arrayToList, listToArray, CalcitList, CalcitSymbol, CalcitRecur}} from {};
import * as $calcit_procs from {};
export * from {};
",
    name, name, name,
  )
}

pub fn tmpl_export_macro(name: String) -> String {
  format!(
    "
export var {} = () => {{/* Macro */}}
{}.isMacro = true;
",
    name, name
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

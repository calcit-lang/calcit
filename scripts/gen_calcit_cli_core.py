#!/usr/bin/env python3
"""Generate calcit.cli namespace block for src/cirru/calcit-core.cirru from injection specs."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOD_RS = ROOT / "src/bin/injection/mod.rs"
SPECS_RS = ROOT / "src/bin/injection/calcit_cli_specs.rs"
CORE = ROOT / "src/cirru/calcit-core.cirru"
MARKER = "    |calcit.internal $ %{} :FileEntry"
NS_HEADER = "    |calcit.cli $ %{} :FileEntry"

LIST_RET = {"list-ns", "list-defs", "search-def", "find-symbol", "list-examples", "list-usages", "list-modules"}

KIND_TO_TYPE = {"string": ":string", "bool": ":bool", "usize": ":number"}

EXAMPLES: dict[str, str] = {
    "list-ns": "calcit.cli/list-ns $ {} (:file-path |calcit/test.cirru)",
    "list-defs": "calcit.cli/list-defs $ {} (:file-path |calcit/test.cirru) (:namespace |app.main)",
    "show-def": "calcit.cli/show-def $ {} (:file-path |calcit/test.cirru) (:target |app.main/main!)",
    "peek-def": "calcit.cli/peek-def $ {} (:file-path |calcit/test.cirru) (:target |app.main/main!) (:lines 5)",
    "search-def": "calcit.cli/search-def $ {} (:file-path |calcit/test.cirru) (:target |app.main/main!) (:keyword |main)",
    "find-symbol": "calcit.cli/find-symbol $ {} (:file-path |calcit/test.cirru) (:symbol |main)",
    "show-schema": "calcit.cli/show-schema $ {} (:file-path |calcit/test.cirru) (:target |app.main/main!)",
    "list-config": "calcit.cli/list-config $ {} (:file-path |calcit/test.cirru)",
    "format-file": "calcit.cli/format-file $ {} (:file-path |calcit/test.cirru)",
    "cirru-parse": "calcit.cli/cirru-parse $ {} (:code \"|range 3\")",
    "cirru-show-guide": "calcit.cli/cirru-show-guide $ {}",
    "docs-search": "calcit.cli/docs-search $ {} (:keyword |trigger-inc)",
    "trigger-inc": "calcit.cli/trigger-inc $ {} (:file-path |calcit/test.cirru) (:changed |app.main/main!)",
    "analyze-call-graph": "calcit.cli/analyze-call-graph $ {} (:file-path |calcit/test.cirru)",
    "show-error": "calcit.cli/show-error $ {}",
}

WRITE_PREFIXES = (
    "edit-",
    "tree-",
    "rm-",
    "set-",
    "add-",
    "rename-",
    "mv-",
    "split-",
    "format-",
    "clear-",
    "trigger-",
)

REGISTER_RE = re.compile(
    r'register_calcit_cli_descriptor\(\s*"calcit\.cli/([^"]+)"\s*,\s*\w+\s*,\s*(\w+)\s*,\s*"([^"]*)"',
)
SPEC_CONST_RE = re.compile(
    r'pub const (\w+): CliArgSpec = spec\(\s*"([^"]+)"\s*,\s*CliArgKind::(\w+)\s*,\s*(true|false)\s*,\s*(None|Some\([^)]+\))',
)
STATIC_SPEC_RE = re.compile(r"pub static (\w+): &\[CliArgSpec\] = &\[(.*?)\];", re.S)


def parse_spec_consts(text: str) -> dict[str, tuple[str, str, bool, str | None]]:
    consts: dict[str, tuple[str, str, bool, str | None]] = {}
    for m in SPEC_CONST_RE.finditer(text):
        name, key, kind, required, default_raw = m.groups()
        default = None
        if default_raw != "None":
            dm = re.search(r"CliArgDefault::(\w+)\(([^)]*)\)", default_raw)
            if dm:
                dk, dv = dm.group(1), dm.group(2)
                default = dv.strip('"') if dk == "String" else dv
        consts[name] = (key, kind.lower(), required == "true", default)
    return consts


def parse_static_specs(text: str, consts: dict[str, tuple[str, str, bool, str | None]]) -> dict[str, list]:
    statics: dict[str, list] = {}
    for m in STATIC_SPEC_RE.finditer(text):
        static_name, body = m.group(1), m.group(2)
        fields = []
        for token in re.findall(r'spec\(\s*"([^"]+)"[^)]+\)|(\w+)', body):
            inline_key, const_name = token
            if inline_key:
                im = re.search(
                    rf'spec\(\s*"{re.escape(inline_key)}"\s*,\s*CliArgKind::(\w+)\s*,\s*(true|false)\s*,\s*(None|Some\([^)]+\))',
                    body,
                )
                if not im:
                    continue
                kind, required, default_raw = im.group(1), im.group(2), im.group(3)
                default = None
                if default_raw != "None":
                    dm = re.search(r"CliArgDefault::(\w+)\(([^)]*)\)", default_raw)
                    if dm:
                        dk, dv = dm.group(1), dm.group(2)
                        default = dv.strip('"') if dk == "String" else dv
                fields.append((inline_key, kind.lower(), required == "true", default))
            elif const_name in consts:
                fields.append(consts[const_name])
        statics[static_name] = fields
    return statics


def parse_mod_rs(text: str) -> list[tuple[str, str, str]]:
    return sorted(REGISTER_RE.findall(text), key=lambda x: x[0])


def field_type_ann(kind: str, required: bool, default: str | None) -> str:
    base = KIND_TO_TYPE.get(kind, ":string")
    if not required or default is not None:
        return f"(:: :optional {base})"
    return base


def args_schema(fields: list) -> str:
    if not fields:
        return ":args $ [] ({})"
    pairs = " ".join(f"(:{key} {field_type_ann(kind, required, default)})" for key, kind, required, default in fields)
    return f":args $ [] ({{}} {pairs})"


def return_schema(name: str) -> str:
    if name in LIST_RET:
        return "(:return (:: :list :string))"
    return "(:return :string)"


def tags_for(name: str) -> str:
    tags = ":proc :cli :io :native :experimental"
    if any(name.startswith(p) for p in WRITE_PREFIXES):
        tags += " :write"
    return tags


def gen_block(procs: list[tuple[str, str, str]], statics: dict[str, list]) -> str:
    # :schema :args is documentation/query metadata only; runtime validation uses Rust cli_options specs.
    lines = [NS_HEADER, "      :defs $ {}"]
    for name, spec_name, hint in procs:
        fields = statics.get(spec_name, [])
        keys_doc = ", ".join(f":{k}" for k, _, _, _ in fields)
        doc = hint or f"Host CLI builtin calcit.cli/{name}."
        if keys_doc:
            doc = f"{doc} Options: {keys_doc}."
        lines.append(f'        |{name} $ %{{}} :CodeEntry (:doc "|{doc}")')
        lines.append("          :code $ quote &runtime-implementation")
        lines.append("          :examples $ []")
        if name in EXAMPLES:
            lines.append(f"            quote $ {EXAMPLES[name]}")
        lines.append("          :schema $ :: :fn")
        lines.append(f"            {{}} {return_schema(name)}")
        lines.append(f"              {args_schema(fields)}")
        lines.append(f"          :tags $ #{{}} {tags_for(name)}")
    lines.append(
        '      :ns $ %{} :NsEntry (:doc "|Host CLI builtins for cr exec. '
        'Call with map options: calcit.cli/f $ {} (:file-path |path) (:target |ns/def).")'
    )
    lines.append("        :code $ quote")
    lines.append("          ns calcit.cli")
    return "\n".join(lines)


def main() -> int:
    mod_text = MOD_RS.read_text()
    specs_text = SPECS_RS.read_text()
    procs = parse_mod_rs(mod_text)
    consts = parse_spec_consts(specs_text)
    statics = parse_static_specs(specs_text, consts)
    if not procs:
        print("no calcit.cli procs found in mod.rs", file=sys.stderr)
        return 1

    block = gen_block(procs, statics)
    core = CORE.read_text()

    if NS_HEADER in core:
        start = core.index(NS_HEADER)
        end = core.index(MARKER, start)
        core = core[:start] + block + "\n" + core[end:]
        action = "updated"
    elif MARKER in core:
        core = core.replace(MARKER, block + "\n" + MARKER, 1)
        action = "inserted"
    else:
        print(f"marker not found in {CORE}", file=sys.stderr)
        return 1

    CORE.write_text(core)
    print(f"{action} calcit.cli with {len(procs)} procs in {CORE.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
import os, re
src = "src/codegen/emit_wasm.rs"
subdir = "src/codegen/emit_wasm"
with open(src) as f:
    lines = f.readlines()
def extract(ranges):
    parts = []
    for (a, b) in ranges:
        parts.extend(lines[a-1:b])
    return "".join(parts)
def make_pub_super(content):
    content = re.sub(r'^fn ', 'pub(super) fn ', content, flags=re.MULTILINE)
    content = re.sub(r'^enum ', 'pub(super) enum ', content, flags=re.MULTILINE)
    content = re.sub(r'^const ', 'pub(super) const ', content, flags=re.MULTILINE)
    return content
H = "use super::*;\n\n"
files = {
    "heap.rs":    make_pub_super(H + extract([(1636, 1899)])),
    "lists.rs":   make_pub_super(H + extract([(1901,2819),(4395,4476),(4963,5089)])),
    "maps.rs":    make_pub_super(H + extract([(2820,3065),(3793,4394),(5090,5165),(5265,5495)])),
    "sets.rs":    make_pub_super(H + extract([(3066,3792),(5166,5264)])),
    "strings.rs": make_pub_super(H + extract([(4477,4962),(5917,len(lines))])),
    "hof.rs":     make_pub_super(H + extract([(5496,5916)])),
}
os.makedirs(subdir, exist_ok=True)
for name, content in files.items():
    path = os.path.join(subdir, name)
    with open(path, "w") as f:
        f.write(content)
    print(f"Created {path} ({content.count(chr(10))} lines)")
NEW = "\n" + "\n".join([
    '#[path = "emit_wasm/heap.rs"]', "mod heap;",
    '#[path = "emit_wasm/lists.rs"]', "mod lists;",
    '#[path = "emit_wasm/maps.rs"]', "mod maps;",
    '#[path = "emit_wasm/sets.rs"]', "mod sets;",
    '#[path = "emit_wasm/strings.rs"]', "mod strings;",
    '#[path = "emit_wasm/hof.rs"]', "mod hof;",
    "",
    "pub(super) use heap::*;",
    "use lists::*;",
    "use maps::*;",
    "use sets::*;",
    "use strings::*;",
    "use hof::*;",
    "",
]) + "\n"
result = extract([(1,62)]) + extract([(63,96)]) + NEW + extract([(97,1634)])
with open(src, "w") as f:
    f.write(result)
print(f"Updated {src} ({result.count(chr(10))} lines)")

import assert from "node:assert/strict";
import { cp, mkdtemp, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const fixtureRoot = await mkdtemp(join(tmpdir(), "calcit-js-runtime-identity-"));

try {
  const runtimeAPath = join(fixtureRoot, "runtime-a", "lib");
  const runtimeBPath = join(fixtureRoot, "runtime-b", "lib");
  await cp(new URL("../lib", import.meta.url), runtimeAPath, { recursive: true });
  await cp(new URL("../lib", import.meta.url), runtimeBPath, { recursive: true });
  await symlink(resolve("node_modules"), join(fixtureRoot, "node_modules"), "dir");

  const runtimeA = await import(pathToFileURL(join(runtimeAPath, "calcit.procs.mjs")).href);
  const symbolFromString = runtimeA.turn_symbol("hello");
  assert.ok(symbolFromString instanceof runtimeA.CalcitSymbol, "turn-symbol must return a Symbol on JS");
  assert.equal(symbolFromString.value, "hello");
  assert.equal(runtimeA.turn_symbol(symbolFromString), symbolFromString, "legacy Symbol input remains runtime-compatible");
  assert.equal(runtimeA.turn_symbol(runtimeA.newTag("hello")).value, "hello", "legacy Tag input remains runtime-compatible");
  assert.throws(() => runtimeA.turn_symbol(new runtimeA.CalcitSliceList([1])), /Unexpected data for symbol/);
  assert.ok(Number.isNaN(runtimeA._$n_parse_float("+nan")), "JS must parse signed NaN like native");
  assert.equal(runtimeA._$n_parse_float("INF"), Infinity, "JS must parse case-insensitive infinity like native");
  assert.equal(runtimeA._$n_parse_float("-Infinity"), -Infinity, "JS must preserve a negative non-finite number");
  assert.equal(runtimeA._$n_parse_float("1e309"), Infinity, "overflowing decimal input remains a successful Number");
  assert.equal(runtimeA._$n_parse_float("infinite"), null, "invalid non-finite spellings must remain errors");
  const writes = [];
  const waits = [];
  globalThis.__calcit_injections__ = {
    read_file: (path) => `content:${path}`,
    read_dir: (path, recursive) => [`${path}/z`, `${path}/${recursive ? "deep/a" : "a"}`],
    write_file: (path, content) => writes.push([path, content]),
    wait_ms: (milliseconds) => waits.push(milliseconds),
  };
  assert.equal(runtimeA.read_file("demo.txt"), "content:demo.txt", "read-file must return the injected host value");
  assert.deepEqual(
    runtimeA.listToArray(runtimeA.read_dir("demo", false)),
    ["demo/a", "demo/z"],
    "read-dir must validate, sort, and convert injected host paths"
  );
  assert.equal(runtimeA.write_file("demo.txt", "next"), undefined, "write-file must preserve its Unit contract");
  assert.deepEqual(writes, [["demo.txt", "next"]]);
  globalThis.__calcit_injections__.read_file = () => undefined;
  assert.throws(() => runtimeA.read_file("bad.txt"), /expected a string result/);
  globalThis.__calcit_injections__.read_dir = () => ["ok", 1];
  assert.throws(() => runtimeA.read_dir("bad", false), /array of strings/);
  assert.equal(runtimeA.get_env, runtimeA._$n_get_env, "legacy get_env export should delegate to the raw proc");
  assert.equal(typeof runtimeA._$n_ffi_response_resolve, "function", "generated async FFI response imports must link on JS");
  assert.equal(typeof runtimeA._$n_ffi_response_reject, "function", "generated async FFI rejection imports must link on JS");
  assert.equal(typeof runtimeA._$n_ffi_task_cancel, "function", "generated async FFI task imports must link on JS");
  assert.equal(runtimeA.get_env("CALCIT_MISSING_ENV_FOR_RUNTIME_TEST", "fallback"), "fallback");
  assert.equal(
    runtimeA._$n_str_$o_replace("a&a&", "&", "&amp;"),
    "a&amp;a&amp;",
    "string replacement must finish when the replacement contains the pattern"
  );
  assert.equal(
    runtimeA._$n_str_$o_replace("a.b", ".", "$&"),
    "a$&b",
    "string replacement must treat patterns and replacement text literally"
  );
  assert.equal(runtimeA._$n_str_$o_replace("ab", "", "-"), "-a-b-", "empty string patterns should replace each boundary");
  assert.equal(runtimeA._$n_str_$o_utf8_byte_count("abc"), 3, "ASCII byte length must match code-unit length");
  assert.equal(runtimeA._$n_str_$o_utf8_byte_count("é"), 2, "two-byte UTF-8 scalars must be counted exactly");
  assert.equal(runtimeA._$n_str_$o_utf8_byte_count("中"), 3, "three-byte UTF-8 scalars must be counted exactly");
  assert.equal(runtimeA._$n_str_$o_utf8_byte_count("😀"), 4, "surrogate pairs must count as one four-byte UTF-8 scalar");
  assert.equal(runtimeA._$n_str_$o_count("A😀"), 2, "string count must use Unicode scalars rather than UTF-16 code units");
  // Non-finite JS host values are not representable as ordinary source literals.
  for (const index of [NaN, Infinity, -Infinity]) {
    assert.throws(() => runtimeA._$n_str_$o_nth("😀", index), /non-negative integer/);
    assert.throws(() => runtimeA._$n_str_$o_contains_$q_("😀", index), /non-negative integer/);
    assert.throws(() => runtimeA._$n_str_$o_slice("😀", 0, index), /non-negative integer/);
  }
  assert.equal(runtimeA._$n_str_$o_slice("A😀中", 1), "😀中", "omitted end retains scalar indexing");
  assert.equal(runtimeA.last("中😀"), "😀", "legacy runtime exports must retain complete scalars");
  assert.equal(runtimeA.butlast("中😀"), "中");
  // Malformed UTF-16 can only arrive through the JS host boundary. Do not
  // normalize it to replacement characters or count a lone surrogate twice.
  for (const surrogate of ["\uD800", "\uDC00"]) {
    assert.equal(runtimeA._$n_str_$o_find_index(`${surrogate}a`, "a"), 1);
    assert.equal(runtimeA._$n_str_$o_find_index(`😀${surrogate}a`, "a"), 2);
    assert.equal(runtimeA._$n_str_$o_find_index(`${surrogate}a`, surrogate), 0);
    assert.equal(runtimeA._$n_str_$o_find_index(`${surrogate}a`, "\uFFFD"), -1);
  }

  const boundedList = new runtimeA.CalcitSliceList([1, 2]);
  assert.equal(runtimeA._$n_list_$o_nth(boundedList, 1), 2);
  assert.throws(
    () => runtimeA._$n_list_$o_nth(boundedList, 2),
    /index 2 out of bounds for list of length 2/,
    "internal &list:nth must fail instead of returning nil outside its T contract",
  );

  const refDerefImpl = new runtimeA.CalcitImpl(
    runtimeA.newTag("&core-ref-methods"),
    [runtimeA.newTag("deref")],
    [runtimeA._$n_atom_$o_deref],
  );
  runtimeA.register_calcit_builtin_impls({ ref: new runtimeA.CalcitSliceList([refDerefImpl]) });
  assert.equal(runtimeA.invoke_method("deref", runtimeA.atom(7)), 7, "dynamic Ref fallback must use the registered Ref impl table");

  // Host-created impl tables bypass source preprocessing. Runtime dispatch
  // still cannot choose between two implementations of one nominal origin.
  const traitMethod = runtimeA.newTag("render");
  const leftOrigin = new runtimeA.CalcitTrait(runtimeA.newTag("Show"), [traitMethod], [null]);
  const rightOrigin = new runtimeA.CalcitTrait(runtimeA.newTag("Show"), [traitMethod], [null]);
  const calls = [];
  const makeImpl = (name, origin, result) => new runtimeA.CalcitImpl(
    runtimeA.newTag(name), [traitMethod], [() => { calls.push(result); return result; }], origin,
  );
  const leftImpl = makeImpl("Left", leftOrigin, "left");
  const secondLeftImpl = makeImpl("SecondLeft", leftOrigin, "second-left");
  const rightImpl = makeImpl("Right", rightOrigin, "right");
  const makeReceiver = (impls) => {
    const definition = new runtimeA.CalcitStructDef(runtimeA.newTag("Card"), [], [], impls);
    return new runtimeA.CalcitStructValue(definition.name, [], [], definition);
  };
  for (const impls of [[leftImpl, rightImpl], [rightImpl, leftImpl]]) {
    const receiver = makeReceiver(impls);
    assert.equal(runtimeA._$n_trait_call(leftOrigin, traitMethod, receiver), "left");
    assert.equal(runtimeA._$n_trait_call(rightOrigin, traitMethod, receiver), "right");
  }
  for (const impls of [[leftImpl, secondLeftImpl], [secondLeftImpl, leftImpl], [leftImpl, leftImpl]]) {
    const receiver = makeReceiver(impls);
    const countBefore = calls.length;
    let evaluated = 0;
    assert.throws(() => runtimeA._$n_trait_call(leftOrigin, traitMethod, receiver, ++evaluated), /E_DUPLICATE_TRAIT_IMPL.*duplicate impls/);
    assert.equal(evaluated, 1, "ordinary argument evaluation must remain exactly once before dispatch");
    runtimeA.register_calcit_builtin_impls({ number: new runtimeA.CalcitSliceList(impls) });
    assert.throws(() => runtimeA._$n_trait_call(leftOrigin, traitMethod, 1), /E_DUPLICATE_TRAIT_IMPL.*duplicate impls/);
    assert.equal(calls.length, countBefore, "duplicate rejection must invoke neither candidate");
  }
  runtimeA.register_calcit_builtin_impls({ number: null });
  assert.throws(() => runtimeA._$n_trait_call(rightOrigin, traitMethod, makeReceiver([leftImpl])), /cannot find impl/);
  const reloadedOrigin = new runtimeA.CalcitTrait(runtimeA.newTag("Show"), [traitMethod], [null]);
  assert.throws(() => runtimeA._$n_trait_call(reloadedOrigin, traitMethod, makeReceiver([leftImpl])), /cannot find impl/);

  const mapKeyA = "map-key-a";
  const mapKeyB = "map-key-b";
  const typedMapKeys = runtimeA._$n_map_$o_keys(new runtimeA.CalcitSliceMap([mapKeyA, 1, mapKeyB, 2]));
  assert.ok(typedMapKeys instanceof runtimeA.CalcitSet, "&map:keys must match its Set<K> contract on JS");
  assert.equal(typedMapKeys.len(), 2);
  assert.equal(typedMapKeys.contains(mapKeyA), true);
  assert.equal(typedMapKeys.contains(mapKeyB), true);

  for (const map of [
    new runtimeA.CalcitSliceMap([mapKeyA, 1, mapKeyB, 1]),
    new runtimeA.CalcitSliceMap([mapKeyA, 1, mapKeyB, 1]).turnMap(),
  ]) {
    const values = runtimeA._$n_map_$o_vals(map);
    assert.ok(values instanceof runtimeA.CalcitSliceList, "&map:vals must match its List<V> contract on JS");
    assert.deepEqual(runtimeA.listToArray(values), [1, 1], "&map:vals must preserve duplicate values");
  }
  assert.throws(() => runtimeA._$n_map_$o_vals(1), /&map:vals expected a Map/);

  const todoName = runtimeA.newTag("TodoState");
  const todoField = runtimeA.newTag("draft");
  const todoType = new runtimeA.CalcitSymbol("String");
  const todoRecord = new runtimeA.CalcitStructValue(todoName, [todoField], [""]);
  const todoStruct = new runtimeA.CalcitStructDef(todoName, [todoField], [todoType]);
  const todoEnum = new runtimeA.CalcitEnumDef(new runtimeA.CalcitStructValue(todoName, [todoField], [todoType]));
  // Enum definitions are data, not their shared JavaScript name() method.
  const makeEnumDefinition = (name, origin, payload = "String", impls = []) => {
    const field = runtimeA.newTag("item");
    const definition = new runtimeA.CalcitStructDef(runtimeA.newTag(name), [field], [null], impls, origin);
    return new runtimeA.CalcitEnumDef(new runtimeA.CalcitStructValue(
      definition.name, [field], [new runtimeA.CalcitSliceList([new runtimeA.CalcitSymbol(payload)])], definition,
    ));
  };
  const operationDefinition = makeEnumDefinition("Operation", "app.left/Operation");
  const equivalentDefinition = makeEnumDefinition("Operation", "app.left/Operation");
  const otherDefinitions = [
    makeEnumDefinition("ClientOperation", "app.left/ClientOperation"),
    makeEnumDefinition("Operation", "app.right/Operation"),
    makeEnumDefinition("Operation", "app.left/Operation", "Number"),
    makeEnumDefinition("Operation", "app.left/Operation", "String", [
      new runtimeA.CalcitImpl(runtimeA.newTag("Marker"), [], []),
    ]),
  ];
  assert.equal(runtimeA._$n__$e_(operationDefinition, equivalentDefinition), true);
  assert.equal(runtimeA.hashFunction(operationDefinition), runtimeA.hashFunction(equivalentDefinition));
  assert.equal(runtimeA._$n_compare(operationDefinition, equivalentDefinition), 0);
  for (const other of otherDefinitions) {
    assert.equal(runtimeA._$n__$e_(operationDefinition, other), false, "Distinct enum definitions must not compare equal");
    assert.notEqual(runtimeA._$n_compare(operationDefinition, other), 0, "Ordering must distinguish enum definitions");
    assert.equal(Math.sign(runtimeA._$n_compare(operationDefinition, other)), -Math.sign(runtimeA._$n_compare(other, operationDefinition)));
    for (const map of [
      new runtimeA.CalcitSliceMap([operationDefinition, "original", other, "other"]),
      new runtimeA.CalcitSliceMap([operationDefinition, "original", other, "other"]).turnMap(),
    ]) {
      assert.equal(map.get(operationDefinition), "original");
      assert.equal(map.get(equivalentDefinition), "original");
      assert.equal(map.get(other), "other");
    }
    const values = runtimeA._SHA__$M_(operationDefinition, equivalentDefinition, other);
    assert.equal(runtimeA._$n_set_$o_count(values), 2);
  }
  // Loading another runtime replaces ternary-tree's process-wide comparator.
  // Finish single-runtime collection checks before exercising reload identity.
  const runtimeB = await import(pathToFileURL(join(runtimeBPath, "calcit.procs.mjs")).href);
  const todoEnumValue = new runtimeA.CalcitEnumValue(todoField, [""], todoEnum);
  const anonymousEnumValue = new runtimeA.CalcitEnumValue(todoField, [""]);
  assert.equal(runtimeA._$n_enum_def_$o_has_variant_$q_(todoEnum, todoField), true);
  assert.equal(runtimeA._$n_enum_def_$o_has_variant, undefined, "retired Enum export alias must not remain public");
  assert.equal(runtimeA._$n__PCT__$M__$q_, undefined, "retired partial Struct constructor must not remain public");
  const jsData = new runtimeA.CalcitSliceMap([todoField, "next"]);
  const jsDataOptions = new runtimeA.CalcitSliceMap([runtimeA.newTag("add-colon"), true]);
  assert.deepEqual(runtimeA.to_js_data(jsData), { draft: "next" });
  assert.deepEqual(runtimeA.to_js_data(jsData, jsDataOptions), { ":draft": "next" });
  assert.throws(() => runtimeA.to_js_data(jsData, true), /no longer accepts a boolean second argument/);
  const zeroWait = runtimeA._$n_wait_ms(todoEnum, 0, "wait failed");
  assert.equal(zeroWait.tag, runtimeA.newTag("ok"));
  assert.deepEqual(waits, [], "zero wait must not invoke the JavaScript host");
  const positiveWait = runtimeA._$n_wait_ms(todoEnum, 7, "wait failed");
  assert.equal(positiveWait.tag, runtimeA.newTag("ok"));
  assert.deepEqual(waits, [7]);
  delete globalThis.__calcit_injections__.wait_ms;
  const nodeWait = runtimeA._$n_wait_ms(todoEnum, 1, "wait failed");
  assert.equal(nodeWait.tag, runtimeA.newTag("ok"), "Node must provide a blocking Atomics.wait fallback");
  const invalidWait = runtimeA._$n_wait_ms(todoEnum, 1.5, "wait failed");
  assert.equal(invalidWait.tag, runtimeA.newTag("err"));
  globalThis.__calcit_injections__.wait_ms = () => {
    throw new Error("interrupted");
  };
  const failedWait = runtimeA._$n_wait_ms(todoEnum, 1, "wait failed");
  assert.equal(failedWait.tag, runtimeA.newTag("err"));
  assert.deepEqual(failedWait.extra, ["wait failed: interrupted"]);
  globalThis.__calcit_injections__.wait_ms = async () => {
    throw new Error("late rejection");
  };
  const asyncWait = runtimeA._$n_wait_ms(todoEnum, 1, "wait failed");
  assert.equal(asyncWait.tag, runtimeA.newTag("err"));
  assert.deepEqual(asyncWait.extra, ["wait failed: wait_ms injection must complete synchronously"]);
  await new Promise((resolve) => setImmediate(resolve));
  globalThis.__calcit_injections__.read_file = (path) => `typed:${path}`;
  globalThis.__calcit_injections__.write_file = (path, content) => writes.push([path, content]);
  const typedRead = runtimeA._$n_fs_read_text(todoEnum, "typed.txt", "read failed");
  assert.equal(typedRead.tag, runtimeA.newTag("ok"));
  assert.deepEqual(typedRead.extra, ["typed:typed.txt"]);
  assert.equal(typedRead.enumPrototype, todoEnum);
  const typedWrite = runtimeA._$n_fs_write_text(todoEnum, "typed.txt", "内容", "write failed");
  assert.equal(typedWrite.tag, runtimeA.newTag("ok"));
  assert.deepEqual(typedWrite.extra, [undefined]);
  assert.deepEqual(writes.at(-1), ["typed.txt", "内容"]);
  const fsPathName = runtimeA.newTag("FsPath");
  const fsPathField = runtimeA.newTag("value");
  const fsPathType = new runtimeA.CalcitStructDef(fsPathName, [fsPathField], [todoType]);
  globalThis.__calcit_injections__.read_dir = (path, recursive) => {
    assert.equal(recursive, false);
    return [`${path}/z`, `${path}/a`];
  };
  const typedDirectory = runtimeA._$n_fs_read_dir(todoEnum, fsPathType, "typed", "read-dir failed");
  assert.equal(typedDirectory.tag, runtimeA.newTag("ok"));
  const typedPaths = runtimeA.listToArray(typedDirectory.extra[0]);
  assert.deepEqual(
    typedPaths.map((path) => path.values[0]),
    ["typed/a", "typed/z"],
    "typed directory reads must preserve deterministic sorting and nominal FsPath values",
  );
  assert.ok(typedPaths.every((path) => path.structRef === fsPathType));
  globalThis.__calcit_injections__.read_file = () => {
    throw new Error("denied");
  };
  const typedFailure = runtimeA._$n_fs_read_text(todoEnum, "typed.txt", "read failed");
  assert.equal(typedFailure.tag, runtimeA.newTag("err"));
  assert.deepEqual(typedFailure.extra, ["read failed: denied"]);
  assert.equal(todoRecord.toString(), "(%{} 'TodoState (:draft |))");
  assert.equal(todoStruct.toString(), "(%struct-def 'TodoState (:draft 'String))");
  assert.equal(todoEnum.toString(), "(%enum-def 'TodoState)");
  assert.equal(todoEnumValue.toString(), "(%:: 'TodoState :draft |)");
  assert.equal(anonymousEnumValue.toString(), "(%:: _ :draft |)");

  assert.equal(runtimeA.type_of(null).value, "nil", "nil must keep its own runtime type");
  assert.equal(runtimeA.type_of(undefined).value, "unit", "&unit must keep its own runtime type");
  assert.equal(runtimeA.nil_$q_(null), true);
  assert.equal(runtimeA.nil_$q_(undefined), false, "&unit must not satisfy nil?");
  assert.equal(runtimeA._$n__$e_(null, undefined), false, "nil and &unit must not compare equal");
  assert.notEqual(runtimeA.hashFunction(null), runtimeA.hashFunction(undefined), "nil and &unit need distinct hashes");
  assert.equal(runtimeA.toString(null, true), "nil");
  assert.equal(runtimeA.toString(undefined, true), "&unit");
  assert.throws(() => runtimeA.json_stringify(undefined), /cannot encode value: &unit/);
  assert.throws(() => runtimeA.to_cirru_edn(undefined), /cannot encode &unit/);
  const nilShape = { version: 3, root: 0, fingerprint: "runtime-nil-shape", nodes: [{ kind: "nil" }] };
  assert.equal(runtimeA.parse_cirru_edn_as("do nil", nilShape), null, "typed EDN decoding must preserve the Nil node");
  assert.throws(
    () => runtimeA.parse_cirru_edn_as("do |value", nilShape),
    /expected Nil, got string/,
    "typed EDN decoding must reject non-Nil input for the Nil node"
  );

  const effectRef = runtimeA.atom(1);
  const watchKey = runtimeA.newTag("runtime-unit-check");
  const watchCalls = [];
  assert.equal(
    runtimeA.add_watch(effectRef, watchKey, (next, previous) => watchCalls.push([next, previous])),
    undefined,
    "add-watch must return &unit"
  );
  assert.equal(runtimeA.reset_$x_(effectRef, 2), 2, "reset! must return the written value");
  assert.deepEqual(watchCalls, [[2, 1]], "reset! must still notify watchers");
  assert.equal(runtimeA.remove_watch(effectRef, watchKey), undefined, "remove-watch must return &unit");
  const validatedEnum = new runtimeA.CalcitEnumDef(
    new runtimeA.CalcitStructValue(todoName, [todoField], [new runtimeA.CalcitSliceList([todoType])])
  );
  const validatedEnumValue = new runtimeA.CalcitEnumValue(todoField, ["ready"], validatedEnum);
  assert.equal(runtimeA._$n_enum_$o_validate(validatedEnumValue, todoField), undefined, "enum validation must return &unit");
  assert.equal(runtimeA.timeout_call(0, () => {}), undefined, "timeout-call must return &unit");

  const anonymousEnumCode = runtimeA.format_cirru_edn(anonymousEnumValue);
  const parsedAnonymousEnum = runtimeA.parse_cirru_edn(anonymousEnumCode, null);
  assert.equal(parsedAnonymousEnum.tag, todoField, "anonymous enum tags should stay interned after a Cirru EDN round-trip");
  assert.equal(
    parsedAnonymousEnum.tag === todoField ? "matched-draft" : "unmatched",
    "matched-draft",
    "a parsed enum should enter the same identity-based branch as a compiled match"
  );

  const enumOptions = new runtimeA.CalcitSliceMap([todoName, todoEnum]);
  const namedEnumCode = runtimeA.format_cirru_edn(todoEnumValue);
  const parsedNamedEnum = runtimeA.parse_cirru_edn(namedEnumCode, enumOptions);
  assert.equal(parsedNamedEnum.tag, todoField, "named enum tags should stay interned after a Cirru EDN round-trip");
  assert.equal(parsedNamedEnum.enumPrototype, todoEnum, "named enum round-trips should restore the provided prototype");
  assert.throws(
    () => runtimeA._$n_struct_$o_get(todoRecord, runtimeA.newTag("missing")),
    /does not define field :missing/,
    "struct field lookup must reject a missing field instead of returning nil"
  );

  const lateField = runtimeA.newTag("zz-layout-field");
  const earlyField = runtimeA.newTag("aa-layout-field");
  assert.ok(lateField.idx < earlyField.idx, "fixture must register fields in reverse lexical order");
  assert.ok(
    runtimeA.compareTagNames(runtimeA.newTag("\ue000"), runtimeA.newTag("𐀀")) < 0,
    "field ordering must match Rust Unicode scalar ordering rather than UTF-16 code-unit ordering"
  );
  const layoutDef = runtimeA.defstruct(
    runtimeA.newTag("LayoutProbe"),
    new runtimeA.CalcitSliceList([lateField, todoType]),
    new runtimeA.CalcitSliceList([earlyField, todoType])
  );
  assert.deepEqual(
    layoutDef.fields.map((field) => field.value),
    ["aa-layout-field", "zz-layout-field"],
    "Struct field layout must be lexical rather than tag-registration order"
  );
  const reverseLayoutValue = new runtimeA.CalcitStructValue(layoutDef.name, [lateField, earlyField], ["late", "early"], layoutDef);
  assert.deepEqual(reverseLayoutValue.fields, [earlyField, lateField], "Struct value fields should be canonicalized");
  assert.deepEqual(reverseLayoutValue.values, ["early", "late"], "canonicalization must preserve field/value alignment");
  assert.equal(reverseLayoutValue.structRef, layoutDef, "Struct values should retain their canonical definition");
  const layoutValue = new runtimeA.CalcitStructValue(layoutDef.name, layoutDef.fields, ["early", "late"], layoutDef);
  const namedLayout = (path) => runtimeA.bind_struct_definition(layoutDef, path);
  const fromDefinition = (definition) => new runtimeA.CalcitStructValue(definition.name, definition.fields, ["early", "late"], definition);
  const nominalLayout = namedLayout("app.a/LayoutProbe");
  const firstNominal = fromDefinition(nominalLayout);
  const reloadedNominal = fromDefinition(namedLayout("app.a/LayoutProbe"));
  const otherNominal = fromDefinition(namedLayout("app.b/LayoutProbe"));
  assert.equal(runtimeA.bind_struct_definition(nominalLayout, "app.alias/Probe"), nominalLayout, "aliases retain definition identity");
  assert.equal(runtimeA._$n__$e_(firstNominal, reloadedNominal), true, "hot reload uses a stable path, not pointer identity");
  assert.equal(runtimeA.hashFunction(firstNominal), runtimeA.hashFunction(reloadedNominal));
  assert.equal(runtimeA._$n_compare(firstNominal, reloadedNominal), 0);
  assert.equal(runtimeA._$n__$e_(firstNominal, otherNominal), false);
  assert.equal(runtimeA._$n_compare(firstNominal, otherNominal), -1);
  assert.equal(runtimeA._$n__$e_(firstNominal, layoutValue), false, "untyped data does not acquire nominal identity");
  assert.equal(runtimeA._$n_compare(layoutValue, firstNominal), -1);
  assert.equal(firstNominal.assocAt(0, earlyField, "early").structRef.definitionRef, "app.a/LayoutProbe");
  assert.equal(nominalLayout.withImpls([]).definitionRef, "app.a/LayoutProbe");
  assert.equal(firstNominal.withImpls([]).structRef.definitionRef, "app.a/LayoutProbe");
  assert.equal(layoutValue.nthAt(0, earlyField), "early", "indexed Struct reads should use the stable layout");
  assert.equal(layoutValue.assocAt(1, lateField, "updated").values[1], "updated");
  assert.deepEqual(layoutValue.withAt(0, earlyField, "a", 1, lateField, "z").values, ["a", "z"]);
  assert.throws(() => layoutValue.nthAt(0, lateField), /expects field :aa-layout-field/);
  assert.throws(() => layoutValue.assocAt(-1, earlyField, "bad"), /non-negative integer index/);
  assert.throws(() => layoutValue.nthAt(0.5, earlyField), /non-negative integer index/);
  assert.throws(() => layoutValue.assocAt(0, lateField, "bad"), /expects field :aa-layout-field/);
  assert.throws(() => layoutValue.withAt(0, lateField, "bad"), /expects field :aa-layout-field/);
  assert.throws(() => layoutValue.withAt(), /index\/tag\/value triples/);
  const parsedReverseLayout = runtimeA.parse_cirru_edn(
    "%{} 'LayoutProbe\n  :zz-layout-field |late\n  :aa-layout-field |early",
    null
  );
  assert.deepEqual(parsedReverseLayout.fields, [earlyField, lateField], "EDN Struct fields should be canonicalized");
  assert.deepEqual(parsedReverseLayout.values, ["early", "late"], "EDN Struct values must follow canonicalized fields");

  runtimeA.load_console_formatter_$x_();
  const formatter = globalThis.devtoolsFormatters.at(-1);
  const embeddedObjects = (node, found = []) => {
    if (Array.isArray(node)) {
      if (node[0] === "object" && node[1]?.object != null) found.push(node[1].object);
      for (const item of node) embeddedObjects(item, found);
    } else if (node != null && typeof node === "object") {
      for (const value of Object.values(node)) embeddedObjects(value, found);
    }
    return found;
  };
  const assertNominalNameIsSymbol = (value, kind) => {
    const name = embeddedObjects(formatter.header(value)).find((item) => item?.value === "TodoState");
    assert.ok(name instanceof runtimeA.CalcitSymbol, `${kind} formatter should render its name as a symbol`);
  };
  assertNominalNameIsSymbol(todoRecord, "struct value");
  assertNominalNameIsSymbol(todoStruct, "struct definition");
  assertNominalNameIsSymbol(todoEnum, "enum definition");
  assertNominalNameIsSymbol(todoEnumValue, "enum value");
  const anonymousEnumName = embeddedObjects(formatter.header(anonymousEnumValue)).find((item) => item?.value === "_");
  assert.ok(anonymousEnumName instanceof runtimeA.CalcitSymbol, "anonymous enum formatter should render `_` as a symbol");
  assert.ok(formatter.hasBody(todoStruct), "struct definition formatter should expose field types");
  assert.ok(formatter.hasBody(todoEnum), "enum definition formatter should expose variants");
  assert.ok(embeddedObjects(formatter.body(todoStruct)).includes(todoType), "struct definition formatter should embed field types");
  assert.ok(embeddedObjects(formatter.body(todoEnum)).includes(todoType), "enum definition formatter should embed variant payload types");
  const assertFormatterRendersNestedUnit = (value, kind) => {
    assert.ok(JSON.stringify(formatter.body(value)).includes("&unit"), `${kind} formatter should render nested &unit values`);
  };
  assertFormatterRendersNestedUnit(new runtimeA.CalcitSliceList([undefined]), "list");
  assertFormatterRendersNestedUnit(new runtimeA.CalcitSet([undefined]), "set");
  assertFormatterRendersNestedUnit(new runtimeA.CalcitSliceMap([todoField, undefined]), "map");
  assertFormatterRendersNestedUnit(new runtimeA.CalcitStructValue(todoName, [todoField], [undefined]), "struct");
  assert.ok(
    !JSON.stringify(formatter.body(new runtimeA.CalcitSliceList([null]))).includes("&unit"),
    "nested nil must remain distinct from &unit"
  );

  const foreignField = runtimeA.newTag("show");
  const method = () => "demo";
  const foreignImpl = new runtimeA.CalcitImpl(runtimeA.newTag("ForeignImpl"), [foreignField], [method], null);

  assert.ok(
    foreignImpl instanceof runtimeB.CalcitImpl,
    "CalcitImpl values from another runtime module instance should remain recognizable"
  );

  const brand = Symbol.for("@calcit/procs/CalcitImpl");
  const inheritedBrand = Object.assign(Object.create({ [brand]: true }), {
    name: runtimeA.newTag("InheritedImpl"),
    origin: null,
    fields: [],
    values: [],
    cachedHash: null,
  });
  const malformedBrand = {
    [brand]: true,
    name: runtimeA.newTag("MalformedImpl"),
    fields: [],
    values: [],
  };
  assert.ok(!(inheritedBrand instanceof runtimeB.CalcitImpl), "inherited impl brands must not be accepted");
  assert.ok(!(malformedBrand instanceof runtimeB.CalcitImpl), "branded objects without impl fields must not be accepted");

  const clonedImpl = runtimeB._$n_impl_$o__$o_new(runtimeB.newTag("ClonedImpl"), foreignImpl);
  assert.ok(clonedImpl instanceof runtimeB.CalcitImpl);
  assert.ok(clonedImpl.fields[0] instanceof runtimeB.CalcitTag);
  assert.equal(clonedImpl.fields[0].value, "show");
  assert.deepEqual(clonedImpl.values, [method]);

  console.log("JS runtime identity check passed");
} finally {
  delete globalThis.__calcit_injections__;
  await rm(fixtureRoot, { recursive: true, force: true });
}

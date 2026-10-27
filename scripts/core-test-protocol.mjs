import { existsSync } from "node:fs";
import { copyFile, mkdir, realpath, symlink } from "node:fs/promises";
import { dirname, isAbsolute, join, parse, relative, resolve, sep } from "node:path";

// Internal protocol shared by the runner and its host-level regression tests.
// Replay Snapshots are private temporary copies. Guard the entire file rather
// than listing thousands of touched definitions in a single process argument.
export const replayRevision = (preview) => {
  if (typeof preview.original_revision !== "string" || preview.original_revision.length === 0) {
    throw new Error("Replay mutation preview is missing its original Snapshot revision");
  }
  return preview.original_revision;
};

// File modules, including transitive ones, resolve from the original project
// directory. Mirror their paths privately instead of moving replay state back
// into the source project or rewriting published module Snapshots.
export const prepareReplayProject = async ({ root, snapshotPath, snapshot, entry = "default", nodeModules, readSnapshot, readModules }) => {
  const sourceDirectory = dirname(snapshotPath);
  const moduleDirectory = join(sourceDirectory, ".calcit", "modules");
  const mirror = (path) => join(root, "source", relative(parse(path).root, path));
  const project = mirror(sourceDirectory);
  await mkdir(project, { recursive: true });
  await symlink(nodeModules, join(project, "node_modules"), "dir");
  if (existsSync(moduleDirectory)) {
    await mkdir(join(project, ".calcit"));
    await symlink(moduleDirectory, join(project, ".calcit", "modules"), "dir");
  }
  const linkedRoots = [join(project, "node_modules"), join(project, ".calcit", "modules")];
  const inLinkedView = (path) => linkedRoots.some(root => {
    const child = relative(root, path);
    return !isAbsolute(child) && child !== ".." && !child.startsWith(`..${sep}`);
  });
  const copy = async (path) => {
    const destination = mirror(path);
    if (inLinkedView(destination)) {
      if (await realpath(path) !== await realpath(destination)) {
        throw new Error(`Replay source file conflicts with the read-only dependency view: ${path}`);
      }
      return false;
    }
    await mkdir(dirname(destination), { recursive: true });
    await copyFile(path, destination);
    return true;
  };
  const copyManifest = async (directory) => {
    const path = join(directory, "deps.cirru");
    if (existsSync(path)) await copy(path);
  };
  const visited = new Set();
  const resources = new Set();
  const visit = async (path, data, selectedEntry, staged) => {
    const identity = `${path}\0${selectedEntry}\0${staged}`;
    if (visited.has(identity)) return;
    visited.add(identity);
    if (staged) {
      const directory = dirname(path);
      await copyManifest(directory);
      for (const file of Object.values(data[":files"] ?? {})) {
        for (const definition of Object.values(file.defs ?? {})) {
          const resource = definition.ffi?.[":js"]?.[":file"];
          if (typeof resource !== "string") continue;
          const original = resolve(directory, resource);
          const localPath = relative(directory, original);
          // An unused/missing JS implementation must not fail native replay.
          // Invalid resource syntax remains the compiler's responsibility.
          if (isAbsolute(resource) || localPath === ".." || localPath.startsWith(`..${sep}`)
            || !existsSync(original) || resources.has(original)) continue;
          resources.add(original);
          const owner = await realpath(directory);
          const ownedPath = relative(owner, await realpath(original));
          if (inLinkedView(mirror(original))) {
            await copy(original);
            continue;
          }
          if (isAbsolute(ownedPath) || ownedPath === ".." || ownedPath.startsWith(`..${sep}`)) {
            // Keep an escaping link outside the private owner too, so the
            // compiler still rejects it rather than blessing copied bytes.
            await mkdir(dirname(mirror(original)), { recursive: true });
            await symlink(await realpath(original), mirror(original));
          } else {
            await copy(original);
          }
        }
      }
    }
    // Use the CLI's normalized configuration rather than assuming an EDN
    // entry key is a tag: the loader also accepts string entry names.
    const modules = readModules(path, selectedEntry);
    if (modules === undefined) throw new Error(`Replay module ${path} has no entry ${selectedEntry}`);
    for (const module of modules) {
      const candidate = module.endsWith("/") ? `${module}calcit.cirru` : module;
      const local = candidate.startsWith("./");
      const dependency = local ? resolve(sourceDirectory, candidate)
        : isAbsolute(candidate) ? candidate : `${moduleDirectory}${sep}${candidate}`;
      // A shared load failure is reported once, before per-test compilation
      // bisection. Reading metadata never substitutes for executing assertions.
      const imported = readSnapshot(dependency);
      const staged = local && await copy(dependency);
      await visit(dependency, imported, "default", staged);
    }
  };
  await visit(snapshotPath, snapshot, entry, true);
  return project;
};

/** Read only diagnostic-table code cells; prose and other columns are not coverage. */
export const parseWasmDiagnosticTable = (text) => {
  const codes = new Set();
  let inSection = false;
  let inTable = false;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.startsWith("## ")) {
      inSection = line === "## 诊断";
      inTable = false;
      continue;
    }
    if (!inSection) continue;
    const cells = line.split("|").map(cell => cell.trim());
    if (!inTable) {
      inTable = cells.length === 5 && cells[0] === "" && cells[1] === "编号"
        && cells[2] === "含义" && cells[3] === "处理方式" && cells[4] === "";
      continue;
    }
    if (cells.length < 5 || cells[0] !== "" || cells.at(-1) !== "") {
      inTable = false;
      continue;
    }
    for (const [, code] of cells[1].matchAll(/`(E_WASM_[A-Z_]+)`/g)) codes.add(code);
  }
  return codes;
};

export const parseBackendSelection = (value, supported) => {
  const backends = value.split(",").filter(Boolean);
  if (backends.length === 0) throw new Error("--backend requires at least one backend");
  for (const backend of backends) {
    if (!supported.includes(backend)) throw new Error(`unknown backend: ${backend}`);
  }
  if (new Set(backends).size !== backends.length) throw new Error("--backend must not contain duplicate backends");
  return backends;
};

export const parseMarkers = (stdout) => {
  const traces = new Map();
  const events = [];
  let current;
  let last;
  for (const line of stdout.split(/\r?\n/)) {
    const marker = /^@@core-test(-end)?:(\d+)$/.exec(line);
    if (marker) {
      const index = Number(marker[2]);
      const kind = marker[1] ? "end" : "start";
      events.push([kind, index]);
      if (kind === "start") {
        current = last = index;
        traces.set(index, []);
      } else {
        current = undefined;
      }
    } else if (current !== undefined) {
      traces.get(current).push(line);
    }
  }
  return { traces, last, events };
};

// A failed execution may stop on a prefix; a successful one must finish every test.
export const markerProtocolError = (events, tests, success) => {
  const expected = tests.flatMap((test) => [["start", test.index], ["end", test.index]]);
  for (let i = 0; i < events.length; i++) {
    if (JSON.stringify(events[i]) !== JSON.stringify(expected[i])) {
      return `invalid test marker at ${i}: expected ${JSON.stringify(expected[i])}, got ${JSON.stringify(events[i])}`;
    }
  }
  if (success && events.length !== expected.length) {
    return `successful process exited before all tests completed: received ${events.length}/${expected.length} test markers`;
  }
  return undefined;
};

export const hasNativeParity = (outcome, native, backend) => outcome?.status === "pass"
  && native?.status === "pass"
  && (backend === "native" || JSON.stringify(outcome.trace) === JSON.stringify(native.trace));

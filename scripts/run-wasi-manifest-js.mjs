import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const [modulePath, workDir, denyOutput] = process.argv.slice(2);
if (!modulePath || !workDir) {
  throw new Error("usage: run-wasi-manifest-js.mjs <module> <work-dir> [deny-output]");
}

const hostPath = (guestPath) => {
  if (!guestPath.startsWith("workspace/") || guestPath.split("/").includes("..")) {
    throw new Error("unavailable guest path: " + guestPath);
  }
  return path.join(workDir, guestPath);
};

globalThis.__calcit_injections__ = {
  read_file: (guestPath) => fs.readFileSync(hostPath(guestPath), "utf8"),
  write_file: (guestPath, content) => {
    if (denyOutput === "deny-output") {
      throw new Error("output denied by test host");
    }
    fs.writeFileSync(hostPath(guestPath), content, "utf8");
  },
};

const command = await import(pathToFileURL(path.resolve(modulePath)).href);
command.manifest_main_$x_();

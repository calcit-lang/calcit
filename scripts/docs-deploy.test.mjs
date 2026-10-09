import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./deploy-docs.sh", import.meta.url));
const mocks = {
  "ssh-agent": `#!/bin/bash
set -euo pipefail
if [[ "$1" == '-s' ]]; then
  printf 'agent-start\\n' >> "$DOCS_MOCK_EVENTS"
  if [[ "\${DOCS_MOCK_AGENT_STATUS:-0}" != '0' ]]; then exit "$DOCS_MOCK_AGENT_STATUS"; fi
  printf 'SSH_AUTH_SOCK=%q; export SSH_AUTH_SOCK; SSH_AGENT_PID=4242; export SSH_AGENT_PID;\\n' "$DOCS_MOCK_SOCKET"
elif [[ "$1" == '-k' ]]; then
  printf 'agent-stop\\n' >> "$DOCS_MOCK_EVENTS"
else
  exit 81
fi
`,
  "ssh-add": `#!/bin/bash
set -euo pipefail
[[ "$1" == '-' ]]
[[ "$SSH_AUTH_SOCK" == "$DOCS_MOCK_SOCKET" ]]
payload="$(</dev/stdin)"
[[ "$payload" == $'dummy\\nkey' ]]
printf 'key-add\\n' >> "$DOCS_MOCK_EVENTS"
exit "\${DOCS_MOCK_KEY_STATUS:-0}"
`,
  rsync: `#!/bin/bash
set -euo pipefail
[[ "$SSH_AUTH_SOCK" == "$DOCS_MOCK_SOCKET" ]]
[[ "$1" == '-avzr' && "$2" == '--progress' && "$3" == '-e' ]]
[[ "$4" == 'ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new' ]]
[[ "$5" == 'docs/guide with spaces.md' ]]
[[ "$6" == 'rsync-user@tiye.me:/web-assets/repo/calcit-lang/calcit/docs' ]]
[[ "$#" == 6 ]]
printf 'transfer\\n' >> "$DOCS_MOCK_EVENTS"
exit "\${DOCS_MOCK_TRANSFER_STATUS:-0}"
`,
};

for (const [name, overrides, status, events] of [
  ["success", {}, 0, ["agent-start", "key-add", "transfer", "agent-stop"]],
  ["missing key", { DEPLOY_KEY: "" }, 1, []],
  ["agent startup failure", { DOCS_MOCK_AGENT_STATUS: "17" }, 17, ["agent-start"]],
  ["key loading failure", { DOCS_MOCK_KEY_STATUS: "18" }, 18, ["agent-start", "key-add", "agent-stop"]],
  ["transfer failure", { DOCS_MOCK_TRANSFER_STATUS: "23" }, 23, ["agent-start", "key-add", "transfer", "agent-stop"]],
]) {
  test(`documentation deployment: ${name}`, (t) => {
    const directory = mkdtempSync(join(tmpdir(), "calcit-docs-deploy-"));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    const bin = join(directory, "bin");
    mkdirSync(bin);
    mkdirSync(join(directory, "docs"));
    for (const [command, source] of Object.entries(mocks)) {
      writeFileSync(join(bin, command), source, { mode: 0o700 });
    }
    writeFileSync(join(directory, "docs", "guide with spaces.md"), "test documentation\n");
    const trace = join(directory, "events");
    const output = join(directory, "output");
    writeFileSync(trace, "");
    writeFileSync(output, "");
    // No inherited deployment credentials or real SSH/rsync executables are used.
    const result = spawnSync("/bin/bash", [script], {
      cwd: directory,
      encoding: "utf8",
      timeout: 10_000,
      env: {
        PATH: bin,
        DEPLOY_KEY: "dummy\\nkey",
        GITHUB_REPOSITORY: "calcit-lang/calcit",
        GITHUB_OUTPUT: output,
        DOCS_MOCK_SOCKET: join(directory, "agent.sock"),
        DOCS_MOCK_EVENTS: trace,
        ...overrides,
      },
    });
    assert.ifError(result.error);
    assert.equal(result.status, status, result.stderr);
    assert.deepEqual(readFileSync(trace, "utf8").trim().split("\n").filter(Boolean), events);
    assert.equal(readFileSync(output, "utf8"), status === 0 ? "status=Content synced successfully.\n" : "");
    assert.ok(!result.stdout.includes("dummy") && !result.stderr.includes("dummy"), "key content must not be printed");
  });
}

let fs = require("fs");
let pkg = require("../package.json");

if (!fs.existsSync("builds")) {
  fs.mkdirSync("builds/");
}
fs.copyFileSync("target/release/calcit_runner", "builds/calcit_runner");
fs.copyFileSync("builds/calcit_runner", `builds/calcit_runner_${pkg.version}`);

let fs = require("fs");
let pkg = require("../package.json");

if (!fs.existsSync("builds")) {
  fs.mkdirSync("builds/");
}
fs.copyFileSync("target/release/cr", "builds/cr");
fs.copyFileSync("builds/cr", "builds/calcit_runner");
fs.copyFileSync("builds/cr", `builds/cr_${pkg.version}`);

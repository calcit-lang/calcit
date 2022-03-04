let fs = require("fs");
let pkg = require("../package.json");

if (!fs.existsSync("builds")) {
  fs.mkdirSync("builds/");
}
fs.copyFileSync("target/release/cr", "builds/cr");
fs.copyFileSync("builds/cr", "builds/calcit");
fs.copyFileSync("builds/cr", `builds/cr_${pkg.version}`);

fs.copyFileSync("target/release/bundle_calcit", "builds/bundle_calcit");
fs.copyFileSync("builds/bundle_calcit", `builds/bundle_calcit_${pkg.version}`);

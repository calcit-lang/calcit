#!/usr/bin/env node
const { build } = require("estrella")

build({
  entry: "./js-out/main.js",
  outfile: "./js-out/bundle.js",
  bundle: true,
  platform: 'node',
  minify: false,
})


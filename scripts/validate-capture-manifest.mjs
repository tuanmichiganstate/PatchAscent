#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const schema = JSON.parse(
  await readFile(
    path.join(repositoryRoot, "schemas", "capture_manifest.schema.json"),
    "utf8",
  ),
);
const manifestArguments = process.argv.slice(2);

if (manifestArguments.length === 0) {
  console.error(
    "Usage: node scripts/validate-capture-manifest.mjs <capture_manifest.json> [...]",
  );
  process.exitCode = 2;
} else {
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  const validate = ajv.compile(schema);
  let failed = false;

  for (const argument of manifestArguments) {
    const manifestPath = path.resolve(process.cwd(), argument);
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    if (!validate(manifest)) {
      failed = true;
      for (const error of validate.errors ?? []) {
        console.error(
          `ERROR: ${argument}${error.instancePath || "/"} ${error.message}`,
        );
      }
      continue;
    }

    for (const file of manifest.files) {
      const capturePath = path.resolve(path.dirname(manifestPath), file.path);
      let bytes;
      try {
        bytes = await readFile(capturePath);
      } catch {
        failed = true;
        console.error(`ERROR: ${argument}: missing capture file ${file.path}`);
        continue;
      }
      const actual = createHash("sha256").update(bytes).digest("hex");
      if (actual !== file.sha256) {
        failed = true;
        console.error(
          `ERROR: ${argument}: ${file.path} hash is ${actual}, expected ${file.sha256}`,
        );
      }
    }

    if (!failed) {
      console.log(`OK: ${argument}`);
    }
  }

  if (failed) {
    process.exitCode = 1;
  }
}

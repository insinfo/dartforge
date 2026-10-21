// `node runtime/wasm/run_node.mjs <module.wasm>`: runs a `wasm32-linear` module
// through the browser adapter in Node (V8). Exit status: `dartforge_run`'s,
// 70 for a trap, 2 when the module did not run. A module that uses
// `dart:js_interop` loads its companion `<module>.mjs`. With DARTFORGE_WASM_REPORT set,
// prints the module's SHA-256 on stderr.
import { existsSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { runModule } from "./dartforge_host_v1.mjs";

const file = process.argv[2];
if (!file) {
  console.error("usage: node run_node.mjs <module.wasm>");
  process.exit(2);
}
try {
  const bytes = readFileSync(file);
  // The runtime options are this process's `DARTFORGE_*` variables, only.
  const options = Object.fromEntries(
    Object.entries(process.env).filter(([name]) => name.startsWith("DARTFORGE_")),
  );
  // The companion `.mjs` of a module that uses `dart:js_interop`, next to it.
  const companionFile = file.replace(/\.wasm$/, "") + ".mjs";
  const companion = existsSync(companionFile) ? await import(pathToFileURL(companionFile).href) : null;
  const r = await runModule(bytes, {
    stdout: (b) => process.stdout.write(b),
    stderr: (b) => process.stderr.write(b),
  }, options, companion);
  if (process.env.DARTFORGE_WASM_REPORT) console.error(`[run_node] sha256 ${r.sha256}`);
  if (r.trap !== null) {
    console.error(`run_node: trap: ${r.trap}`);
    process.exitCode = 70;
  } else {
    process.exitCode = r.status;
  }
} catch (e) {
  console.error(`run_node: ${file}: ${e}`);
  process.exitCode = 2;
}

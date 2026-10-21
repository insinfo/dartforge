// `node runtime/wasm/run_node.mjs <module.wasm>`: runs a `wasm32-linear` module
// through the browser adapter in Node (V8). Exit status: `dartforge_run`'s,
// 70 for a trap, 2 when the module did not run. A module that uses
// `dart:js_interop` loads its companion `<module>.mjs`. With DARTFORGE_WASM_REPORT set,
// prints the module's SHA-256 on stderr.
import { existsSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { DartError, ModuleExit, runModule } from "./dartforge_host_v1.mjs";

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
  // An exception that reaches Node's event loop from the module's callbacks:
  // a Dart one ends the program as unhandled (status 255), the module's exit
  // ends the process with its status.
  process.on("uncaughtException", (e) => {
    try {
      if (e instanceof DartError) e.report();
    } catch (exit) {
      e = exit;
    }
    if (e instanceof ModuleExit) process.exit(e.status);
    console.error(e);
    process.exit(70);
  });
  const r = await runModule(bytes, {
    stdout: (b) => process.stdout.write(b),
    stderr: (b) => process.stderr.write(b),
  }, options, companion);
  // The program goes on in the module's callbacks until it ends.
  if (r.waiting) r.finished.then(({ status }) => { process.exitCode = status; }, (trap) => {
    console.error(`run_node: trap: ${trap}`);
    process.exit(70);
  });
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

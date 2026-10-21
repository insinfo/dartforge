// `node runtime/wasm/run_dart2wasm.mjs <main.mjs> <main.wasm>`: runs a module of
// the official `dart compile wasm` as the reference of DartForge's WebAssembly
// target (crates/differential, `--wasm`), reporting like the Dart VM so the
// outputs compare: the program's `print`s on stdout and, when an error escapes,
// `Unhandled exception:` and its message on stderr with exit code 255.
//
// dart2wasm's loader prints through `console.log`; on an uncaught error the
// module prints the error and then its stack (two calls) and throws. Those two
// trailing calls are the report, not program output.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const [loader, wasm] = process.argv.slice(2);
const printed = [];
console.log = (value) => printed.push(String(value));
let failed = false;
// An escaping error ends the program, as in the VM: periodic timers do not
// keep it alive after the report.
const fail = () => {
  failed = true;
  process.exit(255);
};
process.on("uncaughtException", fail);
process.on("unhandledRejection", fail);
process.on("exit", () => {
  let message = null;
  if (failed) {
    if (printed.length > 0 && printed[printed.length - 1].startsWith("    at ")) printed.pop();
    message = printed.pop() ?? "";
  }
  if (printed.length > 0) process.stdout.write(printed.join("\n") + "\n");
  if (failed) {
    process.stderr.write(`Unhandled exception:\n${message}\n`);
    process.exitCode = 255;
  }
});
try {
  const module = await import(pathToFileURL(loader).href);
  const compiled = await module.compile(readFileSync(wasm));
  const app = await compiled.instantiate({});
  app.invokeMain();
} catch {
  fail();
}

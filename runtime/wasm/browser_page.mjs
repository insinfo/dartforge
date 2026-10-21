// `node runtime/wasm/browser_page.mjs <module.wasm> <page.html>`: writes a
// self-contained page that runs the module through the adapter in a real
// browser and puts `{sha256, status, trap, stdout, stderr, userAgent}` as JSON
// into `<pre id="result">` when the program ends (read with `--headless
// --dump-dom`; `run_browser.mjs` does it). The module travels as base64 and is
// decoded back to the same bytes: its SHA-256 is computed in the browser, on
// the bytes it instantiates. A module that uses `dart:js_interop` takes its
// companion `<module>.mjs`, inline (its `abi` and `snippets`). The runtime
// options are this process's `DARTFORGE_*` variables, as `run_node.mjs`
// passes them.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

/** The page that runs the module `file` (see the header). */
export function browserPage(file) {
  const adapter = readFileSync(fileURLToPath(new URL("./dartforge_host_v1.mjs", import.meta.url)), "utf8")
    .replace(/^export /gm, "");
  const wasm = readFileSync(file).toString("base64");
  const companionFile = file.replace(/\.wasm$/, "") + ".mjs";
  const companion = existsSync(companionFile) ? readFileSync(companionFile, "utf8") : null;
  const options = Object.fromEntries(
    Object.entries(process.env).filter(([name]) => name.startsWith("DARTFORGE_")),
  );
  // `</script>` inside a JSON string would end the script element.
  const json = (v) => JSON.stringify(v).replace(/</g, "\\u003c");
  return `<!doctype html><meta charset="utf-8"><title>DartForge wasm32-linear</title>
<pre id="result">pending</pre>
<script type="module">
${adapter}
const bytes = Uint8Array.from(atob("${wasm}"), (c) => c.charCodeAt(0));
const companion = ${companion === null ? "null" : `(() => {\n${companion.replace(/^export /gm, "")}\nreturn { abi, snippets };\n})()`};
const out = [], err = [];
const tdOut = new TextDecoder(), tdErr = new TextDecoder();
// The result is published on every change — output, the first status, the
// end — and the headless browser dumps the page once its (virtual) time runs
// out: a program that still waits for the host (JavaScript holds its
// callbacks) reports what it has, as Node does when its event loop empties.
const state = { sha256: null, status: null, trap: null, error: null };
function publish() {
  document.getElementById("result").textContent = JSON.stringify({
    ...state, stdout: out.join(""), stderr: err.join(""), userAgent: navigator.userAgent,
  });
}
// An exception that reaches the event loop from the module's callbacks: a
// Dart one ends the program as unhandled (status 255), the module's exit ends
// it with its status; anything else is a trap.
function uncaught(e) {
  try {
    if (e instanceof DartError) e.report();
  } catch (exit) {
    e = exit;
  }
  Object.assign(state, e instanceof ModuleExit ? { status: e.status, trap: null } : { status: null, trap: String(e) });
  publish();
}
addEventListener("error", (event) => { event.preventDefault(); uncaught(event.error); });
addEventListener("unhandledrejection", (event) => { event.preventDefault(); uncaught(event.reason); });
try {
  const r = await runModule(bytes, {
    stdout: (b) => { out.push(tdOut.decode(b, { stream: true })); publish(); },
    stderr: (b) => { err.push(tdErr.decode(b, { stream: true })); publish(); },
  }, ${json(options)}, companion);
  Object.assign(state, { sha256: r.sha256, status: r.status, trap: r.trap });
  if (r.waiting && r.trap === null) {
    r.finished.then(({ status }) => { state.status = status; publish(); }, (trap) => {
      Object.assign(state, { status: null, trap: String(trap) });
      publish();
    });
  }
} catch (e) {
  state.error = String(e);
}
publish();
</script>
`;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [file, out] = process.argv.slice(2);
  if (!file || !out) {
    console.error("usage: node browser_page.mjs <module.wasm> <page.html>");
    process.exit(2);
  }
  writeFileSync(out, browserPage(file));
}

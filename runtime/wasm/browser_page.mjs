// `node runtime/wasm/browser_page.mjs <module.wasm> <page.html>`: writes a
// self-contained page that runs the module through the adapter in a real
// browser and puts `{sha256, status, trap, stdout, stderr}` as JSON into
// `<pre id="result">` (read with `--headless --dump-dom`). The module travels
// as base64 and is decoded back to the same bytes: its SHA-256 is computed in
// the browser, on the bytes it instantiates.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const [file, out] = process.argv.slice(2);
if (!file || !out) {
  console.error("usage: node browser_page.mjs <module.wasm> <page.html>");
  process.exit(2);
}
const adapter = readFileSync(fileURLToPath(new URL("./dartforge_host_v1.mjs", import.meta.url)), "utf8")
  .replace(/^export /gm, "");
const wasm = readFileSync(file).toString("base64");
const page = `<!doctype html><meta charset="utf-8"><title>DartForge wasm32-mvp</title>
<pre id="result">pending</pre>
<script type="module">
${adapter}
const bytes = Uint8Array.from(atob("${wasm}"), (c) => c.charCodeAt(0));
const out = [], err = [];
const td = new TextDecoder();
let r;
try {
  r = await runModule(bytes, { stdout: (b) => out.push(td.decode(b)), stderr: (b) => err.push(td.decode(b)) });
} catch (e) {
  r = { sha256: null, status: null, trap: null, error: String(e) };
}
r.stdout = out.join(""); r.stderr = err.join(""); r.userAgent = navigator.userAgent;
document.getElementById("result").textContent = JSON.stringify(r);
</script>
`;
writeFileSync(out, page);

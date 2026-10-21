// `node runtime/wasm/run_browser.mjs <module.wasm>`: runs a `wasm32-linear`
// module in a real browser, headless — the page of `browser_page.mjs` in the
// Chromium of `DARTFORGE_BROWSER` (its executable) — and reports it like
// `run_node.mjs`: the program's stdout and stderr, exit status `dartforge_run`'s,
// 70 for a trap, 2 when the module did not run or the browser did not finish.
// The SHA-256 the browser computes on the bytes it instantiated must be the
// file's: the same module runs in every host. Timers run on the browser's
// virtual time (`--virtual-time-budget`), so a program that waits does not
// make the run slower; the browser's language is English (`--lang=en-US`).
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { browserPage } from "./browser_page.mjs";

const file = process.argv[2];
const browser = process.env.DARTFORGE_BROWSER;
if (!file || !browser) {
  console.error("usage: DARTFORGE_BROWSER=<chromium> node run_browser.mjs <module.wasm>");
  process.exit(2);
}
// `process.exit` would cut the program's output still in the pipe: the status
// goes in `process.exitCode`.
process.exitCode = run();

function run() {
  const dir = mkdtempSync(join(tmpdir(), "dartforge-browser-"));
  try {
    const page = join(dir, "page.html");
    writeFileSync(page, browserPage(file));
    const chrome = spawnSync(browser, [
      "--headless=new",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      // The language Node has (the time zone name is that of
      // `Date.prototype.toString`, in the browser's language, as in dart2wasm);
      // Chrome on macOS takes the system's instead.
      "--lang=en-US",
      `--user-data-dir=${join(dir, "profile")}`,
      "--virtual-time-budget=600000",
      "--dump-dom",
      pathToFileURL(page).href,
    ], { encoding: "utf8", maxBuffer: 1 << 30 });
    const match = /<pre id="result">([\s\S]*?)<\/pre>/.exec(chrome.stdout ?? "");
    const text = match?.[1]
      .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"")
      .replace(/&#39;/g, "'").replace(/&nbsp;/g, "\u00a0").replace(/&amp;/g, "&");
    if (text === undefined || text === "pending") {
      console.error(`run_browser: ${file}: the page did not finish (${chrome.error ?? `status ${chrome.status}`})`);
      return 2;
    }
    const r = JSON.parse(text);
    process.stdout.write(r.stdout);
    process.stderr.write(r.stderr);
    if (r.error !== null) {
      console.error(`run_browser: ${file}: ${r.error}`);
      return 2;
    }
    const sha256 = createHash("sha256").update(readFileSync(file)).digest("hex");
    if (r.sha256 !== sha256) {
      console.error(`run_browser: ${file}: the browser ran ${r.sha256}, the file is ${sha256}`);
      return 2;
    }
    if (process.env.DARTFORGE_WASM_REPORT) console.error(`[run_browser] sha256 ${r.sha256} ${r.userAgent}`);
    if (r.trap !== null) {
      console.error(`run_browser: trap: ${r.trap}`);
      return 70;
    }
    return r.status;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

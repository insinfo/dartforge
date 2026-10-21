// W2: two instances of the same module in one host are independent: each has its
// own linear memory and runtime context, so both runs print the same output.
import { readFile } from "node:fs/promises";
import { runModule } from "../../../runtime/wasm/dartforge_host_v1.mjs";

const bytes = await readFile(process.argv[2]);
const runs = [];
for (let i = 0; i < 2; i++) {
  const out = [];
  const result = await runModule(bytes, { stdout: (b) => out.push(...b), stderr: () => {} });
  runs.push({ ...result, stdout: Buffer.from(out).toString("utf8") });
}
const same = runs[0].stdout === runs[1].stdout && runs.every((r) => r.status === 0 && r.trap === null);
process.stdout.write(runs[0].stdout);
process.exit(same ? 0 : 1);

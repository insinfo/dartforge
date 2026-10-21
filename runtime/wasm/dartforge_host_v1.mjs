// The browser (and Node) side of DartForge's host ABI v1 for `wasm32-mvp`
// modules (docs/WASM-HOST-ABI-V1.md). The independent executor
// (`crates/wasm_host`, Wasmi) implements the same contract, so one `.wasm`
// file, byte for byte, runs in both.
//
// Only portable pieces: `WebAssembly`, `crypto.subtle` and typed arrays. No
// WASI, no DOM, no BigInt at the boundary. A trap or a JS error inside the
// module's run is returned as `trap`, never thrown across the boundary.

export const ABI_VERSION = 1;
export const HOST_MODULE = "dartforge_host_v1";

export const WRITE_OK = 0;
export const WRITE_OUT_OF_RANGE = -1;
export const WRITE_BAD_CHANNEL = -2;

/** The SHA-256 of `bytes` in lowercase hex. */
export async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Instantiates and runs a module once.
 *
 * `sinks.stdout(bytes)` and `sinks.stderr(bytes)` receive copies of what the
 * module writes. Resolves to `{ sha256, status, trap }`: `status` is
 * `dartforge_run`'s result, `trap` the engine error when it did not return.
 * Rejects only when the module does not run at all (compile, link or ABI).
 */
export async function runModule(bytes, sinks) {
  const sha256 = await sha256Hex(bytes);
  let memory = null;
  const imports = {
    [HOST_MODULE]: {
      write(channel, offset, length) {
        if (memory === null) return WRITE_OUT_OF_RANGE;
        // The view is taken at every call: `memory.grow` replaces the buffer.
        const view = new Uint8Array(memory.buffer);
        const start = offset >>> 0;
        const len = length >>> 0;
        if (offset < 0 || length < 0 || start + len > view.length) return WRITE_OUT_OF_RANGE;
        const copy = view.slice(start, start + len);
        if (channel === 1) sinks.stdout(copy);
        else if (channel === 2) sinks.stderr(copy);
        else return WRITE_BAD_CHANNEL;
        return WRITE_OK;
      },
    },
  };
  const { instance } = await WebAssembly.instantiate(bytes, imports);
  const exports = instance.exports;
  if (!(exports.memory instanceof WebAssembly.Memory)) throw new Error("no exported memory");
  memory = exports.memory;
  if (typeof exports.dartforge_abi_version !== "function") throw new Error("no dartforge_abi_version");
  const version = exports.dartforge_abi_version();
  if (version !== ABI_VERSION) throw new Error(`module ABI ${version}, host ABI ${ABI_VERSION}`);
  if (typeof exports.dartforge_run !== "function") throw new Error("no dartforge_run");
  try {
    return { sha256, status: exports.dartforge_run(), trap: null };
  } catch (e) {
    return { sha256, status: null, trap: String(e) };
  }
}

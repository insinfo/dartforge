// The browser (and Node) side of DartForge's host ABI v1 for `wasm32-linear`
// modules (docs/WASM-HOST-ABI-V1.md). The independent executor
// (`crates/wasm_host`, Wasmtime) implements the same contract, so one `.wasm`
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
export const UNAVAILABLE = -3;

/** The module's `exit(status)`: thrown through its frames, caught by `runModule`. */
class ModuleExit extends Error {
  constructor(status) {
    super(`the module exited with status ${status}`);
    this.status = status;
  }
}

/** `[start, end)` of linear memory, or null outside it (negatives included). */
function range(memory, offset, length) {
  if (memory === null || offset < 0 || length < 0) return null;
  const start = offset >>> 0;
  const end = start + (length >>> 0);
  return end <= memory.buffer.byteLength ? [start, end] : null;
}

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
        const r = range(memory, offset, length);
        if (r === null) return WRITE_OUT_OF_RANGE;
        // The view is taken at every call: `memory.grow` replaces the buffer.
        const copy = new Uint8Array(memory.buffer).slice(r[0], r[1]);
        if (channel === 1) sinks.stdout(copy);
        else if (channel === 2) sinks.stderr(copy);
        else return WRITE_BAD_CHANNEL;
        return WRITE_OK;
      },
      // dart2wasm's `DateTime.now` reads `Date.now()` too (millisecond steps).
      clock_realtime_us() {
        return Date.now() * 1000;
      },
      clock_monotonic_us() {
        return performance.now() * 1000;
      },
      random_fill(offset, length) {
        const r = range(memory, offset, length);
        if (r === null) return WRITE_OUT_OF_RANGE;
        if (typeof crypto?.getRandomValues !== "function") return UNAVAILABLE;
        const view = new Uint8Array(memory.buffer, r[0], r[1] - r[0]);
        // `getRandomValues` fills at most 65 536 bytes per call.
        for (let i = 0; i < view.length; i += 65536) {
          crypto.getRandomValues(view.subarray(i, Math.min(i + 65536, view.length)));
        }
        return WRITE_OK;
      },
      // The same answers as dart2wasm's `DateTime` (date_patch_patch.dart): the
      // offset from `getTimezoneOffset`, the name inside the parentheses of
      // `Date.prototype.toString`.
      time_zone(epochSeconds, offsetOut, nameOffset, nameCapacity) {
        const out = range(memory, offsetOut, 4);
        const name = range(memory, nameOffset, nameCapacity);
        if (out === null || name === null) return WRITE_OUT_OF_RANGE;
        const date = new Date(epochSeconds * 1000);
        new DataView(memory.buffer).setInt32(out[0], -date.getTimezoneOffset() * 60, true);
        const match = /\((.*)\)/.exec(date.toString());
        const bytes = new TextEncoder().encode(match === null ? "" : match[1]);
        new Uint8Array(memory.buffer, name[0], name[1] - name[0]).set(
          bytes.subarray(0, name[1] - name[0]),
        );
        return bytes.length;
      },
      exit(status) {
        throw new ModuleExit(status);
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
    if (e instanceof ModuleExit) return { sha256, status: e.status, trap: null };
    return { sha256, status: null, trap: String(e) };
  }
}

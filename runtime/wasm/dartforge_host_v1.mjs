// The browser (and Node) side of DartForge's host ABI v1 for `wasm32-linear`
// modules (docs/WASM-HOST-ABI-V1.md). The independent executor
// (`crates/wasm_host`, Wasmtime) implements the same contract, so one `.wasm`
// file, byte for byte, runs in both.
//
// Only portable pieces: `WebAssembly`, `crypto.subtle` and typed arrays. No
// WASI, no DOM, no BigInt at the boundary. A trap or a JS error inside the
// module's run is returned as `trap`, never thrown across the boundary. A
// module that uses `dart:js_interop` also imports `dartforge_js`: this file's
// handle table and the snippets of the module's companion `.mjs` (the
// independent executor refuses such a module).

export const ABI_VERSION = 1;
export const HOST_MODULE = "dartforge_host_v1";
export const JS_MODULE = "dartforge_js";

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

/** Code units per `String.fromCharCode` call (its arguments go on the stack). */
const CHUNK = 8192;

/** A string from code units (Latin-1 bytes or UTF-16), lone surrogates kept. */
function stringFromUnits(units) {
  let s = "";
  for (let i = 0; i < units.length; i += CHUNK) {
    s += String.fromCharCode.apply(null, units.subarray(i, i + CHUNK));
  }
  return s;
}

/**
 * The JavaScript side of the module's interop (`dart:js_interop`,
 * docs/WASM-HOST-ABI-V1.md): the table of the values the module's handles name
 * (0 is `null`/`undefined`, which Dart sees as `null`) and the helpers the
 * snippets of the companion `.mjs` call as `r`.
 */
export class Interop {
  constructor() {
    this.values = [null];
    this.free = [];
    this.memory = null;
    this.exports = null;
    /** The property of a JavaScript function that wraps a Dart function. */
    this.wrappedDartFunction = Symbol("dartforge.wrappedDartFunction");
  }

  /** The value of a handle the module keeps. */
  get(handle) {
    return this.values[handle];
  }

  /** The value of a handle the call consumes (a `String` argument). */
  take(handle) {
    const value = this.values[handle];
    if (handle !== 0) this.release(handle);
    return value;
  }

  /** A new handle of `value` for the module. */
  ref(value) {
    if (value === null || value === undefined) return 0;
    const handle = this.free.length > 0 ? this.free.pop() : this.values.length;
    this.values[handle] = value;
    return handle;
  }

  release(handle) {
    this.values[handle] = undefined;
    this.free.push(handle);
  }

  /** A snippet threw: the exception becomes the Dart side's, pending in the module. */
  fail(error) {
    this.exports.dartforge_js_throw();
    return 0;
  }

  /** The `dartforge_js` imports: the runtime's and the companion's snippets. */
  imports(companion) {
    const units = (offset, length, two) =>
      two ? new Uint16Array(this.memory.buffer, offset, length) : new Uint8Array(this.memory.buffer, offset, length);
    return {
      release: (handle) => this.release(handle),
      string_from_latin1: (offset, length) => this.ref(stringFromUnits(units(offset, length, false))),
      string_from_utf16: (offset, length) => this.ref(stringFromUnits(units(offset, length, true))),
      // The length, negative (`-n - 1`) when a unit does not fit in one byte.
      string_length: (handle) => {
        const s = this.values[handle];
        for (let i = 0; i < s.length; i++) {
          if (s.charCodeAt(i) > 0xff) return -s.length - 1;
        }
        return s.length;
      },
      string_read: (handle, offset, two) => {
        const s = this.values[handle];
        const view = units(offset, s.length, two !== 0);
        for (let i = 0; i < s.length; i++) view[i] = s.charCodeAt(i);
      },
      ...companion.snippets(this),
    };
  }
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
 * `options` are the runtime options `option` answers (`DARTFORGE_*` names).
 * `companion` is the module's companion `.mjs` (its namespace), which a module
 * that uses `dart:js_interop` needs.
 */
export async function runModule(bytes, sinks, options = {}, companion = null) {
  const sha256 = await sha256Hex(bytes);
  const module = await WebAssembly.compile(bytes);
  const interop = WebAssembly.Module.imports(module).some((i) => i.module === JS_MODULE) ? new Interop() : null;
  if (interop !== null && (companion === null || companion.abi !== ABI_VERSION)) {
    throw new Error("the module uses dart:js_interop: it needs its companion .mjs");
  }
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
      option(nameOffset, nameLength, outOffset, outCapacity) {
        const name = range(memory, nameOffset, nameLength);
        const out = range(memory, outOffset, outCapacity);
        if (name === null || out === null) return WRITE_OUT_OF_RANGE;
        const key = new TextDecoder().decode(new Uint8Array(memory.buffer, name[0], name[1] - name[0]));
        if (!Object.hasOwn(options, key)) return -1;
        const bytes = new TextEncoder().encode(String(options[key]));
        new Uint8Array(memory.buffer, out[0], out[1] - out[0]).set(bytes.subarray(0, out[1] - out[0]));
        return bytes.length;
      },
    },
  };
  if (interop !== null) imports[JS_MODULE] = interop.imports(companion);
  const instance = await WebAssembly.instantiate(module, imports);
  const exports = instance.exports;
  if (!(exports.memory instanceof WebAssembly.Memory)) throw new Error("no exported memory");
  memory = exports.memory;
  if (interop !== null) {
    interop.memory = memory;
    interop.exports = exports;
  }
  if (typeof exports.dartforge_abi_version !== "function") throw new Error("no dartforge_abi_version");
  const version = exports.dartforge_abi_version();
  if (version !== ABI_VERSION) throw new Error(`module ABI ${version}, host ABI ${ABI_VERSION}`);
  if (typeof exports.dartforge_run !== "function") throw new Error("no dartforge_run");
  try {
    // `-1`: the program waits for a timer; resume it after the delay it asks
    // for, from the event loop, without blocking the page.
    let status = exports.dartforge_run();
    while (status === -1) {
      const delay = exports.dartforge_delay_us() / 1000;
      await new Promise((resolve) => setTimeout(resolve, Number.isFinite(delay) ? delay : 2 ** 31 - 1));
      status = exports.dartforge_resume();
    }
    return { sha256, status, trap: null };
  } catch (e) {
    if (e instanceof ModuleExit) return { sha256, status: e.status, trap: null };
    return { sha256, status: null, trap: String(e) };
  }
}

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

/**
 * The engine stack V8 gives a thread (`--stack-size`, 984 KiB): the module
 * throws `StackOverflowError` before its frames take it all
 * (`dartforge_engine_stack`, docs/WASM-HOST-ABI-V1.md).
 */
const V8_STACK_BYTES = 984 * 1024;

export const WRITE_OK = 0;
export const WRITE_OUT_OF_RANGE = -1;
export const WRITE_BAD_CHANNEL = -2;
export const UNAVAILABLE = -3;

/** The module's `exit(status)`: thrown through its frames, caught by `runModule`. */
export class ModuleExit extends Error {
  constructor(status) {
    super(`the module exited with status ${status}`);
    this.status = status;
  }
}

/** The longest `setTimeout` delay, in milliseconds. */
const LONGEST_DELAY = 2 ** 31 - 1;

/**
 * The module's own events: the one owner of `dartforge_resume`. `settle` takes
 * a status of the loop (`-1` waiting for a timer, `-2` waiting for the host's
 * callbacks, otherwise the exit status) and schedules what comes next. `first`
 * resolves the first time the program waits for the host or ends; `finished`,
 * when it ends (rejected with a trap).
 */
class Program {
  constructor(exports, enter) {
    this.exports = exports;
    this.enter = enter;
    this.timer = null;
    this.ended = false;
    this.first = new Promise((resolve, reject) => {
      this.resolveFirst = resolve;
      this.rejectFirst = reject;
    });
    this.finished = new Promise((resolve, reject) => {
      this.resolveFinished = resolve;
      this.rejectFinished = reject;
    });
    // `finished` is optional for the host: a trap is reported through `first`.
    this.finished.catch(() => {});
  }

  settle(status) {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    if (this.ended) return;
    if (status === -1) {
      const delay = this.exports.dartforge_delay_us() / 1000;
      this.timer = setTimeout(() => this.resume(), Number.isFinite(delay) ? Math.min(delay, LONGEST_DELAY) : LONGEST_DELAY);
    } else if (status === -2) {
      this.resolveFirst({ status: 0, waiting: true });
    } else {
      this.end(status);
    }
  }

  end(status) {
    this.ended = true;
    this.resolveFirst({ status, waiting: false });
    this.resolveFinished({ status });
  }

  /** The module stopped with an exception: its exit, or a trap. */
  fail(error) {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    if (error instanceof ModuleExit) {
      this.end(error.status);
      return;
    }
    this.ended = true;
    this.rejectFirst(error);
    this.rejectFinished(error);
  }

  resume() {
    this.timer = null;
    let status;
    try {
      status = this.enter(() => this.exports.dartforge_resume());
    } catch (e) {
      this.fail(e);
      return;
    }
    this.settle(status);
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

/** A Dart object JavaScript holds: the id of a root of the runtime. */
export class DartRef {
  constructor(id) {
    this.id = id;
  }
}

/**
 * A Dart exception on its way through JavaScript: thrown by the wrapper of a
 * Dart function the module called back, rethrown in Dart by the next snippet
 * that catches it, or `report()`ed by the host when nothing does.
 */
export class DartError extends Error {
  constructor(interop, id) {
    super("Dart exception");
    this.interop = interop;
    this.id = id;
  }

  /** Ends the program as with an unhandled exception (status 255). */
  report() {
    const id = this.consume();
    if (id !== null) this.interop.exports.dartforge_js_report(id);
  }

  consume() {
    if (this.id === null) return null;
    const id = this.id;
    this.id = null;
    this.interop.errors.unregister(this);
    return id;
  }
}

/**
 * The JavaScript side of the module's interop (`dart:js_interop`,
 * docs/WASM-HOST-ABI-V1.md): the table of the values the module's handles name
 * (0 is `null`/`undefined`, which Dart sees as `null`), the Dart objects
 * JavaScript holds, and the helpers the snippets of the companion `.mjs` call as
 * `r`.
 */
export class Interop {
  constructor() {
    this.values = [null];
    this.free = [];
    this.memory = null;
    this.exports = null;
    this.program = null;
    /** How many calls into the module are running (0: the host's own turn). */
    this.depth = 0;
    /** The arguments of the Dart callbacks running, innermost last. */
    this.frames = [];
    this.DartRef = DartRef;
    /** The property of a JavaScript function that wraps a Dart function. */
    this.wrappedDartFunction = Symbol("dartforge.wrappedDartFunction");
    this.refs = new Map();
    this.refsRegistry = new FinalizationRegistry((id) => {
      if (this.refs.get(id)?.deref() === undefined) {
        this.refs.delete(id);
        this.exports.dartforge_js_dart_release(id);
      }
    });
    this.errors = new FinalizationRegistry((id) => this.exports.dartforge_js_discard(id));
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

  /** The string of a handle: `String(v)` for a value that is not a string. */
  text(handle) {
    const v = this.values[handle];
    return typeof v === "string" ? v : String(v);
  }

  /** The `DartRef` of a Dart object's id: one per id while JavaScript keeps it. */
  dartRef(id) {
    let ref = this.refs.get(id)?.deref();
    if (ref === undefined) {
      ref = new DartRef(id);
      this.refs.set(id, new WeakRef(ref));
      this.refsRegistry.register(ref, id);
    }
    return ref;
  }

  /**
   * A JavaScript function that calls the Dart callback `ref` (`Function.toJS`)
   * through the module's `trampoline`: `length` is the count of its JavaScript
   * parameters, and with `captureThis` the callback receives `this` first.
   */
  wrapDartFunction(ref, length, captureThis, trampoline) {
    const interop = this;
    const wrapper = captureThis
      ? function (...args) {
          return interop.call(trampoline, ref, [this, ...args]);
        }
      : function (...args) {
          return interop.call(trampoline, ref, args);
        };
    Object.defineProperty(wrapper, "length", { value: length });
    wrapper.dartFunction = ref;
    wrapper[this.wrappedDartFunction] = true;
    return wrapper;
  }

  /** The argument `i` of the innermost callback (the snippet makes its handle). */
  argument(i) {
    return this.frames[this.frames.length - 1][i];
  }

  /** Runs `body`, a call into the module, counting it. */
  enter(body) {
    this.depth++;
    try {
      return body();
    } finally {
      this.depth--;
    }
  }

  call(trampoline, ref, args) {
    const turn = this.depth === 0;
    this.frames.push(args);
    let status;
    try {
      status = this.enter(() => this.exports.dartforge_js_call(trampoline, ref.id, args.length));
    } finally {
      this.frames.pop();
    }
    // The handle is read before anything else runs in the module.
    const value = status >= 0 ? this.values[status] : undefined;
    // On the host's own turn, the program's events that became due run now
    // (microtasks, timers), as dart2wasm's run after the callback.
    if (turn && this.program !== null && !this.program.ended) {
      let next;
      try {
        next = this.enter(() => this.exports.dartforge_js_drain());
      } catch (e) {
        this.program.fail(e);
        throw e;
      }
      this.program.settle(next >= 0 ? -1 : next === -1 ? -2 : 0);
    }
    if (status < 0) throw this.dartError(-status - 1);
    return value;
  }

  dartError(id) {
    const error = new DartError(this, id);
    this.errors.register(error, id, error);
    return error;
  }

  /**
   * A snippet threw: a Dart exception that went through JavaScript becomes
   * pending again; any other JavaScript exception becomes the Dart side's
   * (`_JavaScriptError`). The module's exit and the engine's traps go on up.
   */
  fail(error) {
    if (error instanceof ModuleExit || error instanceof WebAssembly.RuntimeError) throw error;
    if (error instanceof DartError) {
      const id = error.consume();
      if (id !== null) {
        this.exports.dartforge_js_rethrow(id);
        return 0;
      }
    }
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
      // A value that is not a string reads as `String(v)` (dart2wasm keeps the
      // JavaScript value and prints it as such).
      string_length: (handle) => {
        const s = this.text(handle);
        for (let i = 0; i < s.length; i++) {
          if (s.charCodeAt(i) > 0xff) return -s.length - 1;
        }
        return s.length;
      },
      string_read: (handle, offset, two) => {
        const s = this.text(handle);
        const view = units(offset, s.length, two !== 0);
        for (let i = 0; i < s.length; i++) view[i] = s.charCodeAt(i);
      },
      ...companion.snippets(this),
    };
  }
}

/**
 * The options the host itself answers when the caller does not pass them:
 * `DARTFORGE_URI_BASE`, the `Uri.base` of the module — the page's URL on each
 * call, as dart2wasm reads it (`globalThis.location.href`; none in Node).
 */
function hostOption(key) {
  if (key === "DARTFORGE_URI_BASE" && globalThis.location != null) return globalThis.location.href;
  return undefined;
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
 * module writes. Resolves to `{ sha256, status, trap, waiting, finished,
 * interop }`:
 * `status` is the program's exit status, `trap` the engine error when it did
 * not return. With `waiting`, the program has no event of its own left but
 * JavaScript still holds its callbacks: it goes on when they are called, and
 * `finished` resolves to `{ status }` when it ends (an unhandled exception in a
 * callback's turn ends it through `ModuleExit`; a callback's own exception
 * reaches the host as a `DartError`, which `report()` ends). Rejects only when the module
 * does not run at all (compile, link or ABI).
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
        const value = Object.hasOwn(options, key) ? options[key] : hostOption(key);
        if (value === undefined || value === null) return -1;
        const bytes = new TextEncoder().encode(String(value));
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
  // The engine stack the module's frames may take: V8's (Node and Chrome).
  if (typeof exports.dartforge_engine_stack === "function") exports.dartforge_engine_stack(V8_STACK_BYTES);
  // `-1`: the program waits for a timer; it resumes after the delay it asks
  // for, from the event loop, without blocking the page.
  const program = new Program(exports, interop !== null ? (body) => interop.enter(body) : (body) => body());
  if (interop !== null) interop.program = program;
  const finished = program.finished;
  try {
    let status;
    try {
      status = program.enter(() => exports.dartforge_run());
    } catch (e) {
      program.fail(e);
      throw e;
    }
    program.settle(status);
    const first = await program.first;
    return { sha256, status: first.status, trap: null, waiting: first.waiting, finished, interop };
  } catch (e) {
    if (e instanceof ModuleExit) return { sha256, status: e.status, trap: null, waiting: false, finished, interop };
    return { sha256, status: null, trap: String(e), waiting: false, finished, interop };
  }
}

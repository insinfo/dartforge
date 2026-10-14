
globalThis.self = globalThis;
let reads = 0;
const mode = process.argv[3];
globalThis.installBufferProbe = function() {
  const p = Uint8Array.prototype;
  const t = Object.getPrototypeOf(p);
  const get = Object.getOwnPropertyDescriptor;
  const apply = Reflect.apply;
  if (mode === 'buffer' || mode === 'byteOffset' || mode === 'byteLength' || mode === 'length') {
    const d = get(t, mode);
    Object.defineProperty(p, mode, {configurable: true, get() { reads++; return apply(d.get, this, []); }});
  } else if (mode === 'baseBuffer') {
    const d = get(t, 'buffer');
    Object.defineProperty(t, 'buffer', {...d, get() { reads++; return apply(d.get, this, []); }});
  } else if (mode === 'resizable') {
    const d = get(ArrayBuffer.prototype, 'resizable');
    if (d) Object.defineProperty(ArrayBuffer.prototype, 'resizable', {...d, get() { reads++; return apply(d.get, this, []); }});
  } else if (mode === 'descriptor') {
    Object.getOwnPropertyDescriptor = function(...args) { reads++; return apply(get, Object, args); };
  } else if (mode === 'isView') {
    const f = ArrayBuffer.isView;
    ArrayBuffer.isView = function(...args) { reads++; return apply(f, ArrayBuffer, args); };
  }
};
globalThis.bufferReads = () => reads;
require(process.argv[2]);

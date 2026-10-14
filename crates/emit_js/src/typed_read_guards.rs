//! Private captured intrinsics for certifying a pure typed-array read loop.

use crate::module::ModState;

pub(crate) fn name(library: u32) -> String {
    format!("t$typedReadGuard{library}")
}

pub(crate) fn emit(m: &ModState, out: &mut String) {
    for &library in m.typed_read_guard_libraries.borrow().iter() {
        out.push_str(&format!("const {} = {};\n", name(library), GUARD));
    }
}

// Descriptor inspection precedes any accessor invocation. Captured native
// getters cannot be replaced by application callbacks while the loop executes.
const GUARD: &str = r#"(function() {
  const apply = Reflect.apply;
  const descriptor = Object.getOwnPropertyDescriptor;
  const prototype = Object.getPrototypeOf;
  const hasOwn = Object.prototype.hasOwnProperty;
  const isView = ArrayBuffer.isView;
  const text = Function.prototype.toString;
  const fail = function() { return -1; };
  const owns = (o, k) => apply(hasOwn, o, [k]);
  const data = (o, k) => { const d = descriptor(o, k); return d && owns(d, 'value') ? d.value : null; };
  const getter = (o, k) => { const d = descriptor(o, k); return d && owns(d, 'get') ? d.get : null; };
  const nativeGetter = (o, k) => {
    const g = getter(o, k);
    return typeof g === 'function' && apply(text, g, []) === 'function get ' + k + '() { [native code] }' ? g : null;
  };
  try {
    const constructors = ['Float32Array', 'Float64Array', 'Int8Array', 'Uint8Array', 'Uint8ClampedArray', 'Int16Array', 'Uint16Array', 'Int32Array', 'Uint32Array'];
    const brands = [];
    for (let i = 0; i < constructors.length; i++) {
      const c = data(globalThis, constructors[i]);
      brands[i] = typeof c === 'function' && apply(text, c, []) === 'function ' + constructors[i] + '() { [native code] }' ? data(c, 'prototype') : null;
    }
    if (!brands[3]) return fail;
    const typed = prototype(brands[3]);
    const keys = ['length', 'buffer', 'byteOffset', 'byteLength'];
    const getters = [];
    for (let i = 0; i < keys.length; i++) {
      getters[i] = nativeGetter(typed, keys[i]);
      if (!getters[i]) return fail;
    }
    const ab = data(ArrayBuffer, 'prototype');
    const byteLength = nativeGetter(ab, 'byteLength');
    const resizableDescriptor = descriptor(ab, 'resizable');
    const resizable = resizableDescriptor ? nativeGetter(ab, 'resizable') : null;
    if (!byteLength || (resizableDescriptor && !resizable)) return fail;
    return function(value, brand) {
      try {
        const p = brands[brand];
        if (!p || !isView(value) || prototype(value) !== p || prototype(p) !== typed) return -1;
        for (let i = 0; i < keys.length; i++) {
          const k = keys[i];
          if (owns(value, k) || descriptor(p, k) || getter(typed, k) !== getters[i]) return -1;
        }
        if (getter(ab, 'byteLength') !== byteLength || getter(ab, 'resizable') !== resizable) return -1;
        const buffer = apply(getters[1], value, []);
        if (prototype(buffer) !== ab || owns(buffer, 'byteLength') || owns(buffer, 'resizable')) return -1;
        // This native getter rejects SharedArrayBuffer without public instanceof hooks.
        apply(byteLength, buffer, []);
        if (resizable && apply(resizable, buffer, [])) return -1;
        return apply(getters[0], value, []);
      } catch (_) { return -1; }
    };
  } catch (_) { return fail; }
})()"#;

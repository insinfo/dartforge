// Cópia mínima de package:js 0.6.7 (as anotações que o DDC reconhece pela
// biblioteca `package:js`).
library js;

export 'dart:js' show allowInterop, allowInteropCaptureThis;

class JS {
  final String? name;
  const JS([this.name]);
}

class _Anonymous {
  const _Anonymous();
}

class _StaticInterop {
  const _StaticInterop();
}

const _Anonymous anonymous = _Anonymous();
const _StaticInterop staticInterop = _StaticInterop();

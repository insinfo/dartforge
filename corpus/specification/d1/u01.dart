const a = true && ('x'.length ~/ 0 == 1);
const b = false || 'a'.isEmpty;
const c = null ?? 'a'.isEmpty;
class A { const A(bool b) : assert(b && 'a'.isEmpty); }
const d = const A(true);

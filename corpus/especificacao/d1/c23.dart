var v = 1;
class A { const A(Object? x, {Object? n}); A.nc(); }
const a = const A(v);
const b = const A(1, n: v);
const c = A.nc();
const d = const A.nc();
const e = const A(const A(v));
var f = const A(v);
final g = const A(1 ~/ 0);

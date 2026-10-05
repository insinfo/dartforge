class A {
  final int f;
  const A(this.f);
}
const a = const A(1).f;
const o = const A(1);
const b = o.f;
int get x => 1;
var v = 0;
const c = x;
const d = v;

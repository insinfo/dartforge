class A {
  final m = const A();
  const A();
}
class C {
  final x;
  const C() : x = y;
}
const y = const C();
class R { const R() : this.a(); const R.a() : this(); }
class S { const S([x = const S()]); }
class T { const T() : this(); }

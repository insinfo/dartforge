class A { const A(); }
const a = const A()
  ..toString();
const w2 = w;
const w = const R();
class R { const R() : this.a(); const R.a() : this(); }
const w3 = const R();

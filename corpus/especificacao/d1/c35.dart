class S {
  final int a;
  const S({this.a = 0});
}
class B extends S {
  const B({super.a});
}
const b = const B();
class P { final int q; const P([this.q = 'x' as dynamic]); }
const p = const P();
class R { final String q; const R([this.q]); }
const r = const R();

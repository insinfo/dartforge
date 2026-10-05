mixin M {}
class S { final int x; const S(this.x) : assert(x > 0); }
class C = S with M;
const c = const C(0);
class D extends S with M { const D() : super(-1); }
const d = const D();

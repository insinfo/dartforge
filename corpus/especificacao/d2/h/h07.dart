class S {
  const S();
  const S.n(int x);
  S.nc();
}
mixin M0 {}
mixin MF { final int f = 0; }
mixin MS { static int s = 0; }
mixin MG { int get g => 0; }
class A0 = S with M0;
class AF = S with MF;
class AS = S with MS;
class AG = S with MG;
class A2 = S with M0, MF;
const a = A0();
const b = AF();
const c = AS();
const d = AG();
const e = A2.n(1);
var f = const A0.nc();
var g = const AF.n(1);

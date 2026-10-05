class A {
  var x;
  const A(this.x);
  const A.n() : x = 0, super();
}
abstract class B {
  abstract int y;
  const B();
}
class C {
  static var s;
  final f = 0;
  int get g => 0;
  set g(int v) {}
  const C();
}
class D extends C {
  var z;
  const D() : super();
}
mixin M on C {
  var q;
}
class F extends C with M {
  const F();
}
class G = C with M;
class H extends G {
  const H();
}

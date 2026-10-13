import os,io
base=os.path.dirname(os.path.abspath(__file__))
C={}
C['u3/e08_auto_referencia.dart']=r"""class _C { _C? x; static _C make() => _C(); }
mixin _M { _M? x; }
enum _E { a; _E get me => this; }
extension type _T(int v) { _T get me => this; }
typedef _F = void Function(_F? f);
"""
C['u3/e09_exec_envolvente.dart']=r"""void _viaLocal() { void g() { _viaLocal(); } g(); }
void _viaClosure() { var c = () => _viaClosure(); c(); }
int get _g => _g;
set _s(int v) { _s = v; }
class A {
  int _f = 0;
  void _m() { () { _m(); }(); }
  void _n() { void h() { _n(); } h(); }
  void inc() { _f = _f + 1; }
  int get _p => _p;
}
"""
C['u3/e18_parametros_chamadas.dart']=r"""class _C {
  _C([int? a]);
  _C.r() : this(1);
  _C.s([int? b]);
  _C.t() : this.s(2);
  const _C.k([int? c]);
}
class _D extends _C {
  _D() : super.s(3);
}
void _f([int? a]) {}
void _g([int? a]) {}
void _h([int? a]) {}
class _An { const _An([int? x]); }
@_An(1)
void main() {
  _C.r(); _C.t(); _D();
  (_f)(1);
  _g.call(1);
  var h = _h;
  h(1);
  void _local([int? a]) {}
  void local([int? a]) {}
  _local(); local();
  void cb(void Function([int? z]) p) {}
  cb(([z]) {});
}
"""
C['u3/e21_comando_identificador.dart']=r"""void f() {
  var x = 0;
  x;
  var y = 0;
  (y);
  var z = 0;
  z = z;
  var w = 0;
  w.toString;
  var k = 0;
  k++;
  var j = 0;
  print(j++);
  var n = 0;
  n = n + 1;
}
"""
C['u3/e22_ignore.dart']=r"""// ignore_for_file: unused_field
import 'dart:math'; // ignore: unused_import
class A { int _f = 0; }
void f() {
  // ignore: unused_local_variable
  var x = 0;
  var y = 0; // ignore: unused_element
}
"""
C['parte2/lib.dart']=r"""import 'dart:math';
part 'p.dart';
"""
C['parte2/p.dart']=r"""part of 'lib.dart';
void g() { naoExiste; }
"""
C['parte3/lib.dart']=r"""import 'dart:math';
import 'dart:collection' as col;
part 'p.dart';
"""
C['parte3/p.dart']=r"""part of 'lib.dart';
void g(col.Queue<int> q) { print(pi); }
"""
C['d3/lib_dep.dart']=r"""@deprecated
library;
void f() { velha(); }
@deprecated
void velha() {}
"""
C['d3/campos.dart']=r"""class A {
  @deprecated
  int a = 0, b = 0;
  @deprecated
  A.n();
  A() : this.n();
  void m() { print(a + b); }
}
class B extends A {
  B() : super.n();
  @deprecated
  void old(int a) { old(1); }
}
enum E { @deprecated x, y }
void g(Object o) {
  print(E.x);
  if (o case A(a: var q)) { print(q); }
  if (o case A(:var b)) { print(b); }
}
"""
for n,s in C.items():
    p=os.path.join(base,n); os.makedirs(os.path.dirname(p),exist_ok=True)
    io.open(p,'w',encoding='utf-8',newline='\n').write(s)
print(len(C),'arquivos')

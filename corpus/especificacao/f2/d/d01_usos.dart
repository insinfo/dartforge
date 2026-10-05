import 'd_lib.dart';
import 'd_lib.dart' as p show velha;
import 'd_lib.dart' as h hide velha;
void f(C c, Velha v) {
  velha();
  comMensagem();
  comPonto();
  comVazia();
  C();
  C.n();
  c.campo;
  c.campo = 1;
  c.campo += 1;
  c.g;
  c.s = 1;
  c + 1;
  c();
  c.m(p: 1, q: 2);
  print(velhaConst);
  print(velha);
  p.velha();
  print(C.new);
}
@deprecated
void g() { velha(); }
@deprecated
class D { void m() { velha(); } }
/// [velha] em comentário.
void h() {}

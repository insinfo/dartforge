import 'b.dart' as b;
class A {}
void f(A x) {}
void g(b.A y) { f(y); }

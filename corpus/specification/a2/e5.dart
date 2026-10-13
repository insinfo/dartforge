class A { A(int a, [int? b]); A.n({int? x}); }
class B extends A { B(super.a) : super(0); B.k(super.a, super.b) : super(1); B.j(int a, {super.x}) : super.n(); B.ok(super.a); }
extension type E(int it) { E.n(super.it); E.m({super.x}) : it = 0; }
enum G<T extends G<T>> { a }
enum H<T extends num> { h }
void f(List<int> l) { for (const x in l) {} for (const int y in 3) {} }

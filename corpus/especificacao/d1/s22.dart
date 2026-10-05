void f({int x = X}) {}
void g([int x = X]) {}
class E { const E(int i); }
extension type ET(int i) { const ET.c(this.i); }
class A { const A({ET a = const ET.c(0)}); }

extension type A(int it) { void foo() {} }
extension type B(int it) { void foo() {} }
extension type C(int it) implements A, B {}
extension type D(int it) implements A, B { void foo() {} }
class K { void bar() {} }
extension type F(K it) { void bar() {} }
extension type H(K it) implements K, F {}
extension type N1(Never it) {}
extension type N2(Never? it) {}
extension type N3<T extends Never>(T it) {}

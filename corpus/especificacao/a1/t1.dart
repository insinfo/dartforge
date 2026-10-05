class A<T extends num> {}
class B<T extends B<T>> {}
typedef F<T extends num> = void Function(T);
class C extends A<String> {}
A<String>? a1;
A<dynamic>? a2;
A<Object?>? a3;
A<Never>? a4;
B? b1;
B<B>? b2;
B<Object?>? b3;
var n1 = new A<String>();
var n2 = A<dynamic>();
var n3 = new B();
F<String>? f1;
void g<T extends num>() {}
void h() { g<String>(); g<dynamic>(); var l = <A<String>>[]; print(l); print(A<String>); }
A<String, int>? w1;
A<Undefined>? w2;

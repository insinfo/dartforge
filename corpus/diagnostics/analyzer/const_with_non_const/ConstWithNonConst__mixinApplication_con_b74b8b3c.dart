mixin M {}
class A {
  const A();
}
class B = A with M;
const b = const B();

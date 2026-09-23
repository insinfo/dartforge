class A {
  factory A() => throw '';
}
class B extends A {
  factory B() => throw '';
  factory B.second() => throw '';
}

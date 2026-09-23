class A {}
class B extends A {
  B() : super(), super() {}
//               ^^^^^^^
// [diag.multipleSuperInitializers] A constructor can have at most one 'super' initializer.
}

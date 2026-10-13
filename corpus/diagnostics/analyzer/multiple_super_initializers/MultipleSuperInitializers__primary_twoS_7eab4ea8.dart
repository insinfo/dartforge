class A {}
class B() extends A {
  this : super(), super();
//                ^^^^^
// [diag.multipleSuperInitializers] A constructor can have at most one 'super' initializer.
}

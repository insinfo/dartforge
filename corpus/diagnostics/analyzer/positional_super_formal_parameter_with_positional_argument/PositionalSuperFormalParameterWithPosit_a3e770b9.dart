class A {
  A(int a, int b);
}

class B(super.a) extends A {
//            ^
// [diag.positionalSuperFormalParameterWithPositionalArgument] Positional super parameters can't be used when the super constructor invocation has a positional argument.
  this : super(0);
}

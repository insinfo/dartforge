class A {
  A(int a, int b);
}

class B extends A {
  B(super.b) : super(0);
//        ^
// [diag.positionalSuperFormalParameterWithPositionalArgument] Positional super parameters can't be used when the super constructor invocation has a positional argument.
}

class A {
  A({required int a});
}

class B extends A {
  B({required super.a}) : super(a: 0);
//                              ^
// [diag.duplicateNamedArgument] The argument for the named parameter 'a' was already specified.
}

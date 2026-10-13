class A<T extends Object?> {
  void foo([T a]) {}
//            ^
// [diag.missingDefaultValueForParameterPositional] The parameter 'a' can't have a value of 'null' because of its type, but the implicit default value is 'null'.
}

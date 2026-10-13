class A {
  int operator +(int _) => 0;
//             ^
// [context 1] The first definition of this name.
}

augment class A {
  int operator +(int _) => 0;
//             ^
// [diag.duplicateDefinition][context 1] The name '+' is already defined.
}

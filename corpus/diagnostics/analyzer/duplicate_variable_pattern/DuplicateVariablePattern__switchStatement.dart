void f(int x) {
  switch (x) {
    case var a && var a:
//           ^
// [context 1] The first definition of this name.
//                    ^
// [diag.duplicateVariablePattern][context 1] The variable 'a' is already defined in this pattern.
      a;
  }
}

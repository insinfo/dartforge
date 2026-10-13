main() async {
  late var v = await 42;
//             ^^^^^
// [diag.awaitInLateLocalVariableInitializer] The 'await' expression can't be used in a 'late' local variable's initializer.
  print(v);
}

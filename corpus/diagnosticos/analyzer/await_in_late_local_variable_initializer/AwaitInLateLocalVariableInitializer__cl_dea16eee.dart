main() {
  var v = () async {
    late var v2 = await 42;
//                ^^^^^
// [diag.awaitInLateLocalVariableInitializer] The 'await' expression can't be used in a 'late' local variable's initializer.
    print(v2);
  };
  print(v);
}

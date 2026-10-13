Stream<int> f() async* {
  var a = <String>[];
  yield* a;
//       ^
// [diag.yieldEachOfInvalidType] The type 'List<String>' implied by the 'yield*' expression must be assignable to 'Stream<int>'.
}

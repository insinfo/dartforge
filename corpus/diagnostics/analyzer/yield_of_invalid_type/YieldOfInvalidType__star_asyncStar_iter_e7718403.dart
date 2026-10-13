f() async* {
  var a = <int>[];
  yield* a;
//       ^
// [diag.yieldEachOfInvalidType] The type 'List<int>' implied by the 'yield*' expression must be assignable to 'Stream<dynamic>'.
}

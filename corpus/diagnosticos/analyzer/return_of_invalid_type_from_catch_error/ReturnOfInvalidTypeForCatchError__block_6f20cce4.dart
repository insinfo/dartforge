void f(Future<int> future) {
  future.catchError((e, st) {
    if (1 == 2) {
      return 7;
    } else {
      return 0.5;
//           ^^^
// [diag.returnOfInvalidTypeFromCatchError] A value of type 'double' can't be returned by the 'onError' handler because it must be assignable to 'FutureOr<int>'.
    }
  });
}

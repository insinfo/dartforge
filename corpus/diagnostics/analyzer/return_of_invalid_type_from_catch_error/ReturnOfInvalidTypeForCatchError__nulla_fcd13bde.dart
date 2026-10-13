void f(Future<int?> future) {
  future.catchError((e, st) => '');
//                             ^^
// [diag.returnOfInvalidTypeFromCatchError] A value of type 'String' can't be returned by the 'onError' handler because it must be assignable to 'FutureOr<int?>'.
}

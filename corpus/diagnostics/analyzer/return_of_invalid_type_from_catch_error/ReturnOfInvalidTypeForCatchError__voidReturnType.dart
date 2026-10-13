void f(Future<int> future, void Function() g) {
  future.catchError((e, st) => g());
//                             ^^^
// [diag.returnOfInvalidTypeFromCatchError] A value of type 'void' can't be returned by the 'onError' handler because it must be assignable to 'FutureOr<int>'.
}

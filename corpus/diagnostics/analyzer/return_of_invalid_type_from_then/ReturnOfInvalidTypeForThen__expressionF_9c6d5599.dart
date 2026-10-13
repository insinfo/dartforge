void f(Future<int> future) {
  future.then((_) => 0, onError: (e, st) => 'c');
//                                          ^^^
// [diag.returnOfInvalidTypeFromThen] A value of type 'String' can't be returned by the 'onError' handler because it must be assignable to 'FutureOr<int>', as required by 'Future.then'.
}

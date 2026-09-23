void f(Future<int> future, int? Function(dynamic, StackTrace) cb) {
  future.then((_) => 1, onError: cb);
//                               ^^
// [diag.returnTypeInvalidForThen] The return type 'int?' isn't assignable to 'FutureOr<int>', as required by 'Future.then'.
}

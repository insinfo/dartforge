void f(Future<int> future, double Function(dynamic, StackTrace) callback) {
  future.then((_) => 0, onError: callback);
//                               ^^^^^^^^
// [diag.returnTypeInvalidForThen] The return type 'double' isn't assignable to 'FutureOr<int>', as required by 'Future.then'.
}

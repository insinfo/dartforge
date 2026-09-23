void f(Future<int> future, String Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
//                  ^^
// [diag.returnTypeInvalidForCatchError] The return type 'String' isn't assignable to 'FutureOr<int>', as required by 'Future.catchError'.
}

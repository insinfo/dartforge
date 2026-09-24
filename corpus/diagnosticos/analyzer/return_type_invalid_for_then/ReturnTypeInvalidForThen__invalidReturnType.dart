void f(Future<int> future, String Function(dynamic, StackTrace) cb) {
  future.then<int>((_) => 1, onError: cb);
//                                    ^^
// [diag.returnTypeInvalidForThen] The return type 'String' isn't assignable to 'FutureOr<int>', as required by 'Future.then'.
}

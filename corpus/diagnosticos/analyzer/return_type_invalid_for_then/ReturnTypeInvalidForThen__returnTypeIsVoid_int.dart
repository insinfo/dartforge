void f(Future<int> future, void Function(dynamic, StackTrace) cb) {
  future.then<int>((_) => 1, onError: cb);
//                                    ^^
// [diag.returnTypeInvalidForThen] The return type 'void' isn't assignable to 'FutureOr<int>', as required by 'Future.then'.
}

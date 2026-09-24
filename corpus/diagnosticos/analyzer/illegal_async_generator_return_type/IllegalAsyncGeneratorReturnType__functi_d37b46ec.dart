abstract class SubStream<T> implements Stream<T> {}
SubStream<int> f() async* {}
// [diag.illegalAsyncGeneratorReturnType][column 1][length 14] Functions marked 'async*' must have a return type that is a supertype of 'Stream<T>' for some type 'T'.

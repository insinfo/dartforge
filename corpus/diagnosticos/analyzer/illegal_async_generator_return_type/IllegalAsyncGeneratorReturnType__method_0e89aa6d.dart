abstract class SubStream<T> implements Stream<T> {}
class C {
  SubStream<int> f() async* {}
//^^^^^^^^^^^^^^
// [diag.illegalAsyncGeneratorReturnType] Functions marked 'async*' must have a return type that is a supertype of 'Stream<T>' for some type 'T'.
}

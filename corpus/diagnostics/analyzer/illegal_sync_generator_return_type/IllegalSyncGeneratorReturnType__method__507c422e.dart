abstract class SubIterator<T> implements Iterator<T> {}
class C {
  SubIterator<int> f() sync* {}
//^^^^^^^^^^^^^^^^
// [diag.illegalSyncGeneratorReturnType] Functions marked 'sync*' must have a return type that is a supertype of 'Iterable<T>' for some type 'T'.
}

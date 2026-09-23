extension type A<T>(T it) {}

void m() {
  void local<U extends A<U>>() {}
//           ^
// [diag.typeParameterSupertypeOfItsBound] 'U' can't be a supertype of its upper bound.
  local;
}

void m() {
  void local<T extends T>() {}
//           ^
// [diag.typeParameterSupertypeOfItsBound] 'T' can't be a supertype of its upper bound.
  local;
}

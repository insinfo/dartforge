enum E { ONE }
E e(String name) {
  return new E();
//           ^
// [diag.instantiateEnum] Enums can't be instantiated.
}

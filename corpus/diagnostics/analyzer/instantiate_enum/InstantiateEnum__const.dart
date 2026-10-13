enum E { ONE }
E e(String name) {
  return const E();
//             ^
// [diag.instantiateEnum] Enums can't be instantiated.
}

enum E<T> {
  v<T>();
//  ^
// [diag.typeParameterReferencedByStatic] Static members can't reference type parameters of the class.
}

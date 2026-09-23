extension type E(int it) {
  int get hashCode => 0;
//        ^^^^^^^^
// [diag.extensionTypeDeclaresMemberOfObject] Extension types can't declare members with the same name as a member declared by 'Object'.
}

void f<T>(T a) {}
class A<U> {}
extension<U> on A<U> {
  final x = f<U>;
//      ^
// [diag.extensionDeclaresInstanceField] Extensions can't declare instance fields.
}

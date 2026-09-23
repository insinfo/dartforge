class A<T> {
  void foo({T a}) native;
//                ^^^^^^^
// [diag.nativeFunctionBodyInNonSdkCode] Native functions can only be declared in the SDK and code that is loaded through native extensions.
}

mixin A {
  static int foo(_) native 'string';
//                  ^^^^^^^^^^^^^^^^
// [diag.nativeFunctionBodyInNonSdkCode] Native functions can only be declared in the SDK and code that is loaded through native extensions.
}

import '' deferred as self;
//     ^^
// [diag.deferredImportOfExtension] Deferred library imports must hide all extension declarations.
extension E on int {
  static int f(String s) => 7;
}
const g = self.E.f;
//             ^
// [diag.constInitializedWithNonConstantValueFromDeferredLibrary] Constant values from a deferred library can't be used to initialize a 'const' variable.

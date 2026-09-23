// %before-language-feature: primary-constructors
class C {
  external factory();
//         ^^^^^^^
// [diag.deprecatedFactoryMethod] Methods named 'factory' will become constructors when the primary_constructors feature is enabled.
}

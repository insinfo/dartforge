// %before-language-feature: primary-constructors
class C {
  factory() => throw 0;
//^^^^^^^
// [diag.deprecatedFactoryMethod] Methods named 'factory' will become constructors when the primary_constructors feature is enabled.
}

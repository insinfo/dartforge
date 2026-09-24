// %before-language-feature: class-modifiers
class A extends Object with Function {}
//                          ^^^^^^^^
// [diag.deprecatedMixinFunction] Mixing in 'Function' is deprecated.

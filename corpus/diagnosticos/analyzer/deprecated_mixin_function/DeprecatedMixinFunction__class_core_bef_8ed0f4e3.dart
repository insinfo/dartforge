// %before-language-feature: class-modifiers
typedef F = Function;
class A extends Object with F {}
//                          ^
// [diag.deprecatedMixinFunction] Mixing in 'Function' is deprecated.

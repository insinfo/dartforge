// %before-language-feature: class-modifiers
typedef F = Function;
class A = Object with F;
//                    ^
// [diag.deprecatedMixinFunction] Mixing in 'Function' is deprecated.

// %before-language-feature: class-modifiers
mixin M {}
typedef F = Function;
class A = Object with M implements F;
//                                 ^
// [diag.deprecatedImplementsFunction] Implementing 'Function' has no effect.

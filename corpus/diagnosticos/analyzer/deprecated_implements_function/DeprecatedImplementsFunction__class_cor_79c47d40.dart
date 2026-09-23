// %before-language-feature: class-modifiers
typedef F = Function;
class A implements F {}
//                 ^
// [diag.deprecatedImplementsFunction] Implementing 'Function' has no effect.

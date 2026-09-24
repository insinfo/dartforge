// %before-language-feature: class-modifiers
mixin M {}
class A = Object with M implements Function;
//                                 ^^^^^^^^
// [diag.deprecatedImplementsFunction] Implementing 'Function' has no effect.

// %before-language-feature: class-modifiers
class A implements Function, Function {}
//                 ^^^^^^^^
// [diag.deprecatedImplementsFunction] Implementing 'Function' has no effect.
//                           ^^^^^^^^
// [diag.implementsRepeated] 'Function' can only be implemented once.

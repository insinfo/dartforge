// %before-language-feature: class-modifiers
typedef F = Function;
class A extends F {}
//              ^
// [diag.deprecatedExtendsFunction] Extending 'Function' is deprecated.

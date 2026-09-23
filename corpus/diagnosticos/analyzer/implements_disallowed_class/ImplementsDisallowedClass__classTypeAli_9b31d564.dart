// %before-language-feature: enhanced-enums
mixin M {}
abstract class A = Object with M implements Enum;
//                                          ^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'Enum'.

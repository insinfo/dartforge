// %before-language-feature: enhanced-enums
abstract class A implements Enum {}
//                          ^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'Enum'.

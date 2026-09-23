abstract class A with Enum {}
//                    ^^^^
// [diag.classUsedAsMixin] The class 'Enum' can't be used as a mixin because it's neither a mixin class nor a mixin.
abstract class B = Object with Enum;
//                             ^^^^
// [diag.classUsedAsMixin] The class 'Enum' can't be used as a mixin because it's neither a mixin class nor a mixin.

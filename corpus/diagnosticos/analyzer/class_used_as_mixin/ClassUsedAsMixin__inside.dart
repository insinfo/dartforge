class Foo {}
class Bar with Foo {}
//             ^^^
// [diag.classUsedAsMixin] The class 'Foo' can't be used as a mixin because it's neither a mixin class nor a mixin.

// %before-language-feature: class-modifiers
class A {
  A();
}
class B = Object with A;
//                    ^
// [diag.classUsedAsMixinDeclaresGenerativeConstructor] The class 'A' can't be used as a mixin because it declares a generative constructor.

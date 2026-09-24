void f([int x]);

augment void f([int x = 0]);
//                  ^
// [context 1] The previous formal parameter with default value is here.

augment void f([int x = 1]) {}
//                    ^
// [diag.defaultValueAlreadySpecifiedInAugmentationChain][context 1] The default value for this optional parameter was already specified in the augmentation chain.

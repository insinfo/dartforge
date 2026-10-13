int get foo => 0;
//      ^^^
// [context 1] The complete declaration is here.
set foo(int _) {}
//  ^^^
// [context 2] The complete declaration is here.
augment int foo = 1;
//          ^^^
// [diag.augmentationInducedGetterAlreadyComplete][context 1] The getter induced by this augmentation is complete, but the getter being augmented is already complete.
// [diag.augmentationInducedSetterAlreadyComplete][context 2] The setter induced by this augmentation is complete, but the setter being augmented is already complete.

int get foo => 0;
//      ^^^
// [context 1] The complete declaration is here.
augment final int foo = 1;
//                ^^^
// [diag.augmentationInducedGetterAlreadyComplete][context 1] The getter induced by this augmentation is complete, but the getter being augmented is already complete.

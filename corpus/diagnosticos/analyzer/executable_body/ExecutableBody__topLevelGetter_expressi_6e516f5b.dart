int get foo => 0;
//      ^^^
// [context 1] The corresponding getter is declared here.
// [context 2] The complete declaration is here.
augment int foo = 1;
//          ^^^
// [diag.augmentationWithoutSetterDeclaration][context 1] This augmentation induces a setter, but no setter declaration named 'foo' exists to augment.
// [diag.augmentationInducedGetterAlreadyComplete][context 2] The getter induced by this augmentation is complete, but the getter being augmented is already complete.

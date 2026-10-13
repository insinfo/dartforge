typedef IntAlias = int;

void foo() {}

augment IntAlias foo();
//      ^^^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'IntAlias' must be the same as the introductory declaration's return type 'void'.

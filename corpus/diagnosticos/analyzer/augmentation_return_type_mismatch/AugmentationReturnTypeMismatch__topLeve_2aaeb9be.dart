dynamic foo() => null;

augment Object? foo();
//      ^^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'Object?' must be the same as the introductory declaration's return type 'dynamic'.

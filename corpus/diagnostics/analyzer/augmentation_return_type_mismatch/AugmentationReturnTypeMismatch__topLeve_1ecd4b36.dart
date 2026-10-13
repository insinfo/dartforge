Object? foo() => null;

augment dynamic foo();
//      ^^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'dynamic' must be the same as the introductory declaration's return type 'Object?'.

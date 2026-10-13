abstract final int foo;
augment const int foo = 0;
//                ^^^
// [diag.constantVariableAugmentation] Variable augmentations can't be const.

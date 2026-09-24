int get foo => 0;
//      ^^^
// [context 1] The complete declaration is here.
augment int get foo => 1;
// [diag.functionAlreadyComplete][column 1][length 7][context 1] The augmentation can't provide a body because the function or member is already complete.

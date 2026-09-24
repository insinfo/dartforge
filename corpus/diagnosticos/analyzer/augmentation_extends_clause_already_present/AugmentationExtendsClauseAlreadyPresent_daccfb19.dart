class A {}

class B extends A {}
//    ^
// [context 1] The declaration being augmented.
augment class B extends A {}
//              ^^^^^^^
// [diag.augmentationExtendsClauseAlreadyPresent][context 1] The augmentation has an 'extends' clause, but an augmentation target already includes an 'extends' clause and it isn't allowed to be repeated or changed.

int get foo => 0;

set foo(String _) {}

augment abstract var foo;
//                   ^^^
// [diag.augmentationVariableDifferentGetterSetterTypes] The getter and setter augmented by this variable have different types: 'int' and 'String'.

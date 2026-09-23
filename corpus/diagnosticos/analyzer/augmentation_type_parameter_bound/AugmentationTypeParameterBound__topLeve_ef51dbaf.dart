import 'dart:core';
import 'dart:core' as core;

void foo<T extends num>() {}
augment void foo<T extends core.num>();

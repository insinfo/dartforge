@DefaultAsset('bar')
library;

import 'dart:ffi';

@Native<Void Function()>()
external void foo();

@defaults
library;

import 'dart:ffi';

const defaults = DefaultAsset('foo');

@Native<Void Function()>()
external void foo();

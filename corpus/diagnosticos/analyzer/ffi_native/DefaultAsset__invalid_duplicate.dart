@DefaultAsset('foo')
@DefaultAsset('bar')
// [diag.ffiNativeInvalidDuplicateDefaultAsset][column 2][length 12] There may be at most one @DefaultAsset annotation on a library.
library;

import 'dart:ffi';

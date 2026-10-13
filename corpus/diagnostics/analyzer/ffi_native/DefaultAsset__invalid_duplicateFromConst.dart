@DefaultAsset('bar')
@defaults
// [diag.ffiNativeInvalidDuplicateDefaultAsset][column 2][length 8] There may be at most one @DefaultAsset annotation on a library.
library;

import 'dart:ffi';

const defaults = DefaultAsset('foo');

/// Optional runtime capabilities. Source remains valid on the official SDK.
library;

export 'src/native_pump.dart';
export 'src/annotations.dart';
export 'simd.dart';
export 'native_library.dart';
export 'resources.dart';
export 'memory.dart';
export 'callbacks.dart';
export 'gc.dart';
export 'isolate.dart';

/// Identifies compilation by DartForge, separately from runtime capabilities.
const bool isDartForge = bool.fromEnvironment('dartforge.compiler');

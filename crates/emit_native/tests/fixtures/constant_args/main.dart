// The shape of dart_ui's `PlatformBackendResolver.defaultBackends`: an
// optional parameter that every caller omits, defaulted from a platform
// constant, deciding which backends the program instantiates.
import 'dart:io';

abstract class Backend {
  String get name;
}

class WindowsBackend implements Backend {
  @override
  String get name => 'win32 backend';
}

class MacBackend implements Backend {
  @override
  String get name => 'macos backend';
}

class LinuxBackend implements Backend {
  @override
  String get name => 'linux backend';
}

List<Backend Function()> defaultBackends({String? operatingSystem}) {
  final String platform = operatingSystem ?? Platform.operatingSystem;
  return <Backend Function()>[
    if (platform == 'windows') WindowsBackend.new,
    if (platform == 'macos') MacBackend.new,
    if (platform == 'linux') LinuxBackend.new,
  ];
}

String separator({String? sep}) => sep ?? Platform.pathSeparator;

void main() {
  for (final create in defaultBackends()) {
    print(create().name);
  }
  print(separator() == '/' ? 'posix separator' : 'windows separator');
  print(Platform.lineTerminator.length);
}

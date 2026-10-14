import 'dart:io' as io;
import 'reexport.dart' as forwarded;

typedef Host = io.Platform;

int effects = 0;
class Platform {
  static final bool isWindows = mark(false);
  bool get isMacOS {
    effects++;
    return false;
  }
}
bool mark(bool value) {
  effects++;
  return value;
}
Platform receiver() {
  effects++;
  return Platform();
}

bool windowsBranch() => io.Platform.isWindows;
bool macBranch() => io.Platform.isMacOS;
bool linuxBranch() => io.Platform.isLinux;
bool androidBranch() => io.Platform.isAndroid;
bool iosBranch() => io.Platform.isIOS;
bool fuchsiaBranch() => io.Platform.isFuchsia;
bool forwardedFlag() => forwarded.Platform.isMacOS;
bool aliasFlag() => Host.isMacOS;
bool userFlag() => Platform.isWindows;
bool receiverFlag() => receiver().isMacOS;
bool conditionalReceiver(bool enabled) => (enabled ? receiver() : Platform()).isMacOS;

void main() {
  print([windowsBranch(), macBranch(), linuxBranch(), androidBranch(), iosBranch(), fuchsiaBranch()]);
  print(forwardedFlag());
  print(aliasFlag());
  print(userFlag());
  print(receiverFlag());
  print(conditionalReceiver(true));
  print(effects);
}

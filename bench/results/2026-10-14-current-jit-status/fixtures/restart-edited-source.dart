import 'dart:async';
import 'dart:io';
class State { String value = 'changed'; int turns = 0; String? added; }
final state = State();
String phase() => 'restarted';
String note() => state.added == null ? 'null-added' : state.added!;
void main() {
  Timer.periodic(const Duration(milliseconds: 200), (timer) {
    state.turns++;
    if (state.turns == 1) state.value = 'new-state';
    print('LIVE|${phase()}|${state.value}|${state.turns}|${note()}');
    if (File('/Volumes/ssd_dock/MyRustProjects/dartforge/target/current-jit-validation/restart-stop').existsSync()) timer.cancel();
  });
}

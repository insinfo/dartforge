void main() {
  final value = 1.25;
  final finalizer = Finalizer<String>((token) {});
  for (final action in <void Function()>[
    () => finalizer.attach(value, 'token'),
    () { WeakReference<double>(value); },
    () { Expando<int>()[value] = 1; },
  ]) {
    try { action(); print('accepted'); }
    on ArgumentError { print('ArgumentError'); }
  }
}

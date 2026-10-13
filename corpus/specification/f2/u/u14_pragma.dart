@pragma('vm:entry-point')
void _entrada() {}
@pragma('vm:prefer-inline')
void _outra() {}
@pragma('vm:entry-point')
class _C {
  @pragma('vm:entry-point')
  void _m() {}
  @pragma('vm:entry-point')
  int _campo = 0;
  int _campo2 = 0;
  void _m2() {}
}

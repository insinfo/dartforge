void f() {
  try {} catch (e) {}
  try {} catch (e, s) {}
  try {} on Exception catch (e) {}
  try {} on Exception catch (e, s) {}
  try {} on Exception catch (e, s) { print(e); }
  try {} on Exception catch (e, s) { print(s); }
  try {} on Exception catch (_) {}
  try {} catch (_, __) {}
}

import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// Componente que é acessor de valor por `providers:` (como o
/// custom-select do new_sali).
@Component(
  selector: 'h02-campo',
  template: '<span>campo</span>',
  providers: [
    ExistingProvider.forToken(ngValueAccessor, H02Campo),
  ],
)
class H02Campo implements ControlValueAccessor<Object?> {
  @Input()
  String rotulo = '';

  @override
  void writeValue(Object? value) {}

  @override
  void registerOnChange(ChangeFunction<Object?> fn) {}

  @override
  void registerOnTouched(TouchFunction fn) {}

  @override
  void onDisabledChanged(bool isDisabled) {}
}

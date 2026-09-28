import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// Componente com `@HostBinding` que é acessor de valor por `providers:` (o
/// `li-checkbox` do limitless_ui): o campo é a instância, sem `.instance`.
@Component(
  selector: 'j30-caixa',
  template: '<span>campo</span>',
  providers: [
    ExistingProvider.forToken(ngValueAccessor, J30Caixa),
  ],
)
class J30Caixa implements ControlValueAccessor<Object?> {
  @Input()
  String rotulo = '';

  @HostBinding('class.d-block')
  bool get hostClass => true;

  @override
  void writeValue(Object? value) {}

  @override
  void registerOnChange(ChangeFunction<Object?> fn) {}

  @override
  void registerOnTouched(TouchFunction fn) {}

  @override
  void onDisabledChanged(bool isDisabled) {}
}

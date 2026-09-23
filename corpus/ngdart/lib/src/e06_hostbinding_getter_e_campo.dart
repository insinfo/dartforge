import 'package:ngdart/angular.dart';

/// `@HostBinding` num campo e num getter: o oficial coleta acessores antes
/// de campos, então o getter declarado depois sai primeiro.
@Directive(selector: '[e06-host]')
class E06HostbindingGetterECampo {
  @HostBinding('class.fixa')
  bool fixa = true;

  bool _ativo = false;

  @HostBinding('class.ativo')
  bool get ativo => _ativo;

  set ativo(bool v) => _ativo = v;
}
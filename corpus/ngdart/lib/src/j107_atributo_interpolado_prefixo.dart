import 'package:ngdart/angular.dart';

/// `attr.x="{{..}}"`, `class.x="{{..}}"`, `style.x.px="{{..}}"` e
/// `id="{{..}}"`: o `_createPropertyForAttribute` do oficial manda o nome
/// ao mesmo `createElementPropertyAst` de `[x]`, com o valor
/// `Interpolation` (o `material_toggle`/`material_progress` do
/// ngcomponents).
@Component(
  selector: 'j107-atributo-interpolado-prefixo',
  template: '''
<div attr.aria-label="{{rotulo}}" attr.aria-disabled="{{desligado}}"
     attr.aria-valuemin="{{minimo}}" id="{{identificador}}"
     attr.role="{{papel}}" attr.aria-hidden="{{!desligado}}"></div>
<span style.width.px="{{minimo}}">x</span>''',
)
class J107AtributoInterpoladoPrefixo {
  String? rotulo;
  bool desligado = false;
  int minimo = 0;
  String identificador = 'a';
  final String papel = 'switch';
}

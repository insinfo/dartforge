import 'package:ngdart/angular.dart';

/// `@HostBinding` em estático imutável fora de `class.x`/`style.x` é
/// `hostAttribute` (`_computeHostBindingImmutability`): fica fora da
/// `XNgCd`, e uma diretiva só com eles não tem `XNgCd` (o
/// `ReorderItemDirective`/`ReorderHandleDirective` do ngcomponents).
@Directive(selector: '[j118Item]')
class J118Item {
  @HostBinding('attr.role')
  static const papel = 'listitem';

  @HostBinding('tabIndex')
  static const indice = 0;

  @HostBinding('class.ativo')
  bool ativo = false;
}

@Directive(selector: '[j118Alca]')
class J118Alca {
  @HostBinding('attr.draggable')
  static const arrastavel = 'true';
}

import 'package:ngdart/angular.dart';

/// `@HostBinding` de um getter numa classe abstrata usada como mixin
/// (`with J102TemTab`), herdado por dois níveis — o `HasTabIndex` do
/// ngcomponents no `MaterialButtonComponent`.
abstract mixin class J102TemTab {
  String? _tab;

  @Input()
  set tabindex(String? v) => _tab = v;

  @HostBinding('attr.tabindex')
  String? get tabIndex => _tab ?? '0';
}

class J102Raiz {}

class J102Botao extends J102Raiz with J102TemTab {
  @HostBinding('attr.role')
  String get papel => 'button';
}

class J102Base extends J102Botao {}

@Component(
  selector: 'j102-botao',
  template: '<b>{{ tabIndex }}</b>',
)
class J102MixinComHostBinding extends J102Base {
  @HostBinding('class.grande')
  bool grande = false;
}

@Component(
  selector: 'j102-usa',
  template: '<j102-botao tabindex="3"></j102-botao>',
  directives: [J102MixinComHostBinding],
)
class J102Usa {}

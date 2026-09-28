import 'package:ngdart/angular.dart';

/// `@HostBinding` estático num componente que herda: o da própria classe é
/// lido dela (`StaticRead`) e escrito no construtor da visão; o estático de
/// um supertipo não é herdado (`_addHostBinding`); os dinâmicos da base
/// entram na ordem do `DirectiveVisitor` (supertipos primeiro).
class J98Base {
  @HostBinding('attr.role')
  static const hostRole = 'group';

  @HostBinding('class.ativo')
  bool ativo = false;

  @HostBinding('attr.aria-label')
  String? rotulo;
}

@Component(
  selector: 'j98-estatico-herdado',
  template: '<span>{{ rotulo }}</span>',
)
class J98EstaticoHerdado extends J98Base {
  @HostBinding('class')
  static const hostClass = 'themeable';

  @HostBinding('attr.aria-modal')
  static const modal = 'true';

  @HostBinding('attr.tabindex')
  static const tab = 0;

  @HostBinding('class.grande')
  bool grande = true;
}

@Component(
  selector: 'j98-usa',
  template: '<j98-estatico-herdado></j98-estatico-herdado>',
  directives: [J98EstaticoHerdado],
)
class J98Usa {}

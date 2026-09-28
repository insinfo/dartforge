import 'package:ngdart/angular.dart';

@Component(
  selector: 'j92-filho',
  template: '<i></i>',
)
class J92Filho {
  @Input()
  String? rotulo;
}

@Directive(selector: '[j92-destaque]')
class J92Destaque {
  @HostBinding('class.destaque')
  bool ativo = true;

  @HostBinding('attr.aria-label')
  String? rotulo = 'x';
}

/// Atributo interpolado no elemento de um filho, fora de `@Input`, e
/// `@HostBinding` de diretiva no elemento de um filho.
@Component(
  selector: 'j92-filho-com-ligacoes-do-elemento',
  template: '<j92-filho title="a {{ nome }}" [rotulo]="nome"></j92-filho>'
      '<j92-filho j92-destaque></j92-filho>',
  directives: [J92Filho, J92Destaque],
)
class J92FilhoComLigacoesDoElemento {
  String nome = 'n';
}

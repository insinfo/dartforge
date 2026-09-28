import 'package:ngdart/angular.dart';

/// `@Attribute('x')` no construtor (o `target` do `RouterLink` do
/// ngrouter): o valor escrito no elemento, ou `null`.
@Directive(selector: '[j42-alvo]')
class J42Alvo {
  final String? alvo;
  final String? rel;
  final String? marca;

  J42Alvo(@Attribute('target') this.alvo, @Attribute('rel') this.rel,
      @Attribute('data-marca') this.marca);
}

@Component(
  selector: 'j42-atributo-injetado',
  templateUrl: 'j42_atributo_injetado.html',
  directives: [J42Alvo, NgIf],
)
class J42AtributoInjetado {
  bool mostrar = true;
  String destino = '_self';
}

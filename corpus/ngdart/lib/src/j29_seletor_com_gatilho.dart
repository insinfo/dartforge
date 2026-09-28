import 'package:ngdart/angular.dart';

/// Diretiva e componente no mesmo arquivo (o `li_select.dart` do
/// limitless_ui): a diretiva sem `@HostBinding` não gera classe.
@Directive(selector: 'template[j29Gatilho]')
class J29Gatilho {
  J29Gatilho(this.templateRef);

  final TemplateRef templateRef;
}

@Pipe('j29Maiusculas')
class J29Maiusculas {
  String transform(String v) => v.toUpperCase();
}

@Component(
  selector: 'j29-seletor',
  templateUrl: 'j29_seletor_com_gatilho.html',
  directives: [NgTemplateOutlet],
)
class J29Seletor {
  @ContentChild(J29Gatilho)
  J29Gatilho? gatilho;
}

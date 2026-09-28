import 'package:ngdart/angular.dart';

@Directive(selector: '[j69-form]', exportAs: 'j69Form')
class J69Form {
  @HostBinding('class.ok')
  bool ok = true;
}

@Directive(selector: '[j69-dica]', exportAs: 'j69Dica')
class J69Dica {}

@Component(
  selector: 'j69-cartao',
  template: '<div><ng-content></ng-content></div>',
)
class J69Cartao {}

/// `@ViewChild` de `#ref="exportAs"` no conteúdo projetado de um filho (o
/// `#personForm="liForm"` dentro do cartão no limitless_ui): estático, lido
/// no `build()` como na própria visão.
@Component(
  selector: 'j69-export-as-projetado',
  templateUrl: 'j69_export_as_projetado.html',
  directives: [J69Form, J69Dica, J69Cartao],
)
class J69ExportAsProjetado {
  @ViewChild('forma')
  J69Form? forma;

  @ViewChild('dica')
  J69Dica? dica;

  void enviar(J69Form f) {}
}

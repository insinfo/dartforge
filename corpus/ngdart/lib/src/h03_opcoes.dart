import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// `<option>` dentro de `*ngFor`: o `@Host` do `NgSelectOption` acha o
/// acessor do `<select>` na visão de cima. E um `MaxLengthValidator`
/// (`@HostBinding`: o `XNgCd` e o `detectHostChanges`), `style` e
/// `tabindex` escritos.
@Component(
  selector: 'h03-opcoes',
  templateUrl: 'h03_opcoes.html',
  directives: [coreDirectives, formDirectives],
)
class H03Opcoes {
  String escolha = 'a';
  List<String> opcoes = ['a', 'b'];
}
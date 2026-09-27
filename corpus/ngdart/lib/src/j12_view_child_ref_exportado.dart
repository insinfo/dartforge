import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

/// `@ViewChild` de `#ref` com valor: a instância da diretiva exportada
/// (`#f="ngForm"`), não o elemento.
@Component(
  selector: 'j12-view-child-ref-exportado',
  templateUrl: 'j12_view_child_ref_exportado.html',
  directives: [formDirectives],
)
class J12ViewChildRefExportado {
  @ViewChild('f')
  NgForm? form;

  String nome = '';
}

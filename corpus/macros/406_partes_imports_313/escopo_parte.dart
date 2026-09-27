part of 'main.dart';

import 'escopo_b.dart';
import 'escopo_b.dart' as p;

part 'escopo_neta.dart';
part 'escopo_outra_neta.dart';

String daParte() => 'parte: ${nome()} ${p.nome()} ${Caixa().quem} ${local()}';

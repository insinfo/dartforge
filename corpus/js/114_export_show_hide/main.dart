// export com show/hide: main importa api.dart (X, y, tudo de outro menos Z) e extras.dart com show/hide direto e com prefixo.
import 'api.dart';
import 'extras.dart' show visivel, Ferramenta;
import 'extras.dart' as ex hide invisivel;
import 'api.dart' as api show X, saudacao;

void main() {
  print(versao());
  final x = X(5);
  print(x);
  print(y(4));
  // z, Interno e escondida (impl.dart) não são visíveis aqui: não foram exportados.
  // O método X.interno() existe e devolve um Interno mesmo sem o nome estar em escopo.
  final i = x.interno();
  print(i);
  print(i.v);
  print(usaZ(2));
  print(usaInterno());
  print(usaEscondida());
  print('--');
  print(W('w'));
  print(saudacao('mundo'));
  print(limite);
  print(Estado.ligado);
  print(Estado.values.map((e) => e.name).join(','));
  print(mutavel);
  mutavel = 'alterada via reexport';
  print(mutavel);
  // Z (outro.dart) está oculta pelo `hide Z` de api.dart.
  print('--');
  print(visivel());
  // invisivel() e outraVisivel() não entram pelo import com show.
  print(Ferramenta().usa());
  print(ex.visivel());
  print(ex.outraVisivel());
  // ex.invisivel() não está disponível: hide invisivel.
  print(ex.Ferramenta().usa());
  print(identical(Ferramenta, ex.Ferramenta));
  print('--');
  print(api.X(9));
  print(api.saudacao('prefixo'));
  // api.y e api.W não existem: o import com prefixo mostra só X e saudacao.
  print(api.X(1).runtimeType == X(2).runtimeType);
  print(x is api.X);
  print(<X>[api.X(1), X(2)].map((e) => e.v).toList());
  print('fim');
}

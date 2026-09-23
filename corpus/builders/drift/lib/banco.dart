// drift_dev: tabelas em Dart e em SQL (.drift), um DAO e uma consulta
// declarada no .drift. Exercita o builder de três fábricas (discover,
// analyzer, driftBuilder), o preparing_builder com as expressões Dart do
// .drift e o cleanup que apaga os `.temp.dart`.
import 'package:drift/drift.dart';

part 'banco.g.dart';

enum Situacao { aberta, fechada }

class Clientes extends Table {
  IntColumn get id => integer().autoIncrement()();
  TextColumn get nome => text().withLength(min: 1, max: 80)();
  TextColumn get email => text().nullable().unique()();
  DateTimeColumn get criadoEm => dateTime().withDefault(currentDateAndTime)();
}

@DataClassName('Conta')
class Contas extends Table {
  IntColumn get id => integer().autoIncrement()();
  IntColumn get cliente => integer().references(Clientes, #id)();
  RealColumn get saldo => real().withDefault(const Constant(0))();
  IntColumn get situacao => intEnum<Situacao>()();
}

@DriftAccessor(tables: [Clientes, Contas])
class ContasDao extends DatabaseAccessor<Banco> with _$ContasDaoMixin {
  ContasDao(super.db);

  Future<List<Conta>> doCliente(int cliente) =>
      (select(contas)..where((c) => c.cliente.equals(cliente))).get();
}

@DriftDatabase(tables: [Clientes, Contas], daos: [ContasDao], include: {'consultas.drift'})
class Banco extends _$Banco {
  Banco(super.e);

  @override
  int get schemaVersion => 2;
}

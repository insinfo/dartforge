// Extensão experimental DartForge: macro Rust incorporada, sem import fictício.
@JsonCodable()
class User {
  final String username;
  final int age;
  final bool enabled;
  final String? email;
  final int? score;
  final bool? flag;
}
@JsonCodable()
class Existing {
  final String key;
  Existing(this.key);
}
@JsonCodable()
class Empty {}
void main() {
  var user = User.fromJson({'username':'á🦀', 'age':28, 'enabled':true, 'ignored':'extra'});
  print(user.username);
  print(user.age);
  print(user.enabled);
  print(user.email);
  print(user.score);
  print(user.flag);
  var json = user.toJson();
  print(json.length);
  print(json['username'] as String);
  print(json['email'] == null);
  var direct = User('direct', 9, false, 'mail', 7, true);
  var copy = User.fromJson(direct.toJson());
  print(copy.username);
  print(copy.email);
  print(copy.score);
  print(copy.flag);
  print(Existing.fromJson(<String,Object?>{'key':'existing'}).key);
  print(Empty.fromJson({}).toJson().length);
}

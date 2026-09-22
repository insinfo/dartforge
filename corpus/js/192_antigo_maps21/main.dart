// Convertido de tests/conformance/modules/maps21 (módulo antigo do corpus de conformidade).
// Oráculo manual equivalente; executável no SDK Dart 3.6.2 sem suporte a macros.
class User {
  final String username;
  final int age;
  final bool enabled;
  final String? email;
  final int? score;
  final bool? flag;
  User(this.username,this.age,this.enabled,this.email,this.score,this.flag);
  factory User.fromJson(Map<String,Object?> json) => User(json['username'] as String,json['age'] as int,json['enabled'] as bool,json['email'] as String?,json['score'] as int?,json['flag'] as bool?);
  Map<String,Object?> toJson() { return <String,Object?>{'username':this.username,'age':this.age,'enabled':this.enabled,'email':this.email,'score':this.score,'flag':this.flag}; }
}
class Existing {
  final String key;
  Existing(this.key);
  factory Existing.fromJson(Map<String,Object?> json) => Existing(json['key'] as String);
  Map<String,Object?> toJson() { return <String,Object?>{'key':this.key}; }
}
class Empty {
  Empty();
  factory Empty.fromJson(Map<String,Object?> json) => Empty();
  Map<String,Object?> toJson() { return <String,Object?>{}; }
}
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

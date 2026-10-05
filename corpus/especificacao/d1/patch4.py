# -*- coding: utf-8 -*-
p='bloco-04.md'
s=open(p,encoding='utf-8').read()
def rep(a,b,cnt=1):
    global s
    assert s.count(a)>=1, a[:60]
    s=s.replace(a,b,cnt)
rep("`const_eval_throws_exception off=27 len=11 2:11` (o `as` inteiro); `off=67 len=8 4:12`","`const_eval_throws_exception off=27 len=11 2:11` (o `as` inteiro); `off=73 len=8 4:12`")
rep("(linhas 2–3 de `c24`) `const_eval_type_bool off=40 len=1 2:11` e `off=61 len=4 3:11` (+ `non_bool_condition` nos mesmos intervalos)","- `const_eval_type_bool off=10 len=1 1:11` e `off=31 len=4 2:11` (a condição; + `non_bool_condition` nos mesmos intervalos)")
rep("(linhas 3–5 de `c06`) `off=48 len=2 3:11`; `off=62 len=9 4:11`; `off=83 len=9 5:11` (+ `non_bool_negation_expression`, `non_bool_operand`)","- `const_eval_type_bool off=10 len=2 1:11`; `off=24 len=9 2:11`; `off=45 len=9 3:11` (nó inteiro; + `non_bool_negation_expression off=11 len=1`, `non_bool_operand off=24 len=1` e `off=53 len=1`)")
rep("(linhas 9–12 de `c35`) `const_constructor_param_type_mismatch off=188 len=9 10:11 | … 'String' … 'int' …` e `off=256 len=9 12:11 | … 'Null' … 'String' …` — na criação","- `const_constructor_param_type_mismatch off=71 len=9 2:11 | A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.` e `off=139 len=9 4:11 | A value of type 'Null' … 'String' …` — na criação (+ `missing_default_value_for_parameter off=122 len=1`)")
a=s.index("- **Posição:** fase D: `_errorNode` (a criação / a constante de enum). Fase A:")
b=s.index("- **Mensagem:** `In a const constructor, a value of type '{0}'")
s=s[:a]+"""- **Posição:** fase D: `_errorNode` (a criação / a constante de enum). Fase A: o inicializador do campo **na
  declaração** (`field.constantInitializer`), sem `copyWithEntity` — o erro não é recolocado no uso e, sendo o
  mesmo intervalo para toda criação, sai uma vez só (último exemplo abaixo).
"""+s[b:]
rep("""  - `field_initializer_not_assignable off=227 len=1 14:27`; `const_constructor_field_type_mismatch off=242 len=12 16:11` (na criação: não atribuível estaticamente)
""","""  - `field_initializer_not_assignable off=227 len=1 14:27`; `const_constructor_field_type_mismatch off=242 len=12 16:11` (na criação: não atribuível estaticamente)
  ```dart
  class A {
    final int x = y;
    const A();
  }
  const dynamic y = 'a';
  const a = const A();
  var b = const A();
  ```
  - `const_constructor_field_type_mismatch off=26 len=1 2:17 | In a const constructor, a value of type 'String' can't be assigned to the field 'x', which has type 'int'.` (único, no `y` da declaração)
""")
open(p,'w',encoding='utf-8',newline='\n').write(s)
print('ok')

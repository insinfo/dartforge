# -*- coding: utf-8 -*-
p='bloco-05.md'
s=open(p,encoding='utf-8').read()
a=s.index("```dart\nconst int a = 'x' as dynamic;\nconst List<int> c")
b=s.index("#### 8. No DartForge")
s=s[:a]+"""```dart
const int a = 'x' as dynamic;
const String b = 1 as dynamic;
const List<int> c = <num>[1] as dynamic;
```
- `variable_type_mismatch off=14 len=14 1:15 | A value of type 'String' can't be assigned to a const variable of type 'int'.`; `off=47 len=12 2:18 | … 'int' … 'String'.`; `off=81 len=19 3:21 | … 'List<num>' … 'List<int>'.` (no inicializador inteiro; específico, substitui o padrão)

"""+s[b:]
s=s.replace("  (linhas 1–5 de `c15`) `off=42 len=1 3:16`","  - `off=42 len=1 3:16`")
open(p,'w',encoding='utf-8',newline='\n').write(s)
print('ok')

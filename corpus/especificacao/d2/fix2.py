import io
p=r'E:\dftemp\analise\spec-r4\casos\d2\bloco-04.md'
s=io.open(p,encoding='utf-8').read()
i=s.index("- **Exemplos (oráculo vivo 3.6.2):** `e01`, `e03`, `e04`, `e05` em §4.1; amostras reduzidas:")
j=s.index("##### `const_with_type_parameters`")
novo="""- **Exemplos (oráculo vivo 3.6.2):** `e01`, `e03`, `e04`, `e05` em §4.1; amostras `c_72136293` e `c_cb0c58ee` reduzidas (`j01.dart`):

```dart
void f(A<int> x, B y) {
  if (x case const A<num>()) {}
  if (y case const A<int>()) {}
}
class A<T> { const A(); }
class B extends A<int> { const B(); }
```
```
constant_pattern_never_matches_value_type off=37 len=14 2:14 | The matched value type 'A<int>' can never be equal to this constant of type 'A<num>'.
constant_pattern_never_matches_value_type off=69 len=14 3:14 | The matched value type 'B' can never be equal to this constant of type 'A<int>'.
```

"""
s=s[:i]+novo+s[j:]
io.open(p,'w',encoding='utf-8',newline='\n').write(s)

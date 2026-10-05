import io
p=r'E:\dftemp\analise\spec-r4\partes\D2-verificador.md'
s=io.open(p,encoding='utf-8').read()
a="| `InterfaceType` `int` | `InterfaceType` `double` (com ou sem `?`: `isDartCoreDouble` não olha o sufixo — não verificado para `double?`) | `true` |"
b="| `InterfaceType` `int` | `InterfaceType` `double` ou `double?` (`isDartCoreDouble` não olha o sufixo; conferido em `i01.dart`, linha 2) | `true` |"
assert a in s; s=s.replace(a,b)
a="`NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION`. Não há teste de `InvalidType` aqui (operando não resolvido → o `InvalidConstant` do\nidentificador não resolvido vem com `avoidReporting` — D.R4-1). O operando"
b="`NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION`. Não há teste de `InvalidType` aqui: operando não resolvido sai **junto** com\n`undefined_identifier`, no identificador (`i01.dart`, linha 5); no padrão constante o teste existe e só sai o erro da resolução\n(linha 6). O operando"
assert a in s; s=s.replace(a,b)
io.open(p,'w',encoding='utf-8',newline='\n').write(s)

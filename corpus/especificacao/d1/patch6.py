# -*- coding: utf-8 -*-
p='/e/dftemp/analise/spec-r4/partes/D1-motor.md'
import io,sys
p=r'E:\dftemp\analise\spec-r4\partes\D1-motor.md'
s=open(p,encoding='utf-8').read()
a="`FunctionExpressionInvocation`, `NullAssertion`=`PostfixExpression`, `IsExpression` é próprio…)"
assert a in s
s=s.replace(a,"`FunctionExpressionInvocation`; `x!` é `PostfixExpression`)")
a="| campo | significado | quem liga |\n|---|---|---|"
assert a in s
s=s.replace(a,"Na coluna \"quem liga\" as linhas são de `evaluation.dart`.\n\n"+a)
open(p,'w',encoding='utf-8',newline='\n').write(s)
n=sum(1 for l in s.split('\n') if l.strip().startswith('```'))
print('fences',n, 'lines', s.count('\n'))

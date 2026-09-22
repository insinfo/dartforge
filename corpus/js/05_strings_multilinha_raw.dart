// Strings multilinha ''' e """, quebra inicial, raw r'...' e concatenação adjacente entre linhas.
void main() {
  var m1 = '''primeira
segunda
terceira''';
  print(m1);
  print(m1.split('\n').length);

  var m2 = '''
com quebra inicial ignorada
fim''';
  print(m2);
  print(m2.startsWith('com'));

  var m3 = """
  indentado
    mais
fim""";
  print(m3);
  print(m3.split('\n')[0].length);

  var m4 = '''a
''';
  print(m4.length);
  print(m4.endsWith('\n'));

  var m5 = '''
''';
  print(m5.length);

  print('''interp ${1 + 1} em
multilinha $m4'''.trim());

  print('''com \\ escape e \n dentro''');
  print('''aspas ' " '' "" dentro''');
  print("""aspas " ' "" '' dentro""");

  var raw = r'sem \n escape e $nao interpola';
  print(raw);
  print(raw.length);
  print(r'\\');
  print(r'\\'.length);
  print(r'$');
  print(r'${x}');
  print(r"raw dupla \t $y");
  print(r'''raw
multi \n $z''');
  print(r"""raw
dupla """ r'e adjacente');
  print(r'C:\Users\nome\teste');

  var adj = 'uma '
      'duas '
      'tres';
  print(adj);
  var adj2 = 'a' "b" '''c''' """d""" r'e';
  print(adj2);
  var adj3 = 'x'
      '$adj'
      'y';
  print(adj3);
  print('''
'''.isEmpty);
  print('''

'''.length);
  print(r'\'.length);
  print('\\'.length);
  print('''
  espaco inicial preservado
'''.length);
  print('''
depois de espacos na primeira linha''');
  print('''\
escape de quebra''');
  print(r'''\
raw nao escapa quebra'''.length);
}

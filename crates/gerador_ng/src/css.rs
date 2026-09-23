//! O shim de estilo do ngdart: o que o `shadow_css.dart` do ngcompiler faz.
//!
//! Sem Shadow DOM, o isolamento de estilo é por atributo: cada elemento da
//! visão recebe `_ngcontent-<id>` e cada seletor da folha ganha
//! `._ngcontent-%ID%` — o `%ID%` é trocado pelo id do componente em tempo de
//! execução. O seletor `:host` vira `._nghost-%ID%`.
//!
//! As regras foram lidas da saída do compilador oficial para um CSS que cobre
//! as formas reais (caso b15 do corpus):
//!
//! ```text
//! :host { display: block; }   ._nghost-%ID%{display:block}
//! .a .b { color: red; }       .a._ngcontent-%ID% .b._ngcontent-%ID%{color:red}
//! .c, .d { margin: 0 auto; }  .c._ngcontent-%ID%,.d._ngcontent-%ID%{margin:0 auto}
//! a:hover { … }               a:hover._ngcontent-%ID%{…}
//! @media (max-width: 600px)   @media (max-width:600px){…}
//! ```
//!
//! O que não estiver aqui é **recusado**, não traduzido por aproximação: uma
//! folha traduzida errado quebra a aparência sem quebrar a compilação, que é
//! o pior tipo de defeito.
use crate::visao::Motivo;

const CONTEUDO: &str = "._ngcontent-%ID%";
const HOSPEDEIRO: &str = "._nghost-%ID%";

/// Transforma a folha no texto que vai dentro de `styles` no
/// `<nome>.css.shim.dart`.
pub fn shim(css: &str) -> Result<String, Motivo> {
    let limpo = sem_comentarios(css);
    let mut saida = String::with_capacity(limpo.len());
    regras(&limpo, &mut saida, true)?;
    Ok(saida)
}

/// Remove `/* … */`.
fn sem_comentarios(css: &str) -> String {
    let mut saida = String::with_capacity(css.len());
    let b = css.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            match css[i + 2..].find("*/") {
                Some(f) => i += 2 + f + 2,
                None => break,
            }
            continue;
        }
        saida.push(css[i..].chars().next().unwrap_or('\0'));
        i += css[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    saida
}

/// Percorre regras e regras-arroba no nível dado.
fn regras(css: &str, saida: &mut String, topo: bool) -> Result<(), Motivo> {
    let mut resto = css.trim();
    while !resto.is_empty() {
        if let Some(sem_arroba) = resto.strip_prefix('@') {
            let Some(fim_prelúdio) = sem_arroba.find(['{', ';']) else {
                return Err(Motivo::Estilos);
            };
            let nome = sem_arroba.split_whitespace().next().unwrap_or("");
            let nome = nome
                .trim_start_matches('-')
                .split('-')
                .next_back()
                .unwrap_or(nome);
            // `@keyframes` sai como está: `from`, `to` e as porcentagens não
            // são seletores e não podem ganhar o atributo do escopo.
            if nome == "keyframes" {
                let corpo_ini = fim_prelúdio + 1;
                let fim = fim_do_bloco(sem_arroba, corpo_ini).ok_or(Motivo::Estilos)?;
                saida.push('@');
                saida.push_str(&comprimir(&sem_arroba[..fim_prelúdio]));
                saida.push('{');
                saida.push_str(&sem_escopo(&sem_arroba[corpo_ini..fim])?);
                saida.push('}');
                resto = sem_arroba[fim + 1..].trim_start();
                continue;
            }
            // `@media` aninha regras; o resto (`@font-face`, `@import`) tem
            // semântica própria e fica de fora.
            if nome != "media" || !topo {
                return Err(Motivo::Estilos);
            }
            let prelúdio = &sem_arroba[..fim_prelúdio];
            let corpo_ini = fim_prelúdio + 1;
            let fim = fim_do_bloco(sem_arroba, corpo_ini).ok_or(Motivo::Estilos)?;
            saida.push('@');
            saida.push_str(&comprimir(prelúdio));
            saida.push('{');
            regras(&sem_arroba[corpo_ini..fim], saida, false)?;
            saida.push('}');
            resto = sem_arroba[fim + 1..].trim_start();
            continue;
        }
        let Some(abre) = resto.find('{') else {
            return Err(Motivo::Estilos);
        };
        let seletor = &resto[..abre];
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let corpo = &resto[abre + 1..fim];
        if corpo.contains('{') {
            return Err(Motivo::Estilos); // aninhamento: é Sass, não CSS
        }
        saida.push_str(&seletores(seletor)?);
        saida.push('{');
        saida.push_str(&declaracoes(corpo)?);
        saida.push('}');
        resto = resto[fim + 1..].trim_start();
    }
    Ok(())
}

/// Índice da chave que fecha o bloco aberto em `ini`.
fn fim_do_bloco(texto: &str, ini: usize) -> Option<usize> {
    let mut nivel = 1usize;
    for (i, c) in texto[ini..].char_indices() {
        match c {
            '{' => nivel += 1,
            '}' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some(ini + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Lista de seletores separada por vírgula, cada um com o atributo do escopo.
fn seletores(lista: &str) -> Result<String, Motivo> {
    let mut partes = Vec::new();
    for s in lista.split(',') {
        let s = s.trim();
        if s.is_empty() {
            return Err(Motivo::Estilos);
        }
        partes.push(um_seletor(s)?);
    }
    Ok(partes.join(","))
}

/// Um seletor, com as formas que o compilador oficial trata — cada uma
/// conferida contra a saída dele (caso b16 do corpus):
///
/// | escrito | sai |
/// |---|---|
/// | `.a::before` | `.a._ngcontent-%ID%::before` |
/// | `:host(.x) .b` | `._nghost-%ID%.x .b._ngcontent-%ID%` |
/// | `:host-context(.p) .c` | dois seletores, um por posição do hospedeiro |
/// | `::ng-deep .d` | ` .d` — o que vem depois não é escopado |
/// | `.e > .f` | `.e._ngcontent-%ID% > .f._ngcontent-%ID%` |
fn um_seletor(s: &str) -> Result<String, Motivo> {
    // `::ng-deep`, `>>>` e `/deep/` soltam o escopo daí para a frente.
    for fundo in ["::ng-deep", ">>>", "/deep/"] {
        if let Some((antes, depois)) = s.split_once(fundo) {
            let antes = antes.trim();
            let escopado = if antes.is_empty() {
                String::new()
            } else {
                um_seletor(antes)?
            };
            return Ok(format!("{escopado} {}", depois.trim()));
        }
    }
    if let Some(resto) = s.strip_prefix(":host-context(") {
        // O hospedeiro pode ser o próprio elemento ou um ancestral dele.
        let (dentro, depois) = ate_fechar(resto).ok_or(Motivo::Estilos)?;
        let cauda = compostos(depois.trim())?;
        let junta = |a: String| {
            if cauda.is_empty() {
                a
            } else {
                format!("{a} {cauda}")
            }
        };
        return Ok(format!(
            "{},{}",
            junta(format!("{HOSPEDEIRO}{dentro}")),
            junta(format!("{dentro} {HOSPEDEIRO}"))
        ));
    }
    if let Some(resto) = s.strip_prefix(":host(") {
        let (dentro, depois) = ate_fechar(resto).ok_or(Motivo::Estilos)?;
        let cauda = compostos(depois.trim())?;
        let cabeca = format!("{HOSPEDEIRO}{dentro}");
        return Ok(if cauda.is_empty() {
            cabeca
        } else {
            format!("{cabeca} {cauda}")
        });
    }
    compostos(s)
}

/// O conteúdo até o parêntese que fecha, e o que vem depois.
fn ate_fechar(s: &str) -> Option<(&str, &str)> {
    let mut nivel = 1usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => nivel += 1,
            ')' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some((&s[..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Cada composto da cadeia recebe o atributo — antes do pseudo-elemento,
/// depois da pseudo-classe, como o oficial faz.
fn compostos(s: &str) -> Result<String, Motivo> {
    if s.is_empty() {
        return Ok(String::new());
    }
    let mut saida = String::with_capacity(s.len() + CONTEUDO.len());
    for (i, composto) in s.split_whitespace().enumerate() {
        if i > 0 {
            saida.push(' ');
        }
        // Combinadores soltos (`>`, `+`, `~`) não são compostos.
        if matches!(composto, ">" | "+" | "~") {
            saida.push_str(composto);
            continue;
        }
        if composto == ":host" {
            saida.push_str(HOSPEDEIRO);
            continue;
        }
        if composto.starts_with(":host") {
            return Err(Motivo::Estilos);
        }
        match composto.split_once("::") {
            Some((antes, pseudo)) => {
                saida.push_str(antes);
                saida.push_str(CONTEUDO);
                saida.push_str("::");
                saida.push_str(pseudo);
            }
            None => {
                saida.push_str(composto);
                saida.push_str(CONTEUDO);
            }
        }
    }
    Ok(saida)
}

/// Regras sem escopo nenhum, para dentro de `@keyframes`.
fn sem_escopo(css: &str) -> Result<String, Motivo> {
    let mut saida = String::new();
    let mut resto = css.trim();
    while !resto.is_empty() {
        let Some(abre) = resto.find('{') else {
            return Err(Motivo::Estilos);
        };
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        saida.push_str(&comprimir(&resto[..abre]));
        saida.push('{');
        saida.push_str(&declaracoes(&resto[abre + 1..fim])?);
        saida.push('}');
        resto = resto[fim + 1..].trim_start();
    }
    Ok(saida)
}

/// Separa um `!important` do fim do valor (com espaços entre `!` e a
/// palavra, em qualquer caixa, como o parser do `csslib` aceita).
fn sem_important(valor: &str) -> (&str, bool) {
    let v = valor.trim_end();
    if let Some(i) = v.rfind('!')
        && v[i + 1..].trim().eq_ignore_ascii_case("important")
        && !v[..i].contains(['"', '\''])
    {
        return (v[..i].trim_end(), true);
    }
    (valor, false)
}

/// Declarações `prop:valor`, sem ponto e vírgula final.
fn declaracoes(corpo: &str) -> Result<String, Motivo> {
    let mut partes = Vec::new();
    for d in corpo.split(';') {
        let d = d.trim();
        if d.is_empty() {
            continue;
        }
        let Some((prop, valor)) = d.split_once(':') else {
            return Err(Motivo::Estilos);
        };
        // `!important` é marca da declaração, não do valor: o `csslib`
        // compacto a escreve colada (`emit('$_sp!important')`, `_sp` vazio).
        let (valor, importante) = sem_important(valor);
        partes.push(format!(
            "{}:{}{}",
            prop.trim(),
            cores(&urls(&virgulas(&comprimir_valor(valor))?)?),
            if importante { "!important" } else { "" }
        ));
    }
    Ok(partes.join(";"))
}

/// Espaço interno do valor é preservado (`0 auto`), o das pontas não — e o
/// que encosta em parêntese some, como na saída do compilador oficial
/// (`linear-gradient(45deg, …)`).
fn comprimir_valor(v: &str) -> String {
    let mut saida = String::with_capacity(v.len());
    let mut espaco = false;
    for c in v.trim().chars() {
        if c.is_whitespace() {
            espaco = true;
            continue;
        }
        if espaco && !saida.is_empty() && !saida.ends_with('(') && c != ')' {
            saida.push(' ');
        }
        espaco = false;
        saida.push(c);
    }
    saida
}

/// As vírgulas do valor como o `CssPrinter` do `csslib` (compacto) as
/// escreve depois de reler a folha: numa lista ou nos argumentos de uma
/// função comum, `,` sem espaço (`OperatorComma` e o `_sp` vazio de
/// `visitExpressions`); no valor padrão de `var(--x, y)`, `, ` (é assim que
/// `visitVarUsage` escreve); dentro de `calc`/`min`/`max`/`clamp`, de
/// `url(...)` e de texto entre aspas, o texto como veio (`processCalc`
/// guarda a expressão crua). O Sass comprimido já tira quase todo espaço;
/// o que sobra é função especial, como `rgba(var(--x), 0.14)`.
fn virgulas(v: &str) -> Result<String, Motivo> {
    let mut saida = String::with_capacity(v.len());
    // Pilha das funções abertas e, para cada `var(`, se a vírgula dele já
    // apareceu.
    let mut pilha: Vec<(String, bool)> = Vec::new();
    let mut aspas: Option<char> = None;
    let mut chars = v.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            continue;
        }
        let crua = pilha.iter().any(|(f, _)| CRUAS.contains(&f.as_str()));
        match c {
            '"' | '\'' => {
                aspas = Some(c);
                saida.push(c);
            }
            '(' => {
                let nome: String = saida
                    .chars()
                    .rev()
                    .take_while(|x| x.is_ascii_alphanumeric() || *x == '-' || *x == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                pilha.push((nome.to_ascii_lowercase(), false));
                saida.push(c);
            }
            ')' => {
                pilha.pop();
                saida.push(c);
            }
            ',' if crua => saida.push(c),
            ',' => {
                // Espaço antes da vírgula some em qualquer caso.
                while saida.ends_with(' ') {
                    saida.pop();
                }
                saida.push(',');
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                if let Some((f, vista)) = pilha.last_mut()
                    && f == "var"
                {
                    // Uma vírgula só: o valor padrão. Com mais de uma, a
                    // lista de padrões do `VarUsage` tem regra própria.
                    if *vista {
                        return Err(Motivo::Estilos);
                    }
                    *vista = true;
                    saida.push(' ');
                }
            }
            c => saida.push(c),
        }
    }
    Ok(saida)
}

/// `_EXTENDED_COLOR_NAMES` do `csslib` (`token_kind.dart`), na mesma ordem e
/// com os mesmos valores — `aliceblue` inclusive, que lá está `0xF08FF`. A
/// busca pelo valor fica com o primeiro nome (`aqua` antes de `cyan`).
const CORES: &[(&str, u32)] = &[
    ("aliceblue", 0xF08FF),
    ("antiquewhite", 0xFAEBD7),
    ("aqua", 0x00FFFF),
    ("aquamarine", 0x7FFFD4),
    ("azure", 0xF0FFFF),
    ("beige", 0xF5F5DC),
    ("bisque", 0xFFE4C4),
    ("black", 0x000000),
    ("blanchedalmond", 0xFFEBCD),
    ("blue", 0x0000FF),
    ("blueviolet", 0x8A2BE2),
    ("brown", 0xA52A2A),
    ("burlywood", 0xDEB887),
    ("cadetblue", 0x5F9EA0),
    ("chartreuse", 0x7FFF00),
    ("chocolate", 0xD2691E),
    ("coral", 0xFF7F50),
    ("cornflowerblue", 0x6495ED),
    ("cornsilk", 0xFFF8DC),
    ("crimson", 0xDC143C),
    ("cyan", 0x00FFFF),
    ("darkblue", 0x00008B),
    ("darkcyan", 0x008B8B),
    ("darkgoldenrod", 0xB8860B),
    ("darkgray", 0xA9A9A9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xA9A9A9),
    ("darkkhaki", 0xBDB76B),
    ("darkmagenta", 0x8B008B),
    ("darkolivegreen", 0x556B2F),
    ("darkorange", 0xFF8C00),
    ("darkorchid", 0x9932CC),
    ("darkred", 0x8B0000),
    ("darksalmon", 0xE9967A),
    ("darkseagreen", 0x8FBC8F),
    ("darkslateblue", 0x483D8B),
    ("darkslategray", 0x2F4F4F),
    ("darkslategrey", 0x2F4F4F),
    ("darkturquoise", 0x00CED1),
    ("darkviolet", 0x9400D3),
    ("deeppink", 0xFF1493),
    ("deepskyblue", 0x00BFFF),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1E90FF),
    ("firebrick", 0xB22222),
    ("floralwhite", 0xFFFAF0),
    ("forestgreen", 0x228B22),
    ("fuchsia", 0xFF00FF),
    ("gainsboro", 0xDCDCDC),
    ("ghostwhite", 0xF8F8FF),
    ("gold", 0xFFD700),
    ("goldenrod", 0xDAA520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xADFF2F),
    ("grey", 0x808080),
    ("honeydew", 0xF0FFF0),
    ("hotpink", 0xFF69B4),
    ("indianred", 0xCD5C5C),
    ("indigo", 0x4B0082),
    ("ivory", 0xFFFFF0),
    ("khaki", 0xF0E68C),
    ("lavender", 0xE6E6FA),
    ("lavenderblush", 0xFFF0F5),
    ("lawngreen", 0x7CFC00),
    ("lemonchiffon", 0xFFFACD),
    ("lightblue", 0xADD8E6),
    ("lightcoral", 0xF08080),
    ("lightcyan", 0xE0FFFF),
    ("lightgoldenrodyellow", 0xFAFAD2),
    ("lightgray", 0xD3D3D3),
    ("lightgreen", 0x90EE90),
    ("lightgrey", 0xD3D3D3),
    ("lightpink", 0xFFB6C1),
    ("lightsalmon", 0xFFA07A),
    ("lightseagreen", 0x20B2AA),
    ("lightskyblue", 0x87CEFA),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xB0C4DE),
    ("lightyellow", 0xFFFFE0),
    ("lime", 0x00FF00),
    ("limegreen", 0x32CD32),
    ("linen", 0xFAF0E6),
    ("magenta", 0xFF00FF),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66CDAA),
    ("mediumblue", 0x0000CD),
    ("mediumorchid", 0xBA55D3),
    ("mediumpurple", 0x9370DB),
    ("mediumseagreen", 0x3CB371),
    ("mediumslateblue", 0x7B68EE),
    ("mediumspringgreen", 0x00FA9A),
    ("mediumturquoise", 0x48D1CC),
    ("mediumvioletred", 0xC71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xF5FFFA),
    ("mistyrose", 0xFFE4E1),
    ("moccasin", 0xFFE4B5),
    ("navajowhite", 0xFFDEAD),
    ("navy", 0x000080),
    ("oldlace", 0xFDF5E6),
    ("olive", 0x808000),
    ("olivedrab", 0x6B8E23),
    ("orange", 0xFFA500),
    ("orangered", 0xFF4500),
    ("orchid", 0xDA70D6),
    ("palegoldenrod", 0xEEE8AA),
    ("palegreen", 0x98FB98),
    ("paleturquoise", 0xAFEEEE),
    ("palevioletred", 0xDB7093),
    ("papayawhip", 0xFFEFD5),
    ("peachpuff", 0xFFDAB9),
    ("peru", 0xCD853F),
    ("pink", 0xFFC0CB),
    ("plum", 0xDDA0DD),
    ("powderblue", 0xB0E0E6),
    ("purple", 0x800080),
    ("red", 0xFF0000),
    ("rosybrown", 0xBC8F8F),
    ("royalblue", 0x4169E1),
    ("saddlebrown", 0x8B4513),
    ("salmon", 0xFA8072),
    ("sandybrown", 0xF4A460),
    ("seagreen", 0x2E8B57),
    ("seashell", 0xFFF5EE),
    ("sienna", 0xA0522D),
    ("silver", 0xC0C0C0),
    ("skyblue", 0x87CEEB),
    ("slateblue", 0x6A5ACD),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xFFFAFA),
    ("springgreen", 0x00FF7F),
    ("steelblue", 0x4682B4),
    ("tan", 0xD2B48C),
    ("teal", 0x008080),
    ("thistle", 0xD8BFD8),
    ("tomato", 0xFF6347),
    ("turquoise", 0x40E0D0),
    ("violet", 0xEE82EE),
    ("wheat", 0xF5DEB3),
    ("white", 0xFFFFFF),
    ("whitesmoke", 0xF5F5F5),
    ("yellow", 0xFFFF00),
    ("yellowgreen", 0x9ACD32),
];

/// Funções cujo conteúdo o `csslib` guarda cru (`processCalc`, `url`).
const CRUAS: &[&str] = &[
    "calc",
    "-webkit-calc",
    "-moz-calc",
    "min",
    "max",
    "clamp",
    "url",
];

/// As cores do valor como o `csslib` as relê e escreve (compacto): `#rrggbb`
/// e nome de cor viram um `HexColorTerm` cujo valor é o inteiro dos dígitos
/// **como escritos** (`#fff` é 0xfff, não 0xffffff); o `CssPrinter` escreve
/// o primeiro nome com esse valor (`hexToColorName`) ou `#` e o texto
/// encurtado (`#aabbcc` → `#abc`, em `_parseHex`). Fora disso, o texto como
/// veio.
fn cores(v: &str) -> String {
    let b: Vec<char> = v.chars().collect();
    let mut saida = String::with_capacity(v.len());
    let mut pilha: Vec<String> = Vec::new();
    let mut aspas: Option<char> = None;
    let mut i = 0;
    let nome_por_valor = |valor: u32| CORES.iter().find(|(_, x)| *x == valor).map(|(n, _)| *n);
    while i < b.len() {
        let c = b[i];
        if let Some(q) = aspas {
            saida.push(c);
            if c == q {
                aspas = None;
            }
            i += 1;
            continue;
        }
        let crua = pilha.iter().any(|f| CRUAS.contains(&f.as_str()));
        if c == '"' || c == '\'' {
            aspas = Some(c);
            saida.push(c);
            i += 1;
            continue;
        }
        if c == ')' {
            pilha.pop();
            saida.push(c);
            i += 1;
            continue;
        }
        let de_nome = |x: char| x.is_ascii_alphanumeric() || x == '-' || x == '_';
        if c == '#' && !crua {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_alphanumeric() {
                j += 1;
            }
            let texto: String = b[i + 1..j].iter().collect();
            let hex = !texto.is_empty()
                && texto.len() <= 8
                && texto.chars().all(|x| x.is_ascii_hexdigit());
            if hex {
                let valor = u32::from_str_radix(&texto, 16).unwrap_or(u32::MAX);
                match nome_por_valor(valor) {
                    Some(n) => saida.push_str(n),
                    None => {
                        saida.push('#');
                        saida.push_str(&encurtar_hex(&texto));
                    }
                }
            } else {
                saida.push('#');
                saida.push_str(&texto);
            }
            i = j;
            continue;
        }
        let anterior = saida.chars().last();
        let inicio_de_nome = c.is_ascii_alphabetic()
            && !anterior.is_some_and(|a| de_nome(a) || matches!(a, '#' | '.' | '@' | '$' | '%'));
        if inicio_de_nome {
            let mut j = i;
            while j < b.len() && de_nome(b[j]) {
                j += 1;
            }
            let nome: String = b[i..j].iter().collect();
            if b.get(j) == Some(&'(') {
                pilha.push(nome.to_ascii_lowercase());
                saida.push_str(&nome);
                saida.push('(');
                i = j + 1;
                continue;
            }
            let achada = if crua {
                None
            } else {
                CORES
                    .iter()
                    .find(|(n, _)| *n == nome.to_lowercase())
                    .and_then(|(_, valor)| nome_por_valor(*valor))
            };
            saida.push_str(achada.unwrap_or(&nome));
            i = j;
            continue;
        }
        if c == '(' {
            pilha.push(String::new());
        }
        saida.push(c);
        i += 1;
    }
    saida
}

/// `url(...)` como o `visitUriTerm` do `csslib` o escreve: sempre
/// `url("texto")`, com ou sem aspas na fonte. Texto com aspas duplas dentro
/// teria de ser escapado: recusa.
fn urls(v: &str) -> Result<String, Motivo> {
    let mut saida = String::with_capacity(v.len());
    let mut resto = v;
    while let Some(i) = resto.find("url(") {
        // `url(` dentro de texto entre aspas não é função.
        let antes = &resto[..i];
        if antes.matches('"').count() % 2 == 1 || antes.matches('\'').count() % 2 == 1 {
            return Err(Motivo::Estilos);
        }
        let depois = &resto[i + 4..];
        let f = depois.find(')').ok_or(Motivo::Estilos)?;
        let dentro = depois[..f].trim();
        let dentro = dentro
            .strip_prefix('"')
            .and_then(|d| d.strip_suffix('"'))
            .or_else(|| dentro.strip_prefix('\'').and_then(|d| d.strip_suffix('\'')))
            .unwrap_or(dentro);
        if dentro.contains(['"', '\'', '\\']) {
            return Err(Motivo::Estilos);
        }
        saida.push_str(antes);
        saida.push_str("url(\"");
        saida.push_str(dentro);
        saida.push_str("\")");
        resto = &depois[f + 1..];
    }
    saida.push_str(resto);
    Ok(saida)
}

/// `_parseHex`: `#RRGGBB` com os pares repetidos vira `#RGB` (e `#RRGG` →
/// `#RG`, `#RR` → `#R`), comparando os caracteres como escritos.
fn encurtar_hex(t: &str) -> String {
    let c: Vec<char> = t.chars().collect();
    match c.len() {
        6 if c[0] == c[1] && c[2] == c[3] && c[4] == c[5] => [c[0], c[2], c[4]].iter().collect(),
        4 if c[0] == c[1] && c[2] == c[3] => [c[0], c[2]].iter().collect(),
        2 if c[0] == c[1] => c[0].to_string(),
        _ => t.to_string(),
    }
}

/// Prelúdio de regra-arroba: espaço depois de `:` some (`max-width:600px`).
fn comprimir(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut espaco = false;
    for c in texto.trim().chars() {
        if c.is_whitespace() {
            espaco = true;
            continue;
        }
        if espaco && !saida.is_empty() && !saida.ends_with([':', '(']) && c != ')' {
            saida.push(' ');
        }
        espaco = false;
        saida.push(c);
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A saída exata do compilador oficial para o CSS do caso b15.
    #[test]
    fn folha_rica_igual_ao_oficial() {
        let css = "/* um comentário */\n:host {\n  display: block;\n}\n\n.a .b {\n  color: red;\n}\n\n.c, .d {\n  margin: 0 auto;\n}\n\na:hover {\n  text-decoration: underline;\n}\n\n@media (max-width: 600px) {\n  .a {\n    display: none;\n  }\n}\n";
        let esperado = "._nghost-%ID%{display:block}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:red}.c._ngcontent-%ID%,.d._ngcontent-%ID%{margin:0 auto}a:hover._ngcontent-%ID%{text-decoration:underline}@media (max-width:600px){.a._ngcontent-%ID%{display:none}}";
        assert_eq!(shim(css).unwrap(), esperado);
    }

    /// A do caso b07.
    #[test]
    fn folha_simples_igual_ao_oficial() {
        let css = ".c { color: red; }\nspan { font-weight: bold; }\n";
        assert_eq!(
            shim(css).unwrap(),
            ".c._ngcontent-%ID%{color:red}span._ngcontent-%ID%{font-weight:bold}"
        );
    }

    /// As formas do caso b16, uma a uma, contra a saída do oficial.
    #[test]
    fn formas_do_b16_iguais_ao_oficial() {
        let css = ".a::before {
  content: \"x\";
}

:host(.tema-escuro) .b {
  color: white;
}

:host-context(.pai) .c {
  color: red;
}

::ng-deep .d {
  color: blue;
}

.e > .f {
  margin: 0;
}

input[type=\"text\"] {
  border: 0;
}

@keyframes girar {
  from { opacity: 0; }
  to { opacity: 1; }
}

.g {
  animation: girar 1s;
}
";
        let esperado = ".a._ngcontent-%ID%::before{content:\"x\"}._nghost-%ID%.tema-escuro .b._ngcontent-%ID%{color:white}._nghost-%ID%.pai .c._ngcontent-%ID%,.pai ._nghost-%ID% .c._ngcontent-%ID%{color:red} .d{color:blue}.e._ngcontent-%ID% > .f._ngcontent-%ID%{margin:0}input[type=\"text\"]._ngcontent-%ID%{border:0}@keyframes girar{from{opacity:0}to{opacity:1}}.g._ngcontent-%ID%{animation:girar 1s}";
        assert_eq!(shim(css).unwrap(), esperado);
    }

    /// `assinatura_engine_selector_component` do new_sali: o Sass devolve
    /// `rgba(var(--success-rgb), 0.14)` e o oficial escreve sem o espaço; o
    /// `var` com valor padrão guarda o espaço, e `calc`/`min` ficam crus.
    #[test]
    fn virgulas_como_o_csslib() {
        assert_eq!(
            virgulas("rgba(var(--success-rgb), 0.14)").unwrap(),
            "rgba(var(--success-rgb),0.14)"
        );
        assert_eq!(virgulas("var(--a,#fff)").unwrap(), "var(--a, #fff)");
        assert_eq!(
            virgulas("var(--card-bg, var(--body-bg))").unwrap(),
            "var(--card-bg, var(--body-bg))"
        );
        assert_eq!(
            virgulas("min(100vw - 1rem,52rem)").unwrap(),
            "min(100vw - 1rem,52rem)"
        );
        assert_eq!(virgulas("\"a, b\", c").unwrap(), "\"a, b\",c");
        assert!(virgulas("var(--f, a, b)").is_err());
    }

    /// `pdf_signature_panel_body_component` do new_sali: o Sass devolve
    /// `light-dark(#ffffff, #2b2d33)` e o oficial escreve
    /// `light-dark(white,#2b2d33)`.
    #[test]
    fn cores_como_o_csslib() {
        assert_eq!(
            cores(&virgulas("light-dark(#ffffff, #2b2d33)").unwrap()),
            "light-dark(white,#2b2d33)"
        );
        assert_eq!(cores("#fff"), "#fff");
        assert_eq!(cores("#000"), "black");
        assert_eq!(cores("#aabbcc"), "#abc");
        assert_eq!(cores("1px solid grey"), "1px solid gray");
        assert_eq!(cores("calc(100% - 1px)"), "calc(100% - 1px)");
        assert_eq!(cores("\"white\""), "\"white\"");
        assert_eq!(cores("var(--body-color, #fff)"), "var(--body-color, #fff)");
        assert_eq!(
            urls("url(a/b.png) no-repeat").unwrap(),
            "url(\"a/b.png\") no-repeat"
        );
        assert_eq!(urls("url('a.png')").unwrap(), "url(\"a.png\")");
    }

    #[test]
    fn recusa_o_que_nao_sabe() {
        // Aninhamento é Sass, não CSS.
        assert!(shim(".a { .b { color: red; } }").is_err());
        assert!(shim("@font-face { font-family: x; }").is_err());
    }
}

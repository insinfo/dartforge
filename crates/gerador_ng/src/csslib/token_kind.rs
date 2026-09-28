//! Porte de `csslib/src/token_kind.dart` (csslib 1.0.2): os tipos de token
//! (`TokenKind`), as tabelas de diretivas, unidades, operadores de mídia e
//! cores, e as funções de busca nelas.
//!
//! As constantes são a tabela integral do `TokenKind`, na mesma numeração;
//! parte delas só existe para manter a numeração igual à oficial.
#![allow(dead_code)]

pub(crate) const UNUSED: i32 = 0;
pub(crate) const END_OF_FILE: i32 = 1;
pub(crate) const LPAREN: i32 = 2;
pub(crate) const RPAREN: i32 = 3;
pub(crate) const LBRACK: i32 = 4;
pub(crate) const RBRACK: i32 = 5;
pub(crate) const LBRACE: i32 = 6;
pub(crate) const RBRACE: i32 = 7;
pub(crate) const DOT: i32 = 8;
pub(crate) const SEMICOLON: i32 = 9;
pub(crate) const AT: i32 = 10;
pub(crate) const HASH: i32 = 11;
pub(crate) const PLUS: i32 = 12;
pub(crate) const GREATER: i32 = 13;
pub(crate) const TILDE: i32 = 14;
pub(crate) const ASTERISK: i32 = 15;
pub(crate) const NAMESPACE: i32 = 16;
pub(crate) const COLON: i32 = 17;
pub(crate) const PRIVATE_NAME: i32 = 18;
pub(crate) const COMMA: i32 = 19;
pub(crate) const SPACE: i32 = 20;
pub(crate) const TAB: i32 = 21;
pub(crate) const NEWLINE: i32 = 22;
pub(crate) const RETURN: i32 = 23;
pub(crate) const PERCENT: i32 = 24;
pub(crate) const SINGLE_QUOTE: i32 = 25;
pub(crate) const DOUBLE_QUOTE: i32 = 26;
pub(crate) const SLASH: i32 = 27;
pub(crate) const EQUALS: i32 = 28;
pub(crate) const CARET: i32 = 30;
pub(crate) const DOLLAR: i32 = 31;
pub(crate) const LESS: i32 = 32;
pub(crate) const BANG: i32 = 33;
pub(crate) const MINUS: i32 = 34;
pub(crate) const BACKSLASH: i32 = 35;
pub(crate) const AMPERSAND: i32 = 36;
pub(crate) const INTEGER: i32 = 60;
pub(crate) const HEX_INTEGER: i32 = 61;
pub(crate) const DOUBLE: i32 = 62;
pub(crate) const WHITESPACE: i32 = 63;
pub(crate) const COMMENT: i32 = 64;
pub(crate) const ERROR: i32 = 65;
pub(crate) const INCOMPLETE_STRING: i32 = 66;
pub(crate) const INCOMPLETE_COMMENT: i32 = 67;
pub(crate) const VAR_DEFINITION: i32 = 400;
pub(crate) const VAR_USAGE: i32 = 401;
pub(crate) const STRING: i32 = 500;
pub(crate) const STRING_PART: i32 = 501;
pub(crate) const NUMBER: i32 = 502;
pub(crate) const HEX_NUMBER: i32 = 503;
pub(crate) const HTML_COMMENT: i32 = 504;
pub(crate) const IMPORTANT: i32 = 505;
pub(crate) const CDATA_START: i32 = 506;
pub(crate) const CDATA_END: i32 = 507;
pub(crate) const UNICODE_RANGE: i32 = 508;
pub(crate) const HEX_RANGE: i32 = 509;
pub(crate) const IDENTIFIER: i32 = 511;
pub(crate) const SELECTOR_EXPRESSION: i32 = 512;
pub(crate) const COMBINATOR_NONE: i32 = 513;
pub(crate) const COMBINATOR_DESCENDANT: i32 = 514;
pub(crate) const COMBINATOR_PLUS: i32 = 515;
pub(crate) const COMBINATOR_GREATER: i32 = 516;
pub(crate) const COMBINATOR_TILDE: i32 = 517;
pub(crate) const UNARY_OP_NONE: i32 = 518;
pub(crate) const INCLUDES: i32 = 530;
pub(crate) const DASH_MATCH: i32 = 531;
pub(crate) const PREFIX_MATCH: i32 = 532;
pub(crate) const SUFFIX_MATCH: i32 = 533;
pub(crate) const SUBSTRING_MATCH: i32 = 534;
pub(crate) const NO_MATCH: i32 = 535;
pub(crate) const UNIT_EM: i32 = 600;
pub(crate) const UNIT_EX: i32 = 601;
pub(crate) const UNIT_LENGTH_PX: i32 = 602;
pub(crate) const UNIT_LENGTH_CM: i32 = 603;
pub(crate) const UNIT_LENGTH_MM: i32 = 604;
pub(crate) const UNIT_LENGTH_IN: i32 = 605;
pub(crate) const UNIT_LENGTH_PT: i32 = 606;
pub(crate) const UNIT_LENGTH_PC: i32 = 607;
pub(crate) const UNIT_ANGLE_DEG: i32 = 608;
pub(crate) const UNIT_ANGLE_RAD: i32 = 609;
pub(crate) const UNIT_ANGLE_GRAD: i32 = 610;
pub(crate) const UNIT_ANGLE_TURN: i32 = 611;
pub(crate) const UNIT_TIME_MS: i32 = 612;
pub(crate) const UNIT_TIME_S: i32 = 613;
pub(crate) const UNIT_FREQ_HZ: i32 = 614;
pub(crate) const UNIT_FREQ_KHZ: i32 = 615;
pub(crate) const UNIT_PERCENT: i32 = 616;
pub(crate) const UNIT_FRACTION: i32 = 617;
pub(crate) const UNIT_RESOLUTION_DPI: i32 = 618;
pub(crate) const UNIT_RESOLUTION_DPCM: i32 = 619;
pub(crate) const UNIT_RESOLUTION_DPPX: i32 = 620;
pub(crate) const UNIT_CH: i32 = 621;
pub(crate) const UNIT_REM: i32 = 622;
pub(crate) const UNIT_VIEWPORT_VW: i32 = 623;
pub(crate) const UNIT_VIEWPORT_VH: i32 = 624;
pub(crate) const UNIT_VIEWPORT_VMIN: i32 = 625;
pub(crate) const UNIT_VIEWPORT_VMAX: i32 = 626;
pub(crate) const UNIT_LH: i32 = 627;
pub(crate) const UNIT_RLH: i32 = 628;
pub(crate) const DIRECTIVE_NONE: i32 = 640;
pub(crate) const DIRECTIVE_IMPORT: i32 = 641;
pub(crate) const DIRECTIVE_MEDIA: i32 = 642;
pub(crate) const DIRECTIVE_PAGE: i32 = 643;
pub(crate) const DIRECTIVE_CHARSET: i32 = 644;
pub(crate) const DIRECTIVE_STYLET: i32 = 645;
pub(crate) const DIRECTIVE_KEYFRAMES: i32 = 646;
pub(crate) const DIRECTIVE_WEB_KIT_KEYFRAMES: i32 = 647;
pub(crate) const DIRECTIVE_MOZ_KEYFRAMES: i32 = 648;
pub(crate) const DIRECTIVE_MS_KEYFRAMES: i32 = 649;
pub(crate) const DIRECTIVE_O_KEYFRAMES: i32 = 650;
pub(crate) const DIRECTIVE_FONTFACE: i32 = 651;
pub(crate) const DIRECTIVE_NAMESPACE: i32 = 652;
pub(crate) const DIRECTIVE_HOST: i32 = 653;
pub(crate) const DIRECTIVE_MIXIN: i32 = 654;
pub(crate) const DIRECTIVE_INCLUDE: i32 = 655;
pub(crate) const DIRECTIVE_CONTENT: i32 = 656;
pub(crate) const DIRECTIVE_EXTEND: i32 = 657;
pub(crate) const DIRECTIVE_MOZ_DOCUMENT: i32 = 658;
pub(crate) const DIRECTIVE_SUPPORTS: i32 = 659;
pub(crate) const DIRECTIVE_VIEWPORT: i32 = 660;
pub(crate) const DIRECTIVE_MS_VIEWPORT: i32 = 661;
pub(crate) const MEDIA_OP_ONLY: i32 = 665;
pub(crate) const MEDIA_OP_NOT: i32 = 666;
pub(crate) const MEDIA_OP_AND: i32 = 667;
pub(crate) const MARGIN_DIRECTIVE_TOPLEFTCORNER: i32 = 670;
pub(crate) const MARGIN_DIRECTIVE_TOPLEFT: i32 = 671;
pub(crate) const MARGIN_DIRECTIVE_TOPCENTER: i32 = 672;
pub(crate) const MARGIN_DIRECTIVE_TOPRIGHT: i32 = 673;
pub(crate) const MARGIN_DIRECTIVE_TOPRIGHTCORNER: i32 = 674;
pub(crate) const MARGIN_DIRECTIVE_BOTTOMLEFTCORNER: i32 = 675;
pub(crate) const MARGIN_DIRECTIVE_BOTTOMLEFT: i32 = 676;
pub(crate) const MARGIN_DIRECTIVE_BOTTOMCENTER: i32 = 677;
pub(crate) const MARGIN_DIRECTIVE_BOTTOMRIGHT: i32 = 678;
pub(crate) const MARGIN_DIRECTIVE_BOTTOMRIGHTCORNER: i32 = 679;
pub(crate) const MARGIN_DIRECTIVE_LEFTTOP: i32 = 680;
pub(crate) const MARGIN_DIRECTIVE_LEFTMIDDLE: i32 = 681;
pub(crate) const MARGIN_DIRECTIVE_LEFTBOTTOM: i32 = 682;
pub(crate) const MARGIN_DIRECTIVE_RIGHTTOP: i32 = 683;
pub(crate) const MARGIN_DIRECTIVE_RIGHTMIDDLE: i32 = 684;
pub(crate) const MARGIN_DIRECTIVE_RIGHTBOTTOM: i32 = 685;
pub(crate) const CLASS_NAME: i32 = 700;
pub(crate) const ELEMENT_NAME: i32 = 701;
pub(crate) const HASH_NAME: i32 = 702;
pub(crate) const ATTRIBUTE_NAME: i32 = 703;
pub(crate) const PSEUDO_ELEMENT_NAME: i32 = 704;
pub(crate) const PSEUDO_CLASS_NAME: i32 = 705;
pub(crate) const NEGATION: i32 = 706;
pub(crate) const ASCII_UPPER_A: i32 = 65;
pub(crate) const ASCII_UPPER_Z: i32 = 90;

/// `TokenKind._DIRECTIVES`.
const DIRETIVAS: &[(i32, &str)] = &[
    (DIRECTIVE_IMPORT, "import"),
    (DIRECTIVE_MEDIA, "media"),
    (DIRECTIVE_PAGE, "page"),
    (DIRECTIVE_CHARSET, "charset"),
    (DIRECTIVE_STYLET, "stylet"),
    (DIRECTIVE_KEYFRAMES, "keyframes"),
    (DIRECTIVE_WEB_KIT_KEYFRAMES, "-webkit-keyframes"),
    (DIRECTIVE_MOZ_KEYFRAMES, "-moz-keyframes"),
    (DIRECTIVE_MS_KEYFRAMES, "-ms-keyframes"),
    (DIRECTIVE_O_KEYFRAMES, "-o-keyframes"),
    (DIRECTIVE_FONTFACE, "font-face"),
    (DIRECTIVE_NAMESPACE, "namespace"),
    (DIRECTIVE_HOST, "host"),
    (DIRECTIVE_MIXIN, "mixin"),
    (DIRECTIVE_INCLUDE, "include"),
    (DIRECTIVE_CONTENT, "content"),
    (DIRECTIVE_EXTEND, "extend"),
    (DIRECTIVE_MOZ_DOCUMENT, "-moz-document"),
    (DIRECTIVE_SUPPORTS, "supports"),
    (DIRECTIVE_VIEWPORT, "viewport"),
    (DIRECTIVE_MS_VIEWPORT, "-ms-viewport"),
];

/// `TokenKind.MEDIA_OPERATORS`.
pub(crate) const MEDIA_OPERATORS: &[(i32, &str)] = &[
    (MEDIA_OP_ONLY, "only"),
    (MEDIA_OP_NOT, "not"),
    (MEDIA_OP_AND, "and"),
];

/// `TokenKind.MARGIN_DIRECTIVES` — com o `left-bottom` que o oficial
/// escreve `right-bottom`.
pub(crate) const MARGIN_DIRECTIVES: &[(i32, &str)] = &[
    (MARGIN_DIRECTIVE_TOPLEFTCORNER, "top-left-corner"),
    (MARGIN_DIRECTIVE_TOPLEFT, "top-left"),
    (MARGIN_DIRECTIVE_TOPCENTER, "top-center"),
    (MARGIN_DIRECTIVE_TOPRIGHT, "top-right"),
    (MARGIN_DIRECTIVE_TOPRIGHTCORNER, "top-right-corner"),
    (MARGIN_DIRECTIVE_BOTTOMLEFTCORNER, "bottom-left-corner"),
    (MARGIN_DIRECTIVE_BOTTOMLEFT, "bottom-left"),
    (MARGIN_DIRECTIVE_BOTTOMCENTER, "bottom-center"),
    (MARGIN_DIRECTIVE_BOTTOMRIGHT, "bottom-right"),
    (MARGIN_DIRECTIVE_BOTTOMRIGHTCORNER, "bottom-right-corner"),
    (MARGIN_DIRECTIVE_LEFTTOP, "left-top"),
    (MARGIN_DIRECTIVE_LEFTMIDDLE, "left-middle"),
    (MARGIN_DIRECTIVE_LEFTBOTTOM, "right-bottom"),
    (MARGIN_DIRECTIVE_RIGHTTOP, "right-top"),
    (MARGIN_DIRECTIVE_RIGHTMIDDLE, "right-middle"),
    (MARGIN_DIRECTIVE_RIGHTBOTTOM, "right-bottom"),
];

/// `TokenKind._UNITS`.
const UNIDADES: &[(i32, &str)] = &[
    (UNIT_EM, "em"),
    (UNIT_EX, "ex"),
    (UNIT_LENGTH_PX, "px"),
    (UNIT_LENGTH_CM, "cm"),
    (UNIT_LENGTH_MM, "mm"),
    (UNIT_LENGTH_IN, "in"),
    (UNIT_LENGTH_PT, "pt"),
    (UNIT_LENGTH_PC, "pc"),
    (UNIT_ANGLE_DEG, "deg"),
    (UNIT_ANGLE_RAD, "rad"),
    (UNIT_ANGLE_GRAD, "grad"),
    (UNIT_ANGLE_TURN, "turn"),
    (UNIT_TIME_MS, "ms"),
    (UNIT_TIME_S, "s"),
    (UNIT_FREQ_HZ, "hz"),
    (UNIT_FREQ_KHZ, "khz"),
    (UNIT_FRACTION, "fr"),
    (UNIT_RESOLUTION_DPI, "dpi"),
    (UNIT_RESOLUTION_DPCM, "dpcm"),
    (UNIT_RESOLUTION_DPPX, "dppx"),
    (UNIT_CH, "ch"),
    (UNIT_REM, "rem"),
    (UNIT_VIEWPORT_VW, "vw"),
    (UNIT_VIEWPORT_VH, "vh"),
    (UNIT_VIEWPORT_VMIN, "vmin"),
    (UNIT_VIEWPORT_VMAX, "vmax"),
    (UNIT_LH, "lh"),
    (UNIT_RLH, "rlh"),
];

/// `TokenKind._EXTENDED_COLOR_NAMES`, na ordem oficial (a busca por valor
/// devolve o primeiro nome: `aqua` antes de `cyan`, `gray` antes de `grey`).
const CORES_ESTENDIDAS: &[(&str, i64)] = &[
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

/// `TokenKind.matchList`: compara `texto` (unidades UTF-16) com cada nome da
/// lista; maiúscula ASCII casa com a minúscula da tabela.
fn match_list(lista: &[(i32, &str)], texto: &[u16]) -> i32 {
    for &(tipo, nome) in lista {
        let nome = nome.as_bytes();
        if texto.len() != nome.len() {
            continue;
        }
        let casa = texto.iter().zip(nome).all(|(&c, &n)| {
            let n = u16::from(n);
            c == n || ((ASCII_UPPER_A as u16..=ASCII_UPPER_Z as u16).contains(&c) && c + 32 == n)
        });
        if casa {
            return tipo;
        }
    }
    -1
}

/// `TokenKind.matchUnits`.
pub(crate) fn match_units(texto: &[u16]) -> i32 {
    match_list(UNIDADES, texto)
}

/// `TokenKind.matchDirectives`.
pub(crate) fn match_directives(texto: &[u16]) -> i32 {
    match_list(DIRETIVAS, texto)
}

/// `TokenKind.matchMarginDirectives`.
pub(crate) fn match_margin_directives(texto: &[u16]) -> i32 {
    match_list(MARGIN_DIRECTIVES, texto)
}

/// `TokenKind.matchMediaOperator`.
pub(crate) fn match_media_operator(texto: &[u16]) -> i32 {
    match_list(MEDIA_OPERATORS, texto)
}

/// `TokenKind.idToValue`.
pub(crate) fn id_to_value(lista: &[(i32, &'static str)], id: i32) -> Option<&'static str> {
    lista.iter().find(|(t, _)| *t == id).map(|(_, v)| *v)
}

/// `TokenKind.unitToString`.
pub(crate) fn unit_to_string(unidade: i32) -> &'static str {
    if unidade == PERCENT {
        return "%";
    }
    UNIDADES
        .iter()
        .find(|(u, _)| *u == unidade)
        .map(|(_, v)| *v)
        .unwrap_or("<BAD UNIT>")
}

/// `TokenKind.matchColorName`: o valor da cor de nome `texto` (sem
/// distinguir caixa).
pub(crate) fn match_color_name(texto: &str) -> Option<i64> {
    let nome = texto.to_lowercase();
    CORES_ESTENDIDAS
        .iter()
        .find(|(n, _)| *n == nome)
        .map(|(_, v)| *v)
}

/// `TokenKind.hexToColorName`.
pub(crate) fn hex_to_color_name(valor: i64) -> Option<&'static str> {
    CORES_ESTENDIDAS
        .iter()
        .find(|(_, v)| *v == valor)
        .map(|(n, _)| *n)
}

/// `TokenKind.decimalToHex`.
pub(crate) fn decimal_to_hex(numero: i64, min_digitos: usize) -> String {
    const DIGITOS: &[u8] = b"0123456789abcdef";
    let mut resultado = Vec::new();
    let mut dividendo = numero >> 4;
    resultado.push(DIGITOS[numero.rem_euclid(16) as usize]);
    while dividendo != 0 {
        resultado.push(DIGITOS[dividendo.rem_euclid(16) as usize]);
        dividendo >>= 4;
    }
    let mut saida = String::new();
    for _ in resultado.len()..min_digitos {
        saida.push('0');
    }
    for &d in resultado.iter().rev() {
        saida.push(char::from(d));
    }
    saida
}

/// `TokenKind.kindToString`: `None` onde o oficial lança `StateError`.
pub(crate) fn kind_to_string(tipo: i32) -> Option<&'static str> {
    Some(match tipo {
        UNUSED => "ERROR",
        END_OF_FILE => "end of file",
        LPAREN => "(",
        RPAREN => ")",
        LBRACK => "[",
        RBRACK => "]",
        LBRACE => "{",
        RBRACE => "}",
        DOT => ".",
        SEMICOLON => ";",
        AT => "@",
        HASH => "#",
        PLUS => "+",
        GREATER => ">",
        TILDE => "~",
        ASTERISK => "*",
        NAMESPACE => "|",
        COLON => ":",
        PRIVATE_NAME => "_",
        COMMA => ",",
        SPACE => " ",
        TAB => "\t",
        NEWLINE => "\n",
        RETURN => "\r",
        PERCENT => "%",
        SINGLE_QUOTE => "'",
        DOUBLE_QUOTE => "\"",
        SLASH => "/",
        EQUALS => "=",
        CARET => "^",
        DOLLAR => "$",
        LESS => "<",
        BANG => "!",
        MINUS => "-",
        BACKSLASH => "\\",
        _ => return None,
    })
}

/// `TokenKind.isKindIdentifier`.
pub(crate) fn is_kind_identifier(tipo: i32) -> bool {
    matches!(
        tipo,
        DIRECTIVE_IMPORT
            | DIRECTIVE_MEDIA
            | DIRECTIVE_PAGE
            | DIRECTIVE_CHARSET
            | DIRECTIVE_STYLET
            | DIRECTIVE_KEYFRAMES
            | DIRECTIVE_WEB_KIT_KEYFRAMES
            | DIRECTIVE_MOZ_KEYFRAMES
            | DIRECTIVE_MS_KEYFRAMES
            | DIRECTIVE_O_KEYFRAMES
            | DIRECTIVE_FONTFACE
            | DIRECTIVE_NAMESPACE
            | DIRECTIVE_HOST
            | DIRECTIVE_MIXIN
            | DIRECTIVE_INCLUDE
            | DIRECTIVE_CONTENT
            | UNIT_EM
            | UNIT_EX
            | UNIT_LENGTH_PX
            | UNIT_LENGTH_CM
            | UNIT_LENGTH_MM
            | UNIT_LENGTH_IN
            | UNIT_LENGTH_PT
            | UNIT_LENGTH_PC
            | UNIT_ANGLE_DEG
            | UNIT_ANGLE_RAD
            | UNIT_ANGLE_GRAD
            | UNIT_TIME_MS
            | UNIT_TIME_S
            | UNIT_FREQ_HZ
            | UNIT_FREQ_KHZ
            | UNIT_FRACTION
            | UNIT_LH
            | UNIT_RLH
    )
}

/// `TokenKind.isIdentifier`.
pub(crate) fn is_identifier(tipo: i32) -> bool {
    tipo == IDENTIFIER
}

//! O `ManifestValidator` do analyzer
//! (`analyzer/lib/src/manifest/manifest_validator.dart` e
//! `manifest_values.dart`, lidos por inteiro na 6.11.0;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.10): as checagens de
//! Chrome OS sobre o `AndroidManifest.xml`, que só rodam com
//! `analyzer: optional-checks: chrome-os-manifest-checks`.
//!
//! O analisador de XML é o do original, transcrito: só guarda os elementos
//! `manifest`, `application`, `activity`, `uses-feature` e
//! `uses-permission`, com nomes de marca e de atributo em minúsculas.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g::manifesto as c;
use super::Relato;
use dartforge_diagnostics::Span;

const RELEVANTES: [&str; 5] = ["activity", "application", "manifest", "uses-feature", "uses-permission"];

const HARDWARE_SEM_SUPORTE: [&str; 37] = [
    "android.hardware.camera",
    "android.hardware.camera.autofocus",
    "android.hardware.camera.capability.manual_post_processing",
    "android.hardware.camera.capability.manual_sensor",
    "android.hardware.camera.capability.raw",
    "android.hardware.camera.flash",
    "android.hardware.camera.level.full",
    "android.hardware.consumerir",
    "android.hardware.location.gps",
    "android.hardware.nfc",
    "android.hardware.nfc.hce",
    "android.hardware.sensor.barometer",
    "android.hardware.telephony",
    "android.hardware.telephony.cdma",
    "android.hardware.telephony.gsm",
    "android.hardware.type.automotive",
    "android.hardware.type.television",
    "android.hardware.usb.accessory",
    "android.hardware.usb.host",
    "android.hardware.sensor.accelerometer",
    "android.hardware.sensor.compass",
    "android.hardware.sensor.gyroscope",
    "android.hardware.sensor.light",
    "android.hardware.sensor.proximity",
    "android.hardware.sensor.stepcounter",
    "android.hardware.sensor.stepdetector",
    "android.software.app_widgets",
    "android.software.device_admin",
    "android.software.home_screen",
    "android.software.input_methods",
    "android.software.leanback",
    "android.software.live_wallpaper",
    "android.software.live_tv",
    "android.software.managed_users",
    "android.software.midi",
    "android.software.sip",
    "android.software.sip.voip",
];

const ORIENTACOES_SEM_SUPORTE: [&str; 8] =
    ["landscape", "portrait", "reverseLandscape", "reversePortrait", "sensorLandscape", "sensorPortrait", "userLandscape", "userPortrait"];

/// `getImpliedUnsupportedHardware`: o hardware que a permissão supõe.
fn hardware_da_permissao(permissao: &str) -> Option<&'static str> {
    match permissao {
        "android.permission.CAMERA" => Some("android.hardware.camera"),
        "android.permission.CALL_PHONE"
        | "android.permission.CALL_PRIVILEGED"
        | "android.permission.MODIFY_PHONE_STATE"
        | "android.permission.PROCESS_OUTGOING_CALLS"
        | "android.permission.READ_SMS"
        | "android.permission.RECEIVE_SMS"
        | "android.permission.RECEIVE_MMS"
        | "android.permission.RECEIVE_WAP_PUSH"
        | "android.permission.SEND_SMS"
        | "android.permission.WRITE_APN_SETTINGS"
        | "android.permission.WRITE_SMS" => Some("android.hardware.telephony"),
        _ => None,
    }
}

struct Atributo {
    nome: String,
    valor: String,
    /// Do nome à aspa de fecho, exclusive.
    span: Span,
}

struct Elemento {
    nome: String,
    atributos: Vec<Atributo>,
    filhos: Vec<Elemento>,
    span: Span,
}

impl Elemento {
    fn atributo(&self, nome: &str) -> Option<&Atributo> {
        // Um nome repetido: o último vale (o mapa do original).
        self.atributos.iter().rev().find(|a| a.nome == nome)
    }

    fn valor(&self, nome: &str) -> Option<&str> {
        self.atributo(nome).map(|a| a.valor.as_str())
    }
}

/// O que `parseXmlTag` achou.
enum Marca {
    /// Um elemento; `None` quando não é dos relevantes (ou é comentário ou
    /// declaração).
    Elemento(Option<Elemento>),
    Fecho,
    Fim,
    Erro,
}

/// O fecho de uma lista de atributos.
enum Fecho {
    Marca,
    ElementoVazio,
}

struct Leitor<'a> {
    b: &'a [u8],
    fonte: &'a str,
    pos: usize,
}

impl Leitor<'_> {
    fn fechando(&self) -> bool {
        self.b.get(self.pos) == Some(&b'>')
    }

    fn fechando_em_dois(&self) -> bool {
        self.pos + 1 < self.b.len() && (self.b[self.pos] == b'?' || self.b[self.pos] == b'/') && self.b[self.pos + 1] == b'>'
    }

    fn branco(&self) -> bool {
        self.b.get(self.pos).is_some_and(u8::is_ascii_whitespace)
    }

    /// `parseXmlTag`.
    fn marca(&mut self) -> Marca {
        while self.pos < self.b.len() && self.b[self.pos] != b'<' {
            self.pos += 1;
        }
        if self.pos >= self.b.len() {
            return Marca::Fim;
        }
        let p = self.pos;
        if p + 3 < self.b.len() && &self.b[p + 1..p + 4] == b"!--" {
            // Comentário: até `-->`.
            self.pos += 4;
            loop {
                if self.pos + 2 >= self.b.len() {
                    return Marca::Erro;
                }
                if &self.b[self.pos..self.pos + 3] == b"-->" {
                    break;
                }
                self.pos += 1;
            }
            self.pos += 2;
            return Marca::Elemento(None);
        }
        if p + 1 < self.b.len() && self.b[p + 1] == b'!' {
            // Declaração: até `>`.
            self.pos += 2;
            while !self.fechando() {
                self.pos += 1;
                if self.pos >= self.b.len() {
                    return Marca::Erro;
                }
            }
            return Marca::Elemento(None);
        }
        self.normal()
    }

    /// `_parseAttribute`.
    fn atributos(&mut self, relevante: bool) -> Option<(Vec<Atributo>, Fecho)> {
        let mut saida = Vec::new();
        let fecho = loop {
            if self.pos >= self.b.len() {
                return None;
            }
            if self.fechando() {
                break Fecho::Marca;
            } else if self.fechando_em_dois() {
                self.pos += 1;
                break Fecho::ElementoVazio;
            } else if !self.branco() {
                return None;
            }
            while self.branco() {
                self.pos += 1;
            }
            if self.pos >= self.b.len() {
                return None;
            }
            if self.fechando() {
                break Fecho::Marca;
            } else if self.fechando_em_dois() {
                self.pos += 1;
                break Fecho::ElementoVazio;
            }
            // O nome, até o `=`.
            let inicio_do_nome = self.pos;
            self.pos += 1;
            loop {
                if self.pos >= self.b.len() {
                    return None;
                }
                if self.b[self.pos] == b'=' {
                    break;
                }
                // Atributo sem valor não existe em XML.
                if self.branco() || self.fechando() || self.fechando_em_dois() {
                    return None;
                }
                self.pos += 1;
            }
            let nome = self.fonte.get(inicio_do_nome..self.pos)?.to_lowercase();
            self.pos += 1;
            // O valor, entre aspas.
            let aspa = *self.b.get(self.pos)?;
            if aspa != b'\'' && aspa != b'"' {
                return None;
            }
            self.pos += 1;
            let inicio_do_valor = self.pos;
            while *self.b.get(self.pos)? != aspa {
                self.pos += 1;
            }
            if relevante {
                let valor = self.fonte.get(inicio_do_valor..self.pos)?.to_string();
                saida.push(Atributo { nome, valor, span: Span { start: inicio_do_nome, end: self.pos } });
            }
            self.pos += 1;
        };
        Some((saida, fecho))
    }

    /// `_parseNormalTag`.
    fn normal(&mut self) -> Marca {
        let inicio = self.pos;
        self.pos += 1;
        if self.pos >= self.b.len() || self.branco() {
            return Marca::Erro;
        }
        let de_fecho = self.b[self.pos] == b'/';
        if de_fecho {
            self.pos += 1;
        }
        let inicio_do_nome = self.pos;
        while !self.fechando() && !self.fechando_em_dois() && !self.branco() {
            self.pos += 1;
            if self.pos >= self.b.len() {
                return Marca::Erro;
            }
        }
        let Some(nome) = self.fonte.get(inicio_do_nome..self.pos).map(str::to_lowercase) else { return Marca::Erro };
        // `None`: ainda não fechou; `Some(vazio)`: fechou a marca.
        let mut fechada: Option<bool> = None;
        if self.fechando() {
            fechada = Some(false);
        } else if self.fechando_em_dois() {
            fechada = Some(true);
            self.pos += 1;
        }
        if de_fecho {
            while self.branco() {
                self.pos += 1;
            }
            // Marca de fecho não tem atributos.
            return if self.fechando() { Marca::Fecho } else { Marca::Erro };
        }
        let relevante = RELEVANTES.contains(&nome.as_str());
        let (atributos, mut vazio) = match fechada {
            Some(vazio) => (Vec::new(), vazio),
            None => match self.atributos(relevante) {
                Some((atributos, fecho)) => (atributos, matches!(fecho, Fecho::ElementoVazio)),
                None => return Marca::Erro,
            },
        };
        if nome.starts_with('!') {
            vazio = true;
        }
        let mut filhos = Vec::new();
        if !vazio {
            self.pos += 1;
            loop {
                match self.marca() {
                    Marca::Fecho => break,
                    Marca::Fim => return Marca::Fim,
                    Marca::Erro => return Marca::Erro,
                    Marca::Elemento(None) => {}
                    Marca::Elemento(Some(filho)) => {
                        filhos.push(filho);
                        self.pos += 1;
                    }
                }
            }
        }
        if relevante {
            let fim = (self.pos + 1).min(self.b.len());
            Marca::Elemento(Some(Elemento { nome, atributos, filhos, span: Span { start: inicio, end: fim } }))
        } else {
            Marca::Elemento(None)
        }
    }
}

/// `_reportErrorForNode`: no atributo `chave`, ou no elemento inteiro.
fn relatar(out: &mut Vec<Relato>, elemento: &Elemento, chave: Option<&str>, codigo: &'static super::CodigoNaoDart, args: &[&str]) {
    let span = chave.and_then(|k| elemento.atributo(k)).map_or(elemento.span, |a| a.span);
    out.push(Relato::novo(codigo, span, args));
}

/// `ManifestValidator.validate` com `checkManifest` ligado: os relatos do
/// `AndroidManifest.xml` de texto `fonte`.
pub fn validar(fonte: &str) -> Vec<Relato> {
    let mut out = Vec::new();
    let mut leitor = Leitor { b: fonte.as_bytes(), fonte, pos: 0 };
    // Até o elemento `manifest`.
    let manifesto = loop {
        match leitor.marca() {
            Marca::Fim | Marca::Erro => return out,
            Marca::Elemento(Some(e)) if e.nome == "manifest" => break e,
            _ => {}
        }
    };
    let nome = "android:name";
    let exigido = "android:required";
    let recursos: Vec<&Elemento> = manifesto.filhos.iter().filter(|e| e.nome == "uses-feature").collect();
    let permissoes: Vec<&Elemento> = manifesto.filhos.iter().filter(|e| e.nome == "uses-permission").collect();
    // `_validateTouchScreenFeature`.
    let tela = "android.hardware.touchscreen";
    match recursos.iter().find(|e| e.valor(nome) == Some(tela)) {
        Some(recurso) => {
            if recurso.atributo(exigido).is_none() {
                relatar(&mut out, recurso, Some(nome), &c::UNSUPPORTED_CHROME_OS_HARDWARE, &[tela]);
            } else if recurso.valor(exigido) == Some("true") {
                relatar(&mut out, recurso, Some(nome), &c::UNSUPPORTED_CHROME_OS_FEATURE, &[tela]);
            }
        }
        None => relatar(&mut out, &manifesto, None, &c::NO_TOUCHSCREEN_FEATURE, &[]),
    }
    // `_validateFeatures`.
    for recurso in &recursos {
        let Some(valor) = recurso.valor(nome).filter(|v| HARDWARE_SEM_SUPORTE.contains(v)) else { continue };
        if recurso.atributo(exigido).is_none() {
            relatar(&mut out, recurso, Some(nome), &c::UNSUPPORTED_CHROME_OS_HARDWARE, &[valor]);
        } else if recurso.valor(exigido) == Some("true") {
            relatar(&mut out, recurso, Some(nome), &c::UNSUPPORTED_CHROME_OS_FEATURE, &[valor]);
        }
    }
    // `_validatePermissions`.
    let tem = |recurso: &str| recursos.iter().any(|e| e.valor(nome) == Some(recurso));
    for permissao in &permissoes {
        if permissao.valor(nome) == Some("android.permission.CAMERA") {
            if !tem("android.hardware.camera") || !tem("android.hardware.camera.autofocus") {
                relatar(&mut out, permissao, Some(nome), &c::CAMERA_PERMISSIONS_INCOMPATIBLE, &[]);
            }
        } else if let Some(hardware) = permissao.valor(nome).and_then(hardware_da_permissao) {
            relatar(&mut out, permissao, Some(nome), &c::PERMISSION_IMPLIES_UNSUPPORTED_HARDWARE, &[hardware]);
        }
    }
    // `_validateActivity`, nas atividades da primeira `application`.
    if let Some(aplicacao) = manifesto.filhos.iter().find(|e| e.nome == "application") {
        for atividade in aplicacao.filhos.iter().filter(|e| e.nome == "activity") {
            let orientacao = "android:screenorientation";
            if atividade.valor(orientacao).is_some_and(|v| ORIENTACOES_SEM_SUPORTE.contains(&v)) {
                relatar(&mut out, atividade, Some(orientacao), &c::SETTING_ORIENTATION_ON_ACTIVITY, &[]);
            }
            let redimensionavel = "android:resizeableactivity";
            if atividade.valor(redimensionavel) == Some("false") {
                relatar(&mut out, atividade, Some(redimensionavel), &c::NON_RESIZABLE_ACTIVITY, &[]);
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        validar(fonte).into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn sem_tela_de_toque() {
        let fonte = "<manifest>\n</manifest>\n";
        assert_eq!(achados(fonte), vec![("no_touchscreen_feature", "<manifest>\n</manifest>".to_string())]);
    }

    #[test]
    fn recursos_permissoes_e_atividades() {
        let fonte = concat!(
            "<?xml version=\"1.0\"?>\n",
            "<!-- comentário -->\n",
            "<manifest xmlns:android=\"x\">\n",
            "  <uses-feature android:name=\"android.hardware.touchscreen\" android:required=\"false\" />\n",
            "  <uses-feature android:name=\"android.hardware.nfc\" />\n",
            "  <uses-permission android:name=\"android.permission.CAMERA\" />\n",
            "  <uses-permission android:name=\"android.permission.SEND_SMS\" />\n",
            "  <application>\n",
            "    <activity android:screenOrientation=\"portrait\" android:resizeableActivity=\"false\"></activity>\n",
            "  </application>\n",
            "</manifest>\n",
        );
        assert_eq!(
            achados(fonte),
            vec![
                ("unsupported_chrome_os_hardware", "android:name=\"android.hardware.nfc".to_string()),
                ("camera_permissions_incompatible", "android:name=\"android.permission.CAMERA".to_string()),
                ("permission_implies_unsupported_hardware", "android:name=\"android.permission.SEND_SMS".to_string()),
                ("setting_orientation_on_activity", "android:screenOrientation=\"portrait".to_string()),
                ("non_resizable_activity", "android:resizeableActivity=\"false".to_string()),
            ]
        );
    }

    #[test]
    fn xml_quebrado_nao_relata() {
        assert!(achados("<manifest attr>").is_empty());
        assert!(achados("sem marcas").is_empty());
    }
}

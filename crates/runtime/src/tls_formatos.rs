// Runtime nativo: os formatos de chave e certificado do `SecurityContext`
// (`runtime/bin/security_context.cc` da VM, sobre o BoringSSL), para o
// fragmento `tls`:
//
// * PEM, com a ordem de decisão da VM: os blocos PEM primeiro; só quando não
//   há a linha de início do bloco procurado (`PEM_R_NO_START_LINE`) os bytes
//   são lidos como PKCS#12 — um DER solto (certificado ou chave) não é aceito,
//   como na VM;
// * chave privada cifrada: PKCS#8 (`ENCRYPTED PRIVATE KEY`, PBES2 ou as PBE
//   do PKCS#12) e o PEM cifrado legado do OpenSSL (`Proc-Type: 4,ENCRYPTED`,
//   `DEK-Info`, chave pelo `EVP_BytesToKey` com MD5);
// * PKCS#12: a MAC conferida (senha errada é `INCORRECT_PASSWORD`), as
//   `SafeContents` abertas ou cifradas, sacos de chave (abertos e cifrados),
//   de certificado e aninhados; BER de comprimento indefinido também.
//
// Os erros levam os textos do BoringSSL (a razão e o local), que a VM põe no
// `OSError` da `TlsException`.

/// Um valor DER/BER: a etiqueta e o conteúdo (já sem comprimento
/// indefinido nem `OCTET STRING` segmentada quando isso importa).
struct Tlv<'a> {
    etiqueta: u8,
    conteudo: std::borrow::Cow<'a, [u8]>,
}

const ETIQUETA_INTEIRO: u8 = 0x02;
const ETIQUETA_OCTETOS: u8 = 0x04;
const ETIQUETA_OID: u8 = 0x06;
const ETIQUETA_SEQUENCIA: u8 = 0x30;
const ETIQUETA_CONJUNTO: u8 = 0x31;

/// Lê um TLV de `b`: o valor e o resto. Aceita o comprimento indefinido do
/// BER (até o `00 00`), e junta os pedaços de uma `OCTET STRING`
/// construída.
fn ler_tlv(b: &[u8]) -> Option<(Tlv<'_>, &[u8])> {
    let (&etiqueta, resto) = b.split_first()?;
    if etiqueta & 0x1f == 0x1f {
        return None; // etiquetas de vários bytes não aparecem nestes formatos
    }
    let (&c, mut resto) = resto.split_first()?;
    if c == 0x80 {
        // Comprimento indefinido: os filhos até o fim de conteúdo.
        if etiqueta & 0x20 == 0 {
            return None;
        }
        let inicio = resto;
        let mut filhos = Vec::new();
        loop {
            if resto.starts_with(&[0, 0]) {
                let usado = inicio.len() - resto.len();
                let conteudo = if etiqueta == ETIQUETA_OCTETOS | 0x20 {
                    std::borrow::Cow::Owned(filhos.concat())
                } else {
                    std::borrow::Cow::Owned(reescrever_definido(&inicio[..usado])?)
                };
                let etiqueta = if etiqueta == ETIQUETA_OCTETOS | 0x20 { ETIQUETA_OCTETOS } else { etiqueta };
                return Some((Tlv { etiqueta, conteudo }, &resto[2..]));
            }
            let (filho, r) = ler_tlv(resto)?;
            if etiqueta == ETIQUETA_OCTETOS | 0x20 {
                filhos.push(filho.conteudo.into_owned());
            }
            resto = r;
        }
    }
    let n = if c < 0x80 {
        c as usize
    } else {
        let k = (c & 0x7f) as usize;
        if k == 0 || k > 4 || resto.len() < k {
            return None;
        }
        let n = resto[..k].iter().fold(0usize, |a, &x| (a << 8) | x as usize);
        resto = &resto[k..];
        n
    };
    if resto.len() < n {
        return None;
    }
    let (conteudo, resto) = resto.split_at(n);
    if etiqueta == ETIQUETA_OCTETOS | 0x20 {
        // `OCTET STRING` construída (BER): a concatenação dos pedaços.
        let mut juntos = Vec::new();
        let mut r = conteudo;
        while !r.is_empty() {
            let (f, rr) = ler_tlv(r)?;
            juntos.extend_from_slice(&f.conteudo);
            r = rr;
        }
        return Some((Tlv { etiqueta: ETIQUETA_OCTETOS, conteudo: std::borrow::Cow::Owned(juntos) }, resto));
    }
    Some((Tlv { etiqueta, conteudo: std::borrow::Cow::Borrowed(conteudo) }, resto))
}

/// Os filhos de um construído indefinido reescritos com comprimentos
/// definidos (para ler o conteúdo como uma sequência comum).
fn reescrever_definido(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut saida = Vec::new();
    while !b.is_empty() {
        let (t, r) = ler_tlv(b)?;
        saida.push(t.etiqueta);
        codificar_comprimento(&mut saida, t.conteudo.len());
        saida.extend_from_slice(&t.conteudo);
        b = r;
    }
    Some(saida)
}

fn codificar_comprimento(saida: &mut Vec<u8>, n: usize) {
    if n < 0x80 {
        saida.push(n as u8);
    } else {
        let bytes = (n as u32).to_be_bytes();
        let pula = bytes.iter().take_while(|&&x| x == 0).count();
        saida.push(0x80 | (4 - pula) as u8);
        saida.extend_from_slice(&bytes[pula..]);
    }
}

/// O TLV de etiqueta `etiqueta` no início de `b`.
fn esperar(b: &[u8], etiqueta: u8) -> Option<(Tlv<'_>, &[u8])> {
    let (t, r) = ler_tlv(b)?;
    (t.etiqueta == etiqueta).then_some((t, r))
}

/// Os elementos de uma sequência (ou conjunto).
fn elementos(b: &[u8]) -> Option<Vec<Tlv<'_>>> {
    let mut v = Vec::new();
    let mut r = b;
    while !r.is_empty() {
        let (t, rr) = ler_tlv(r)?;
        v.push(t);
        r = rr;
    }
    Some(v)
}

fn inteiro_pequeno(t: &Tlv) -> Option<u64> {
    if t.etiqueta != ETIQUETA_INTEIRO || t.conteudo.is_empty() || t.conteudo.len() > 8 || t.conteudo[0] & 0x80 != 0 {
        return None;
    }
    Some(t.conteudo.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

/// Os OIDs usados (codificação DER do conteúdo).
mod oid {
    pub const DADOS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x01];
    pub const DADOS_CIFRADOS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x06];
    pub const SACO_DE_CHAVE: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x01];
    pub const SACO_DE_CHAVE_CIFRADA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x02];
    pub const SACO_DE_CERTIFICADO: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x03];
    pub const SACO_DE_SACOS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x06];
    pub const CERTIFICADO_X509: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x16, 0x01];
    pub const ID_LOCAL_DA_CHAVE: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x15];
    // PBE do PKCS#12 (RFC 7292, apêndice C).
    pub const PBE_SHA1_RC4_128: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x01, 0x01];
    pub const PBE_SHA1_RC4_40: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x01, 0x02];
    pub const PBE_SHA1_RC2_128: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x01, 0x05];
    pub const PBE_SHA1_RC2_40: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x01, 0x06];
    pub const PBE_SHA1_3DES: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x01, 0x03];
    pub const PBE_SHA1_2DES: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x01, 0x04];
    // PBES2 (RFC 8018).
    pub const PBES2: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x05, 0x0d];
    pub const PBKDF2: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x05, 0x0c];
    pub const HMAC_SHA1: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x07];
    pub const HMAC_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x09];
    pub const HMAC_SHA384: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x0a];
    pub const HMAC_SHA512: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x0b];
    pub const AES128_CBC: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x01, 0x02];
    pub const AES192_CBC: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x01, 0x16];
    pub const AES256_CBC: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x01, 0x2a];
    pub const DES_EDE3_CBC: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x03, 0x07];
    pub const DES_CBC: &[u8] = &[0x2b, 0x0e, 0x03, 0x02, 0x07];
    // Resumos da MAC do PKCS#12.
    pub const SHA1: &[u8] = &[0x2b, 0x0e, 0x03, 0x02, 0x1a];
    pub const SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
    pub const SHA384: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02];
    pub const SHA512: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03];
}

/// Um `AlgorithmIdentifier`: o OID e os parâmetros (o resto da sequência).
fn algoritmo<'a>(t: &'a Tlv) -> Option<(&'a [u8], &'a [u8])> {
    if t.etiqueta != ETIQUETA_SEQUENCIA {
        return None;
    }
    let (o, params) = esperar(&t.conteudo, ETIQUETA_OID)?;
    let o = match o.conteudo {
        std::borrow::Cow::Borrowed(b) => b,
        std::borrow::Cow::Owned(_) => return None,
    };
    Some((o, params))
}

// ---------------------------------------------------------------------------
// Derivação de chaves e cifras.

/// Um resumo do `ring` com o tamanho do bloco (o `v` do PKCS#12).
fn resumo_do_oid(o: &[u8]) -> Option<(&'static ring::digest::Algorithm, usize)> {
    Some(match o {
        oid::SHA1 => (&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, 64),
        oid::SHA256 => (&ring::digest::SHA256, 64),
        oid::SHA384 => (&ring::digest::SHA384, 128),
        oid::SHA512 => (&ring::digest::SHA512, 128),
        _ => return None,
    })
}

/// A senha como `BMPString` terminada em dois zeros (a do PKCS#12; a senha
/// vazia é só o terminador).
fn senha_bmp(senha: &str) -> Vec<u8> {
    let mut v: Vec<u8> = senha.encode_utf16().flat_map(u16::to_be_bytes).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

/// A derivação do PKCS#12 (RFC 7292, apêndice B.2): `n` bytes para o
/// propósito `id` (1 chave, 2 IV, 3 MAC).
fn kdf_pkcs12(resumo: &'static ring::digest::Algorithm, v: usize, id: u8, senha: &[u8], sal: &[u8], iteracoes: u64, n: usize) -> Vec<u8> {
    let u = resumo.output_len();
    let repetir = |x: &[u8]| -> Vec<u8> {
        if x.is_empty() {
            return Vec::new();
        }
        let tam = v * x.len().div_ceil(v);
        x.iter().cycle().take(tam).copied().collect()
    };
    let d = vec![id; v];
    let mut i = repetir(sal);
    i.extend(repetir(senha));
    let mut saida = Vec::with_capacity(n);
    while saida.len() < n {
        let mut ctx = ring::digest::Context::new(resumo);
        ctx.update(&d);
        ctx.update(&i);
        let mut a = ctx.finish().as_ref().to_vec();
        for _ in 1..iteracoes.max(1) {
            a = ring::digest::digest(resumo, &a).as_ref().to_vec();
        }
        saida.extend_from_slice(&a[..u.min(n - saida.len())]);
        if saida.len() >= n {
            break;
        }
        // I_j = (I_j + B + 1) mod 2^(8v), B = A repetido até v bytes.
        let b: Vec<u8> = a.iter().cycle().take(v).copied().collect();
        for bloco in i.chunks_mut(v) {
            let mut vai = 1u16;
            for k in (0..v).rev() {
                let s = u16::from(bloco[k]) + u16::from(b[k]) + vai;
                bloco[k] = s as u8;
                vai = s >> 8;
            }
        }
    }
    saida
}

/// Decifra em CBC com o preenchimento PKCS#7 (`None`: preenchimento
/// inválido, o sinal de senha errada).
fn decifrar_cbc<C>(cifra: C, iv: &[u8], dados: &[u8]) -> Option<Vec<u8>>
where
    C: cbc::cipher::BlockDecryptMut + cbc::cipher::BlockCipher,
{
    use cbc::cipher::{BlockDecryptMut, InnerIvInit, block_padding::Pkcs7};
    let d = cbc::Decryptor::<C>::inner_iv_slice_init(cifra, iv).ok()?;
    let mut buf = dados.to_vec();
    let n = d.decrypt_padded_mut::<Pkcs7>(&mut buf).ok()?.len();
    buf.truncate(n);
    Some(buf)
}

/// Decifra com a cifra de `oid_cifra` (AES, 3DES, DES em CBC).
fn decifrar_com(oid_cifra: &[u8], chave: &[u8], iv: &[u8], dados: &[u8]) -> Option<Vec<u8>> {
    use cbc::cipher::KeyInit;
    match oid_cifra {
        oid::AES128_CBC => decifrar_cbc(aes::Aes128::new_from_slice(chave).ok()?, iv, dados),
        oid::AES192_CBC => decifrar_cbc(aes::Aes192::new_from_slice(chave).ok()?, iv, dados),
        oid::AES256_CBC => decifrar_cbc(aes::Aes256::new_from_slice(chave).ok()?, iv, dados),
        oid::DES_EDE3_CBC => decifrar_cbc(des::TdesEde3::new_from_slice(chave).ok()?, iv, dados),
        oid::DES_CBC => decifrar_cbc(des::Des::new_from_slice(chave).ok()?, iv, dados),
        _ => None,
    }
}

/// O tamanho da chave de uma cifra do PBES2.
fn tamanho_da_chave(oid_cifra: &[u8]) -> Option<usize> {
    Some(match oid_cifra {
        oid::AES128_CBC => 16,
        oid::AES192_CBC => 24,
        oid::AES256_CBC => 32,
        oid::DES_EDE3_CBC => 24,
        oid::DES_CBC => 8,
        _ => return None,
    })
}

/// Decifra `dados` pelo algoritmo `alg` (um `AlgorithmIdentifier` de PBES2
/// ou de PBE do PKCS#12) com a senha.
fn decifrar_pbe(alg: &Tlv, senha: &str, dados: &[u8]) -> Result<Vec<u8>, &'static str> {
    const DECODIFICACAO: &str = "DECODE_ERROR(p5_pbev2.c:0)";
    let (o, params) = algoritmo(alg).ok_or(DECODIFICACAO)?;
    if o == oid::PBES2 {
        let (seq, _) = esperar(params, ETIQUETA_SEQUENCIA).ok_or(DECODIFICACAO)?;
        let partes = elementos(&seq.conteudo).ok_or(DECODIFICACAO)?;
        let [kdf, esquema] = partes.as_slice() else { return Err(DECODIFICACAO) };
        let (o_kdf, p_kdf) = algoritmo(kdf).ok_or(DECODIFICACAO)?;
        if o_kdf != oid::PBKDF2 {
            return Err("UNSUPPORTED_KEY_DERIVATION_FUNCTION(p5_pbev2.c:0)");
        }
        let (o_cifra, p_cifra) = algoritmo(esquema).ok_or(DECODIFICACAO)?;
        let (iv, _) = esperar(p_cifra, ETIQUETA_OCTETOS).ok_or(DECODIFICACAO)?;
        let n = tamanho_da_chave(o_cifra).ok_or("UNSUPPORTED_CIPHER(p5_pbev2.c:0)")?;
        let (pk, _) = esperar(p_kdf, ETIQUETA_SEQUENCIA).ok_or(DECODIFICACAO)?;
        let pk = elementos(&pk.conteudo).ok_or(DECODIFICACAO)?;
        let sal = pk.first().filter(|t| t.etiqueta == ETIQUETA_OCTETOS).ok_or(DECODIFICACAO)?;
        let iteracoes = pk.get(1).and_then(inteiro_pequeno).ok_or(DECODIFICACAO)?;
        let mut prf = ring::pbkdf2::PBKDF2_HMAC_SHA1;
        for t in &pk[2..] {
            if t.etiqueta == ETIQUETA_INTEIRO {
                if inteiro_pequeno(t) != Some(n as u64) {
                    return Err("UNSUPPORTED_KEYLENGTH(p5_pbev2.c:0)");
                }
            } else if let Some((o_prf, _)) = algoritmo(t) {
                prf = match o_prf {
                    oid::HMAC_SHA1 => ring::pbkdf2::PBKDF2_HMAC_SHA1,
                    oid::HMAC_SHA256 => ring::pbkdf2::PBKDF2_HMAC_SHA256,
                    oid::HMAC_SHA384 => ring::pbkdf2::PBKDF2_HMAC_SHA384,
                    oid::HMAC_SHA512 => ring::pbkdf2::PBKDF2_HMAC_SHA512,
                    _ => return Err("UNSUPPORTED_PRF(p5_pbev2.c:0)"),
                };
            }
        }
        let iteracoes = std::num::NonZeroU32::new(u32::try_from(iteracoes).map_err(|_| DECODIFICACAO)?).ok_or(DECODIFICACAO)?;
        let mut chave = vec![0u8; n];
        ring::pbkdf2::derive(prf, iteracoes, &sal.conteudo, senha.as_bytes(), &mut chave);
        return decifrar_com(o_cifra, &chave, &iv.conteudo, dados).ok_or("BAD_DECRYPT(cipher.c:0)");
    }
    // As PBE do PKCS#12: SHA-1, a senha em BMP, `(sal, iterações)`.
    let (seq, _) = esperar(params, ETIQUETA_SEQUENCIA).ok_or(DECODIFICACAO)?;
    let partes = elementos(&seq.conteudo).ok_or(DECODIFICACAO)?;
    let [sal, iteracoes] = partes.as_slice() else { return Err(DECODIFICACAO) };
    let iteracoes = inteiro_pequeno(iteracoes).ok_or(DECODIFICACAO)?;
    let sha1 = &ring::digest::SHA1_FOR_LEGACY_USE_ONLY;
    let bmp = senha_bmp(senha);
    let k = |id: u8, n: usize| kdf_pkcs12(sha1, 64, id, &bmp, &sal.conteudo, iteracoes, n);
    use cbc::cipher::KeyInit;
    let r = match o {
        oid::PBE_SHA1_3DES => decifrar_cbc(des::TdesEde3::new_from_slice(&k(1, 24)).map_err(|_| DECODIFICACAO)?, &k(2, 8), dados),
        oid::PBE_SHA1_2DES => decifrar_cbc(des::TdesEde2::new_from_slice(&k(1, 16)).map_err(|_| DECODIFICACAO)?, &k(2, 8), dados),
        oid::PBE_SHA1_RC2_128 => decifrar_cbc(rc2::Rc2::new_with_eff_key_len(&k(1, 16), 128), &k(2, 8), dados),
        oid::PBE_SHA1_RC2_40 => decifrar_cbc(rc2::Rc2::new_with_eff_key_len(&k(1, 5), 40), &k(2, 8), dados),
        // RC4 (sem IV): a chave do `http_multi_server` nos testes dele;
        // o BoringSSL da VM a aceita.
        oid::PBE_SHA1_RC4_128 => Some(rc4(&k(1, 16), dados)),
        oid::PBE_SHA1_RC4_40 => Some(rc4(&k(1, 5), dados)),
        _ => return Err("UNKNOWN_ALGORITHM(algorithm.c:0)"),
    };
    r.ok_or("BAD_DECRYPT(cipher.c:0)")
}

/// RC4 (a cifra de fluxo; cifrar e decifrar são o mesmo).
fn rc4(chave: &[u8], dados: &[u8]) -> Vec<u8> {
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(chave[i % chave.len()]);
        s.swap(i, j as usize);
    }
    let (mut i, mut j) = (0u8, 0u8);
    dados
        .iter()
        .map(|&b| {
            i = i.wrapping_add(1);
            j = j.wrapping_add(s[i as usize]);
            s.swap(i as usize, j as usize);
            b ^ s[s[i as usize].wrapping_add(s[j as usize]) as usize]
        })
        .collect()
}

/// `EncryptedPrivateKeyInfo` → o `PrivateKeyInfo` (DER) decifrado.
fn decifrar_pkcs8(der: &[u8], senha: &str) -> Result<Vec<u8>, &'static str> {
    let (seq, _) = esperar(der, ETIQUETA_SEQUENCIA).ok_or("DECODE_ERROR(pkcs8.c:0)")?;
    let partes = elementos(&seq.conteudo).ok_or("DECODE_ERROR(pkcs8.c:0)")?;
    let [alg, dados] = partes.as_slice() else { return Err("DECODE_ERROR(pkcs8.c:0)") };
    if dados.etiqueta != ETIQUETA_OCTETOS {
        return Err("DECODE_ERROR(pkcs8.c:0)");
    }
    let chave = decifrar_pbe(alg, senha, &dados.conteudo)?;
    // O resultado tem de ser um `PrivateKeyInfo` (senão a senha estava
    // errada e o preenchimento acertou por acaso).
    esperar(&chave, ETIQUETA_SEQUENCIA).filter(|(_, r)| r.is_empty()).ok_or("BAD_DECRYPT(cipher.c:0)")?;
    Ok(chave)
}

// ---------------------------------------------------------------------------
// PEM.

/// Um bloco PEM: o rótulo, os cabeçalhos (`Proc-Type`, `DEK-Info`) e os
/// bytes.
struct BlocoPem {
    rotulo: String,
    cabecalhos: Vec<(String, String)>,
    dados: Result<Vec<u8>, ()>,
}

/// Os blocos PEM de `bytes`, em ordem.
fn blocos_pem(bytes: &[u8]) -> Vec<BlocoPem> {
    let texto = String::from_utf8_lossy(bytes);
    let mut blocos = Vec::new();
    let mut linhas = texto.lines();
    while let Some(l) = linhas.next() {
        let l = l.trim_end();
        let Some(rotulo) = l.strip_prefix("-----BEGIN ").and_then(|r| r.strip_suffix("-----")) else { continue };
        let fim = format!("-----END {rotulo}-----");
        let mut cabecalhos = Vec::new();
        let mut corpo = String::new();
        let mut em_cabecalhos = true;
        let mut fechado = false;
        for l in linhas.by_ref() {
            let l = l.trim_end();
            if l == fim {
                fechado = true;
                break;
            }
            if em_cabecalhos {
                if let Some((k, v)) = l.split_once(':') {
                    cabecalhos.push((k.trim().to_string(), v.trim().to_string()));
                    continue;
                }
                em_cabecalhos = false;
                if l.is_empty() {
                    continue;
                }
            }
            corpo.push_str(l.trim());
        }
        let dados = if fechado { base64_decodificar(&corpo).ok_or(()) } else { Err(()) };
        blocos.push(BlocoPem { rotulo: rotulo.to_string(), cabecalhos, dados });
    }
    blocos
}

fn base64_decodificar(s: &str) -> Option<Vec<u8>> {
    let valor = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let s = s.as_bytes();
    if s.len() % 4 != 0 {
        return None;
    }
    let mut saida = Vec::with_capacity(s.len() / 4 * 3);
    for (i, q) in s.chunks(4).enumerate() {
        let ultimo = i + 1 == s.len() / 4;
        let pad = q.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !ultimo) {
            return None;
        }
        let mut n = 0u32;
        for &c in &q[..4 - pad] {
            n = (n << 6) | valor(c)?;
        }
        n <<= 6 * pad as u32;
        let b = n.to_be_bytes();
        saida.extend_from_slice(&b[1..4 - pad]);
    }
    Some(saida)
}

/// A chave de um bloco PEM cifrado do jeito legado do OpenSSL
/// (`DEK-Info: CIFRA,IV`; chave pelo `EVP_BytesToKey` com MD5 e o IV como
/// sal).
fn decifrar_pem_legado(dek: &str, senha: &str, dados: &[u8]) -> Option<Vec<u8>> {
    let (nome, iv_hex) = dek.split_once(',')?;
    let iv: Vec<u8> = (0..iv_hex.len() / 2).map(|i| u8::from_str_radix(iv_hex.get(2 * i..2 * i + 2)?, 16).ok()).collect::<Option<_>>()?;
    let (o, n) = match nome.trim() {
        "AES-128-CBC" => (oid::AES128_CBC, 16),
        "AES-192-CBC" => (oid::AES192_CBC, 24),
        "AES-256-CBC" => (oid::AES256_CBC, 32),
        "DES-EDE3-CBC" => (oid::DES_EDE3_CBC, 24),
        "DES-CBC" => (oid::DES_CBC, 8),
        _ => return None,
    };
    use md5::Digest;
    let sal = iv.get(..8)?;
    let mut chave = Vec::new();
    let mut anterior: Vec<u8> = Vec::new();
    while chave.len() < n {
        let mut h = md5::Md5::new();
        h.update(&anterior);
        h.update(senha.as_bytes());
        h.update(sal);
        anterior = h.finalize().to_vec();
        chave.extend_from_slice(&anterior);
    }
    chave.truncate(n);
    decifrar_com(o, &chave, &iv, dados)
}

/// A primeira chave privada dos blocos PEM (`PEM_read_bio_PrivateKey`):
/// `Ok(None)` se nenhum bloco é de chave (a linha de início não existe e a
/// VM tenta o PKCS#12).
fn chave_pem(bytes: &[u8], senha: &str) -> Result<Option<rustls::pki_types::PrivateKeyDer<'static>>, ()> {
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs1KeyDer, PrivatePkcs8KeyDer, PrivateSec1KeyDer};
    for b in blocos_pem(bytes) {
        let tipo = match b.rotulo.as_str() {
            "PRIVATE KEY" | "ENCRYPTED PRIVATE KEY" | "RSA PRIVATE KEY" | "EC PRIVATE KEY" => b.rotulo.as_str(),
            _ => continue,
        };
        let dados = b.dados?;
        let dek = b.cabecalhos.iter().find(|(k, _)| k == "DEK-Info").map(|(_, v)| v.as_str());
        let cifrado = b.cabecalhos.iter().any(|(k, v)| k == "Proc-Type" && v.ends_with("ENCRYPTED"));
        let dados = match (cifrado, dek) {
            (true, Some(dek)) => decifrar_pem_legado(dek, senha, &dados).ok_or(())?,
            (true, None) => return Err(()),
            _ => dados,
        };
        return Ok(Some(match tipo {
            "PRIVATE KEY" => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(dados)),
            "ENCRYPTED PRIVATE KEY" => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(decifrar_pkcs8(&dados, senha).map_err(|_| ())?)),
            "RSA PRIVATE KEY" => PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(dados)),
            _ => PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(dados)),
        }));
    }
    Ok(None)
}

/// Os certificados dos blocos PEM (`PEM_read_bio_X509` em laço): `Ok(None)`
/// se não há nenhum bloco de certificado.
fn certificados_pem(bytes: &[u8]) -> Result<Option<Vec<rustls::pki_types::CertificateDer<'static>>>, String> {
    let mut certs = Vec::new();
    for b in blocos_pem(bytes) {
        if !matches!(b.rotulo.as_str(), "CERTIFICATE" | "X509 CERTIFICATE" | "TRUSTED CERTIFICATE") {
            continue;
        }
        let dados = b.dados.map_err(|()| "BAD_BASE64_DECODE(pem_lib.c:752)".to_string())?;
        certs.push(rustls::pki_types::CertificateDer::from(dados));
    }
    Ok((!certs.is_empty()).then_some(certs))
}

// ---------------------------------------------------------------------------
// PKCS#12.

/// O conteúdo de um PKCS#12: a chave (com o `localKeyId`) e os
/// certificados (com o `localKeyId` de cada), na ordem do arquivo.
#[derive(Default)]
struct Pkcs12 {
    chave: Option<(Vec<u8>, Option<Vec<u8>>)>,
    certificados: Vec<(Vec<u8>, Option<Vec<u8>>)>,
}

const PKCS12_MALFORMADO: &str = "BAD_PKCS12_DATA(pkcs8_x509.c:599)";
const PKCS12_INVALIDO: &str = "BAD_PKCS12_DATA(pkcs8_x509.c:611)";
const SENHA_INCORRETA: &str = "INCORRECT_PASSWORD(pkcs8_x509.c:710)";

/// Lê um PKCS#12 (`PKCS12_get_key_and_certs`), conferindo a MAC.
fn ler_pkcs12(der: &[u8], senha: &str) -> Result<Pkcs12, &'static str> {
    let (pfx, resto) = esperar(der, ETIQUETA_SEQUENCIA).ok_or(PKCS12_MALFORMADO)?;
    if !resto.is_empty() {
        return Err(PKCS12_MALFORMADO);
    }
    let (versao, r) = ler_tlv(&pfx.conteudo).ok_or(PKCS12_MALFORMADO)?;
    if inteiro_pequeno(&versao) != Some(3) {
        return Err(PKCS12_INVALIDO);
    }
    let (seguro, r) = esperar(r, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
    let (tipo, conteudo) = esperar(&seguro.conteudo, ETIQUETA_OID).ok_or(PKCS12_INVALIDO)?;
    if tipo.conteudo.as_ref() != oid::DADOS {
        return Err(PKCS12_INVALIDO);
    }
    let (explicito, _) = esperar(conteudo, 0xa0).ok_or(PKCS12_INVALIDO)?;
    let (octetos, _) = esperar(&explicito.conteudo, ETIQUETA_OCTETOS).ok_or(PKCS12_INVALIDO)?;
    let seguro = octetos.conteudo;
    // A MAC (sem ela o arquivo é aceito, como no BoringSSL).
    if let Some((mac, _)) = esperar(r, ETIQUETA_SEQUENCIA) {
        conferir_mac(&mac.conteudo, &seguro, senha)?;
    }
    let mut p = Pkcs12::default();
    let (lista, _) = esperar(&seguro, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
    for ci in elementos(&lista.conteudo).ok_or(PKCS12_INVALIDO)? {
        let (tipo, conteudo) = esperar(&ci.conteudo, ETIQUETA_OID).ok_or(PKCS12_INVALIDO)?;
        let (explicito, _) = esperar(conteudo, 0xa0).ok_or(PKCS12_INVALIDO)?;
        let sacos = match tipo.conteudo.as_ref() {
            oid::DADOS => esperar(&explicito.conteudo, ETIQUETA_OCTETOS).ok_or(PKCS12_INVALIDO)?.0.conteudo.into_owned(),
            oid::DADOS_CIFRADOS => {
                // EncryptedData { versão, EncryptedContentInfo { tipo, alg, [0] cifrado } }
                let (ed, _) = esperar(&explicito.conteudo, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
                let (_, r) = ler_tlv(&ed.conteudo).ok_or(PKCS12_INVALIDO)?;
                let (eci, _) = esperar(r, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
                let (_, r) = esperar(&eci.conteudo, ETIQUETA_OID).ok_or(PKCS12_INVALIDO)?;
                let (alg, r) = esperar(r, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
                let (cifrado, _) = ler_tlv(r).ok_or(PKCS12_INVALIDO)?;
                if cifrado.etiqueta & 0xdf != 0x80 {
                    return Err(PKCS12_INVALIDO);
                }
                let cifrado = if cifrado.etiqueta == 0xa0 {
                    // `[0]` construído: os pedaços `OCTET STRING`.
                    elementos(&cifrado.conteudo).ok_or(PKCS12_INVALIDO)?.iter().flat_map(|t| t.conteudo.iter().copied()).collect()
                } else {
                    cifrado.conteudo.into_owned()
                };
                decifrar_pbe(&alg, senha, &cifrado)?
            }
            _ => continue,
        };
        ler_sacos(&sacos, senha, &mut p, 0)?;
    }
    Ok(p)
}

/// Confere a `MacData` (a senha em BMP; a vazia também sem terminador, como
/// o BoringSSL tenta).
fn conferir_mac(mac: &[u8], dados: &[u8], senha: &str) -> Result<(), &'static str> {
    let partes = elementos(mac).ok_or(PKCS12_INVALIDO)?;
    let info = partes.first().filter(|t| t.etiqueta == ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
    let (alg, r) = esperar(&info.conteudo, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
    let (resumo, _) = esperar(r, ETIQUETA_OCTETOS).ok_or(PKCS12_INVALIDO)?;
    let (o, _) = esperar(&alg.conteudo, ETIQUETA_OID).ok_or(PKCS12_INVALIDO)?;
    let (h, v) = resumo_do_oid(&o.conteudo).ok_or("UNSUPPORTED_DIGEST(pkcs8_x509.c:0)")?;
    let sal = partes.get(1).filter(|t| t.etiqueta == ETIQUETA_OCTETOS).ok_or(PKCS12_INVALIDO)?;
    let iteracoes = match partes.get(2) {
        Some(t) => inteiro_pequeno(t).ok_or(PKCS12_INVALIDO)?,
        None => 1,
    };
    let hmac = match h.output_len() {
        20 => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
        32 => ring::hmac::HMAC_SHA256,
        48 => ring::hmac::HMAC_SHA384,
        _ => ring::hmac::HMAC_SHA512,
    };
    let mut candidatas = vec![senha_bmp(senha)];
    if senha.is_empty() {
        candidatas.push(Vec::new());
    }
    for s in candidatas {
        let chave = kdf_pkcs12(h, v, 3, &s, &sal.conteudo, iteracoes, h.output_len());
        let k = ring::hmac::Key::new(hmac, &chave);
        if ring::hmac::verify(&k, dados, &resumo.conteudo).is_ok() {
            return Ok(());
        }
    }
    Err(SENHA_INCORRETA)
}

/// As `SafeContents` (`SEQUENCE OF SafeBag`).
fn ler_sacos(der: &[u8], senha: &str, p: &mut Pkcs12, profundidade: usize) -> Result<(), &'static str> {
    if profundidade > 3 {
        return Err(PKCS12_INVALIDO);
    }
    let (seq, _) = esperar(der, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
    for saco in elementos(&seq.conteudo).ok_or(PKCS12_INVALIDO)? {
        let (tipo, r) = esperar(&saco.conteudo, ETIQUETA_OID).ok_or(PKCS12_INVALIDO)?;
        let (valor, r) = esperar(r, 0xa0).ok_or(PKCS12_INVALIDO)?;
        let id_local = esperar(r, ETIQUETA_CONJUNTO).and_then(|(atributos, _)| id_local_da_chave(&atributos.conteudo));
        match tipo.conteudo.as_ref() {
            oid::SACO_DE_CHAVE => {
                if p.chave.is_none() {
                    let (k, _) = esperar(&valor.conteudo, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
                    let _ = k;
                    p.chave = Some((valor.conteudo.to_vec(), id_local));
                }
            }
            oid::SACO_DE_CHAVE_CIFRADA => {
                if p.chave.is_none() {
                    p.chave = Some((decifrar_pkcs8(&valor.conteudo, senha)?, id_local));
                }
            }
            oid::SACO_DE_CERTIFICADO => {
                let (cb, _) = esperar(&valor.conteudo, ETIQUETA_SEQUENCIA).ok_or(PKCS12_INVALIDO)?;
                let (t, r) = esperar(&cb.conteudo, ETIQUETA_OID).ok_or(PKCS12_INVALIDO)?;
                if t.conteudo.as_ref() != oid::CERTIFICADO_X509 {
                    continue;
                }
                let (e, _) = esperar(r, 0xa0).ok_or(PKCS12_INVALIDO)?;
                let (cert, _) = esperar(&e.conteudo, ETIQUETA_OCTETOS).ok_or(PKCS12_INVALIDO)?;
                p.certificados.push((cert.conteudo.into_owned(), id_local));
            }
            oid::SACO_DE_SACOS => ler_sacos(&valor.conteudo, senha, p, profundidade + 1)?,
            _ => {}
        }
    }
    Ok(())
}

/// O `localKeyId` dos atributos de um saco.
fn id_local_da_chave(atributos: &[u8]) -> Option<Vec<u8>> {
    for a in elementos(atributos)? {
        let (o, r) = esperar(&a.conteudo, ETIQUETA_OID)?;
        if o.conteudo.as_ref() == oid::ID_LOCAL_DA_CHAVE {
            let (valores, _) = esperar(r, ETIQUETA_CONJUNTO)?;
            let (v, _) = esperar(&valores.conteudo, ETIQUETA_OCTETOS)?;
            return Some(v.conteudo.into_owned());
        }
    }
    None
}

impl Pkcs12 {
    /// `PKCS12_parse`: o certificado da chave (o de mesmo `localKeyId`, ou o
    /// primeiro) e os demais, na ordem do arquivo.
    fn certificado_e_demais(self) -> Vec<rustls::pki_types::CertificateDer<'static>> {
        let id = self.chave.as_ref().and_then(|(_, id)| id.clone());
        let mut certs = self.certificados;
        let i = id.and_then(|id| certs.iter().position(|(_, c)| c.as_deref() == Some(id.as_slice()))).unwrap_or(0);
        if i < certs.len() {
            let c = certs.remove(i);
            certs.insert(0, c);
        }
        certs.into_iter().map(|(c, _)| rustls::pki_types::CertificateDer::from(c)).collect()
    }
}

// ---------------------------------------------------------------------------
// O que o fragmento `tls` chama.

/// A chave privada de `bytes` (`GetPrivateKey` da VM): o PEM e, sem linha
/// de início, o PKCS#12. `Err` quando não há chave (a VM lança a
/// `ArgumentError` "Expected private key, but none was found").
fn chave_de(bytes: &[u8], senha: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>, ()> {
    match chave_pem(bytes, senha)? {
        Some(k) => Ok(k),
        None => {
            let p = ler_pkcs12(bytes, senha).map_err(|_| ())?;
            let (k, _) = p.chave.ok_or(())?;
            Ok(rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(k)))
        }
    }
}

/// Os certificados de `bytes` (`SetTrustedCertificatesBytes`,
/// `UseChainBytes`, `SetClientAuthoritiesBytes` da VM): os do PEM e, sem
/// linha de início, os do PKCS#12 (o da chave primeiro). O erro é o texto
/// do BoringSSL.
fn certificados_de(bytes: &[u8], senha: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, String> {
    match certificados_pem(bytes)? {
        Some(c) => Ok(c),
        None => {
            let p = ler_pkcs12(bytes, senha).map_err(str::to_string)?;
            Ok(p.certificado_e_demais())
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn kdf_pkcs12_do_rfc() {
        // Vetor do OpenSSL (`test/pkcs12_format_test` / RFC 7292): senha
        // "smeg", sal 0A58CF64530D823F, 1 iteração, ID 1, 24 bytes.
        let sal = [0x0a, 0x58, 0xcf, 0x64, 0x53, 0x0d, 0x82, 0x3f];
        let k = kdf_pkcs12(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, 64, 1, &senha_bmp("smeg"), &sal, 1, 24);
        let esperado = [
            0x8a, 0xaa, 0xe6, 0x29, 0x7b, 0x6c, 0xb0, 0x46, 0x42, 0xab, 0x5b, 0x07, 0x78, 0x51, 0x28, 0x4e, 0xb7, 0x12, 0x8f, 0x1a, 0x2a, 0x7f, 0xbc,
            0xa3,
        ];
        assert_eq!(k, esperado);
    }

    #[test]
    fn rc4_vetor_conhecido() {
        // O vetor clássico: chave "Key", texto "Plaintext".
        assert_eq!(rc4(b"Key", b"Plaintext"), [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3]);
    }

    #[test]
    fn base64_com_e_sem_preenchimento() {
        assert_eq!(base64_decodificar("TWFu").unwrap(), b"Man");
        assert_eq!(base64_decodificar("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decodificar("TQ==").unwrap(), b"M");
        assert!(base64_decodificar("T@==").is_none());
    }

    #[test]
    fn ber_indefinido_e_octetos_segmentados() {
        // SEQUENCE indefinida com um INTEGER 5; OCTET STRING construída.
        let (t, r) = ler_tlv(&[0x30, 0x80, 0x02, 0x01, 0x05, 0x00, 0x00, 0xff]).unwrap();
        assert_eq!((t.etiqueta, t.conteudo.as_ref(), r), (0x30, &[0x02, 0x01, 0x05][..], &[0xff][..]));
        let (t, _) = ler_tlv(&[0x24, 0x80, 0x04, 0x01, 0xaa, 0x04, 0x01, 0xbb, 0x00, 0x00]).unwrap();
        assert_eq!((t.etiqueta, t.conteudo.as_ref()), (0x04, &[0xaa, 0xbb][..]));
    }
}

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
    tag: u8,
    content: std::borrow::Cow<'a, [u8]>,
}

const TAG_INTEGER: u8 = 0x02;
const TAG_OCTETS: u8 = 0x04;
const TAG_OID: u8 = 0x06;
const TAG_SEQUENCE: u8 = 0x30;
const TAG_SET: u8 = 0x31;

/// Lê um TLV de `b`: o valor e o resto. Aceita o comprimento indefinido do
/// BER (até o `00 00`), e junta os pedaços de uma `OCTET STRING`
/// construída.
fn read_tlv(b: &[u8]) -> Option<(Tlv<'_>, &[u8])> {
    let (&tag, rest) = b.split_first()?;
    if tag & 0x1f == 0x1f {
        return None; // etiquetas de vários bytes não aparecem nestes formatos
    }
    let (&c, mut rest) = rest.split_first()?;
    if c == 0x80 {
        // Comprimento indefinido: os filhos até o fim de conteúdo.
        if tag & 0x20 == 0 {
            return None;
        }
        let start = rest;
        let mut children = Vec::new();
        loop {
            if rest.starts_with(&[0, 0]) {
                let used = start.len() - rest.len();
                let content = if tag == TAG_OCTETS | 0x20 {
                    std::borrow::Cow::Owned(children.concat())
                } else {
                    std::borrow::Cow::Owned(rewrite_defined(&start[..used])?)
                };
                let tag = if tag == TAG_OCTETS | 0x20 { TAG_OCTETS } else { tag };
                return Some((Tlv { tag, content }, &rest[2..]));
            }
            let (child, r) = read_tlv(rest)?;
            if tag == TAG_OCTETS | 0x20 {
                children.push(child.content.into_owned());
            }
            rest = r;
        }
    }
    let n = if c < 0x80 {
        c as usize
    } else {
        let k = (c & 0x7f) as usize;
        if k == 0 || k > 4 || rest.len() < k {
            return None;
        }
        let n = rest[..k].iter().fold(0usize, |a, &x| (a << 8) | x as usize);
        rest = &rest[k..];
        n
    };
    if rest.len() < n {
        return None;
    }
    let (content, rest) = rest.split_at(n);
    if tag == TAG_OCTETS | 0x20 {
        // `OCTET STRING` construída (BER): a concatenação dos pedaços.
        let mut together = Vec::new();
        let mut r = content;
        while !r.is_empty() {
            let (f, rr) = read_tlv(r)?;
            together.extend_from_slice(&f.content);
            r = rr;
        }
        return Some((Tlv { tag: TAG_OCTETS, content: std::borrow::Cow::Owned(together) }, rest));
    }
    Some((Tlv { tag, content: std::borrow::Cow::Borrowed(content) }, rest))
}

/// Os filhos de um construído indefinido reescritos com comprimentos
/// definidos (para ler o conteúdo como uma sequência comum).
fn rewrite_defined(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let (t, r) = read_tlv(b)?;
        out.push(t.tag);
        encode_length(&mut out, t.content.len());
        out.extend_from_slice(&t.content);
        b = r;
    }
    Some(out)
}

fn encode_length(out: &mut Vec<u8>, n: usize) {
    if n < 0x80 {
        out.push(n as u8);
    } else {
        let bytes = (n as u32).to_be_bytes();
        let skips = bytes.iter().take_while(|&&x| x == 0).count();
        out.push(0x80 | (4 - skips) as u8);
        out.extend_from_slice(&bytes[skips..]);
    }
}

/// O TLV de etiqueta `tag` no início de `b`.
fn wait_for(b: &[u8], tag: u8) -> Option<(Tlv<'_>, &[u8])> {
    let (t, r) = read_tlv(b)?;
    (t.tag == tag).then_some((t, r))
}

/// Os elementos de uma sequência (ou conjunto).
fn elements(b: &[u8]) -> Option<Vec<Tlv<'_>>> {
    let mut v = Vec::new();
    let mut r = b;
    while !r.is_empty() {
        let (t, rr) = read_tlv(r)?;
        v.push(t);
        r = rr;
    }
    Some(v)
}

fn small_int(t: &Tlv) -> Option<u64> {
    if t.tag != TAG_INTEGER || t.content.is_empty() || t.content.len() > 8 || t.content[0] & 0x80 != 0 {
        return None;
    }
    Some(t.content.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

/// Os OIDs usados (codificação DER do conteúdo).
mod oid {
    pub const DADOS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x01];
    pub const ENCRYPTED_DATA: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x06];
    pub const KEY_BAG: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x01];
    pub const SHROUDED_KEY_BAG: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x02];
    pub const CERT_BAG: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x03];
    pub const SACO_DE_SACOS: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x0c, 0x0a, 0x01, 0x06];
    pub const X509_CERTIFICATE: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x16, 0x01];
    pub const LOCAL_KEY_ID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x15];
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
fn algorithm<'a>(t: &'a Tlv) -> Option<(&'a [u8], &'a [u8])> {
    if t.tag != TAG_SEQUENCE {
        return None;
    }
    let (o, params) = wait_for(&t.content, TAG_OID)?;
    let o = match o.content {
        std::borrow::Cow::Borrowed(b) => b,
        std::borrow::Cow::Owned(_) => return None,
    };
    Some((o, params))
}

// ---------------------------------------------------------------------------
// Derivação de chaves e cifras.

/// Um resumo do `ring` com o tamanho do bloco (o `v` do PKCS#12).
fn oid_digest(o: &[u8]) -> Option<(&'static ring::digest::Algorithm, usize)> {
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
fn bmp_password(password: &str) -> Vec<u8> {
    let mut v: Vec<u8> = password.encode_utf16().flat_map(u16::to_be_bytes).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

/// A derivação do PKCS#12 (RFC 7292, apêndice B.2): `n` bytes para o
/// propósito `id` (1 chave, 2 IV, 3 MAC).
fn kdf_pkcs12(digest_alg: &'static ring::digest::Algorithm, v: usize, id: u8, password: &[u8], salt: &[u8], iterations: u64, n: usize) -> Vec<u8> {
    let u = digest_alg.output_len();
    let repeat = |x: &[u8]| -> Vec<u8> {
        if x.is_empty() {
            return Vec::new();
        }
        let sz = v * x.len().div_ceil(v);
        x.iter().cycle().take(sz).copied().collect()
    };
    let d = vec![id; v];
    let mut i = repeat(salt);
    i.extend(repeat(password));
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let mut ctx = ring::digest::Context::new(digest_alg);
        ctx.update(&d);
        ctx.update(&i);
        let mut a = ctx.finish().as_ref().to_vec();
        for _ in 1..iterations.max(1) {
            a = ring::digest::digest(digest_alg, &a).as_ref().to_vec();
        }
        out.extend_from_slice(&a[..u.min(n - out.len())]);
        if out.len() >= n {
            break;
        }
        // I_j = (I_j + B + 1) mod 2^(8v), B = A repetido até v bytes.
        let b: Vec<u8> = a.iter().cycle().take(v).copied().collect();
        for block in i.chunks_mut(v) {
            let mut goes = 1u16;
            for k in (0..v).rev() {
                let s = u16::from(block[k]) + u16::from(b[k]) + goes;
                block[k] = s as u8;
                goes = s >> 8;
            }
        }
    }
    out
}

/// Decifra em CBC com o preenchimento PKCS#7 (`None`: preenchimento
/// inválido, o sinal de senha errada).
fn decrypt_cbc<C>(cipher_kind: C, iv: &[u8], data: &[u8]) -> Option<Vec<u8>>
where
    C: cbc::cipher::BlockDecryptMut + cbc::cipher::BlockCipher,
{
    use cbc::cipher::{BlockDecryptMut, InnerIvInit, block_padding::Pkcs7};
    let d = cbc::Decryptor::<C>::inner_iv_slice_init(cipher_kind, iv).ok()?;
    let mut buf = data.to_vec();
    let n = d.decrypt_padded_mut::<Pkcs7>(&mut buf).ok()?.len();
    buf.truncate(n);
    Some(buf)
}

/// Decifra com a cifra de `cipher_oid_of` (AES, 3DES, DES em CBC).
fn decrypt_with(cipher_oid_of: &[u8], key: &[u8], iv: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    use cbc::cipher::KeyInit;
    match cipher_oid_of {
        oid::AES128_CBC => decrypt_cbc(aes::Aes128::new_from_slice(key).ok()?, iv, data),
        oid::AES192_CBC => decrypt_cbc(aes::Aes192::new_from_slice(key).ok()?, iv, data),
        oid::AES256_CBC => decrypt_cbc(aes::Aes256::new_from_slice(key).ok()?, iv, data),
        oid::DES_EDE3_CBC => decrypt_cbc(des::TdesEde3::new_from_slice(key).ok()?, iv, data),
        oid::DES_CBC => decrypt_cbc(des::Des::new_from_slice(key).ok()?, iv, data),
        _ => None,
    }
}

/// O tamanho da chave de uma cifra do PBES2.
fn key_size(cipher_oid_of: &[u8]) -> Option<usize> {
    Some(match cipher_oid_of {
        oid::AES128_CBC => 16,
        oid::AES192_CBC => 24,
        oid::AES256_CBC => 32,
        oid::DES_EDE3_CBC => 24,
        oid::DES_CBC => 8,
        _ => return None,
    })
}

/// Decifra `data` pelo algoritmo `alg` (um `AlgorithmIdentifier` de PBES2
/// ou de PBE do PKCS#12) com a senha.
fn decrypt_pbe(alg: &Tlv, password: &str, data: &[u8]) -> Result<Vec<u8>, &'static str> {
    const DECODING: &str = "DECODE_ERROR(p5_pbev2.c:0)";
    let (o, params) = algorithm(alg).ok_or(DECODING)?;
    if o == oid::PBES2 {
        let (seq, _) = wait_for(params, TAG_SEQUENCE).ok_or(DECODING)?;
        let part_list = elements(&seq.content).ok_or(DECODING)?;
        let [kdf, scheme] = part_list.as_slice() else { return Err(DECODING) };
        let (kdf_oid, kdf_params) = algorithm(kdf).ok_or(DECODING)?;
        if kdf_oid != oid::PBKDF2 {
            return Err("UNSUPPORTED_KEY_DERIVATION_FUNCTION(p5_pbev2.c:0)");
        }
        let (cipher_oid, cipher_params) = algorithm(scheme).ok_or(DECODING)?;
        let (iv, _) = wait_for(cipher_params, TAG_OCTETS).ok_or(DECODING)?;
        let n = key_size(cipher_oid).ok_or("UNSUPPORTED_CIPHER(p5_pbev2.c:0)")?;
        let (pk, _) = wait_for(kdf_params, TAG_SEQUENCE).ok_or(DECODING)?;
        let pk = elements(&pk.content).ok_or(DECODING)?;
        let salt = pk.first().filter(|t| t.tag == TAG_OCTETS).ok_or(DECODING)?;
        let iterations = pk.get(1).and_then(small_int).ok_or(DECODING)?;
        let mut prf = ring::pbkdf2::PBKDF2_HMAC_SHA1;
        for t in &pk[2..] {
            if t.tag == TAG_INTEGER {
                if small_int(t) != Some(n as u64) {
                    return Err("UNSUPPORTED_KEYLENGTH(p5_pbev2.c:0)");
                }
            } else if let Some((prf_oid, _)) = algorithm(t) {
                prf = match prf_oid {
                    oid::HMAC_SHA1 => ring::pbkdf2::PBKDF2_HMAC_SHA1,
                    oid::HMAC_SHA256 => ring::pbkdf2::PBKDF2_HMAC_SHA256,
                    oid::HMAC_SHA384 => ring::pbkdf2::PBKDF2_HMAC_SHA384,
                    oid::HMAC_SHA512 => ring::pbkdf2::PBKDF2_HMAC_SHA512,
                    _ => return Err("UNSUPPORTED_PRF(p5_pbev2.c:0)"),
                };
            }
        }
        let iterations = std::num::NonZeroU32::new(u32::try_from(iterations).map_err(|_| DECODING)?).ok_or(DECODING)?;
        let mut key = vec![0u8; n];
        ring::pbkdf2::derive(prf, iterations, &salt.content, password.as_bytes(), &mut key);
        return decrypt_with(cipher_oid, &key, &iv.content, data).ok_or("BAD_DECRYPT(cipher.c:0)");
    }
    // As PBE do PKCS#12: SHA-1, a senha em BMP, `(salt, iterações)`.
    let (seq, _) = wait_for(params, TAG_SEQUENCE).ok_or(DECODING)?;
    let part_list = elements(&seq.content).ok_or(DECODING)?;
    let [salt, iterations] = part_list.as_slice() else { return Err(DECODING) };
    let iterations = small_int(iterations).ok_or(DECODING)?;
    let sha1 = &ring::digest::SHA1_FOR_LEGACY_USE_ONLY;
    let bmp = bmp_password(password);
    let k = |id: u8, n: usize| kdf_pkcs12(sha1, 64, id, &bmp, &salt.content, iterations, n);
    use cbc::cipher::KeyInit;
    let r = match o {
        oid::PBE_SHA1_3DES => decrypt_cbc(des::TdesEde3::new_from_slice(&k(1, 24)).map_err(|_| DECODING)?, &k(2, 8), data),
        oid::PBE_SHA1_2DES => decrypt_cbc(des::TdesEde2::new_from_slice(&k(1, 16)).map_err(|_| DECODING)?, &k(2, 8), data),
        oid::PBE_SHA1_RC2_128 => decrypt_cbc(rc2::Rc2::new_with_eff_key_len(&k(1, 16), 128), &k(2, 8), data),
        oid::PBE_SHA1_RC2_40 => decrypt_cbc(rc2::Rc2::new_with_eff_key_len(&k(1, 5), 40), &k(2, 8), data),
        // RC4 (sem IV): a chave do `http_multi_server` nos testes dele;
        // o BoringSSL da VM a aceita.
        oid::PBE_SHA1_RC4_128 => Some(rc4(&k(1, 16), data)),
        oid::PBE_SHA1_RC4_40 => Some(rc4(&k(1, 5), data)),
        _ => return Err("UNKNOWN_ALGORITHM(algorithm.c:0)"),
    };
    r.ok_or("BAD_DECRYPT(cipher.c:0)")
}

/// RC4 (a cifra de fluxo; cifrar e decifrar são o mesmo).
fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut s: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
        s.swap(i, j as usize);
    }
    let (mut i, mut j) = (0u8, 0u8);
    data
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
fn decrypt_pkcs8(der: &[u8], password: &str) -> Result<Vec<u8>, &'static str> {
    let (seq, _) = wait_for(der, TAG_SEQUENCE).ok_or("DECODE_ERROR(pkcs8.c:0)")?;
    let part_list = elements(&seq.content).ok_or("DECODE_ERROR(pkcs8.c:0)")?;
    let [alg, data] = part_list.as_slice() else { return Err("DECODE_ERROR(pkcs8.c:0)") };
    if data.tag != TAG_OCTETS {
        return Err("DECODE_ERROR(pkcs8.c:0)");
    }
    let key = decrypt_pbe(alg, password, &data.content)?;
    // O resultado tem de ser um `PrivateKeyInfo` (senão a senha estava
    // errada e o preenchimento acertou por acaso).
    wait_for(&key, TAG_SEQUENCE).filter(|(_, r)| r.is_empty()).ok_or("BAD_DECRYPT(cipher.c:0)")?;
    Ok(key)
}

// ---------------------------------------------------------------------------
// PEM.

/// Um bloco PEM: o rótulo, os cabeçalhos (`Proc-Type`, `DEK-Info`) e os
/// bytes.
struct PemBlock {
    label_name: String,
    headers: Vec<(String, String)>,
    data: Result<Vec<u8>, ()>,
}

/// Os blocos PEM de `bytes`, em ordem.
fn pem_blocks(bytes: &[u8]) -> Vec<PemBlock> {
    let text = String::from_utf8_lossy(bytes);
    let mut blocks = Vec::new();
    let mut line_list = text.lines();
    while let Some(l) = line_list.next() {
        let l = l.trim_end();
        let Some(label_name) = l.strip_prefix("-----BEGIN ").and_then(|r| r.strip_suffix("-----")) else { continue };
        let end = format!("-----END {label_name}-----");
        let mut headers = Vec::new();
        let mut body = String::new();
        let mut in_headers = true;
        let mut closed_flag = false;
        for l in line_list.by_ref() {
            let l = l.trim_end();
            if l == end {
                closed_flag = true;
                break;
            }
            if in_headers {
                if let Some((k, v)) = l.split_once(':') {
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                    continue;
                }
                in_headers = false;
                if l.is_empty() {
                    continue;
                }
            }
            body.push_str(l.trim());
        }
        let data = if closed_flag { base64_decode(&body).ok_or(()) } else { Err(()) };
        blocks.push(PemBlock { label_name: label_name.to_string(), headers, data });
    }
    blocks
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
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
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for (i, q) in s.chunks(4).enumerate() {
        let last_one = i + 1 == s.len() / 4;
        let pad = q.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last_one) {
            return None;
        }
        let mut n = 0u32;
        for &c in &q[..4 - pad] {
            n = (n << 6) | value(c)?;
        }
        n <<= 6 * pad as u32;
        let b = n.to_be_bytes();
        out.extend_from_slice(&b[1..4 - pad]);
    }
    Some(out)
}

/// A chave de um bloco PEM cifrado do jeito legado do OpenSSL
/// (`DEK-Info: CIFRA,IV`; chave pelo `EVP_BytesToKey` com MD5 e o IV como
/// sal).
fn decrypt_legacy_pem(dek: &str, password: &str, data: &[u8]) -> Option<Vec<u8>> {
    let (name, iv_hex) = dek.split_once(',')?;
    let iv: Vec<u8> = (0..iv_hex.len() / 2).map(|i| u8::from_str_radix(iv_hex.get(2 * i..2 * i + 2)?, 16).ok()).collect::<Option<_>>()?;
    let (o, n) = match name.trim() {
        "AES-128-CBC" => (oid::AES128_CBC, 16),
        "AES-192-CBC" => (oid::AES192_CBC, 24),
        "AES-256-CBC" => (oid::AES256_CBC, 32),
        "DES-EDE3-CBC" => (oid::DES_EDE3_CBC, 24),
        "DES-CBC" => (oid::DES_CBC, 8),
        _ => return None,
    };
    use md5::Digest;
    let salt = iv.get(..8)?;
    let mut key = Vec::new();
    let mut previous: Vec<u8> = Vec::new();
    while key.len() < n {
        let mut h = md5::Md5::new();
        h.update(&previous);
        h.update(password.as_bytes());
        h.update(salt);
        previous = h.finalize().to_vec();
        key.extend_from_slice(&previous);
    }
    key.truncate(n);
    decrypt_with(o, &key, &iv, data)
}

/// A primeira chave privada dos blocos PEM (`PEM_read_bio_PrivateKey`):
/// `Ok(None)` se nenhum bloco é de chave (a linha de início não existe e a
/// VM tenta o PKCS#12).
fn pem_key(bytes: &[u8], password: &str) -> Result<Option<rustls::pki_types::PrivateKeyDer<'static>>, ()> {
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs1KeyDer, PrivatePkcs8KeyDer, PrivateSec1KeyDer};
    for b in pem_blocks(bytes) {
        let tipo = match b.label_name.as_str() {
            "PRIVATE KEY" | "ENCRYPTED PRIVATE KEY" | "RSA PRIVATE KEY" | "EC PRIVATE KEY" => b.label_name.as_str(),
            _ => continue,
        };
        let data = b.data?;
        let dek = b.headers.iter().find(|(k, _)| k == "DEK-Info").map(|(_, v)| v.as_str());
        let encrypted = b.headers.iter().any(|(k, v)| k == "Proc-Type" && v.ends_with("ENCRYPTED"));
        let data = match (encrypted, dek) {
            (true, Some(dek)) => decrypt_legacy_pem(dek, password, &data).ok_or(())?,
            (true, None) => return Err(()),
            _ => data,
        };
        return Ok(Some(match tipo {
            "PRIVATE KEY" => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(data)),
            "ENCRYPTED PRIVATE KEY" => PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(decrypt_pkcs8(&data, password).map_err(|_| ())?)),
            "RSA PRIVATE KEY" => PrivateKeyDer::Pkcs1(PrivatePkcs1KeyDer::from(data)),
            _ => PrivateKeyDer::Sec1(PrivateSec1KeyDer::from(data)),
        }));
    }
    Ok(None)
}

/// Os certificados dos blocos PEM (`PEM_read_bio_X509` em laço): `Ok(None)`
/// se não há nenhum bloco de certificado.
fn pem_certificates(bytes: &[u8]) -> Result<Option<Vec<rustls::pki_types::CertificateDer<'static>>>, String> {
    let mut certs = Vec::new();
    for b in pem_blocks(bytes) {
        if !matches!(b.label_name.as_str(), "CERTIFICATE" | "X509 CERTIFICATE" | "TRUSTED CERTIFICATE") {
            continue;
        }
        let data = b.data.map_err(|()| "BAD_BASE64_DECODE(pem_lib.c:752)".to_string())?;
        certs.push(rustls::pki_types::CertificateDer::from(data));
    }
    Ok((!certs.is_empty()).then_some(certs))
}

// ---------------------------------------------------------------------------
// PKCS#12.

/// O conteúdo de um PKCS#12: a chave (com o `localKeyId`) e os
/// certificados (com o `localKeyId` de cada), na ordem do arquivo.
#[derive(Default)]
struct Pkcs12 {
    key: Option<(Vec<u8>, Option<Vec<u8>>)>,
    certificates: Vec<(Vec<u8>, Option<Vec<u8>>)>,
}

const PKCS12_MALFORMED: &str = "BAD_PKCS12_DATA(pkcs8_x509.c:599)";
const PKCS12_INVALID: &str = "BAD_PKCS12_DATA(pkcs8_x509.c:611)";
const WRONG_PASSWORD: &str = "INCORRECT_PASSWORD(pkcs8_x509.c:710)";

/// Lê um PKCS#12 (`PKCS12_get_key_and_certs`), conferindo a MAC.
fn read_pkcs12(der: &[u8], password: &str) -> Result<Pkcs12, &'static str> {
    let (pfx, rest) = wait_for(der, TAG_SEQUENCE).ok_or(PKCS12_MALFORMED)?;
    if !rest.is_empty() {
        return Err(PKCS12_MALFORMED);
    }
    let (version, r) = read_tlv(&pfx.content).ok_or(PKCS12_MALFORMED)?;
    if small_int(&version) != Some(3) {
        return Err(PKCS12_INVALID);
    }
    let (secure, r) = wait_for(r, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
    let (tipo, content) = wait_for(&secure.content, TAG_OID).ok_or(PKCS12_INVALID)?;
    if tipo.content.as_ref() != oid::DADOS {
        return Err(PKCS12_INVALID);
    }
    let (explicit, _) = wait_for(content, 0xa0).ok_or(PKCS12_INVALID)?;
    let (octets, _) = wait_for(&explicit.content, TAG_OCTETS).ok_or(PKCS12_INVALID)?;
    let secure = octets.content;
    // A MAC (sem ela o arquivo é aceito, como no BoringSSL).
    if let Some((mac, _)) = wait_for(r, TAG_SEQUENCE) {
        check_mac(&mac.content, &secure, password)?;
    }
    let mut p = Pkcs12::default();
    let (list, _) = wait_for(&secure, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
    for ci in elements(&list.content).ok_or(PKCS12_INVALID)? {
        let (tipo, content) = wait_for(&ci.content, TAG_OID).ok_or(PKCS12_INVALID)?;
        let (explicit, _) = wait_for(content, 0xa0).ok_or(PKCS12_INVALID)?;
        let bags = match tipo.content.as_ref() {
            oid::DADOS => wait_for(&explicit.content, TAG_OCTETS).ok_or(PKCS12_INVALID)?.0.content.into_owned(),
            oid::ENCRYPTED_DATA => {
                // EncryptedData { versão, EncryptedContentInfo { tipo, alg, [0] cifrado } }
                let (ed, _) = wait_for(&explicit.content, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
                let (_, r) = read_tlv(&ed.content).ok_or(PKCS12_INVALID)?;
                let (eci, _) = wait_for(r, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
                let (_, r) = wait_for(&eci.content, TAG_OID).ok_or(PKCS12_INVALID)?;
                let (alg, r) = wait_for(r, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
                let (encrypted, _) = read_tlv(r).ok_or(PKCS12_INVALID)?;
                if encrypted.tag & 0xdf != 0x80 {
                    return Err(PKCS12_INVALID);
                }
                let encrypted = if encrypted.tag == 0xa0 {
                    // `[0]` construído: os pedaços `OCTET STRING`.
                    elements(&encrypted.content).ok_or(PKCS12_INVALID)?.iter().flat_map(|t| t.content.iter().copied()).collect()
                } else {
                    encrypted.content.into_owned()
                };
                decrypt_pbe(&alg, password, &encrypted)?
            }
            _ => continue,
        };
        read_bags(&bags, password, &mut p, 0)?;
    }
    Ok(p)
}

/// Confere a `MacData` (a senha em BMP; a vazia também sem terminador, como
/// o BoringSSL tenta).
fn check_mac(mac: &[u8], data: &[u8], password: &str) -> Result<(), &'static str> {
    let part_list = elements(mac).ok_or(PKCS12_INVALID)?;
    let info = part_list.first().filter(|t| t.tag == TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
    let (alg, r) = wait_for(&info.content, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
    let (digest_alg, _) = wait_for(r, TAG_OCTETS).ok_or(PKCS12_INVALID)?;
    let (o, _) = wait_for(&alg.content, TAG_OID).ok_or(PKCS12_INVALID)?;
    let (h, v) = oid_digest(&o.content).ok_or("UNSUPPORTED_DIGEST(pkcs8_x509.c:0)")?;
    let salt = part_list.get(1).filter(|t| t.tag == TAG_OCTETS).ok_or(PKCS12_INVALID)?;
    let iterations = match part_list.get(2) {
        Some(t) => small_int(t).ok_or(PKCS12_INVALID)?,
        None => 1,
    };
    let hmac = match h.output_len() {
        20 => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
        32 => ring::hmac::HMAC_SHA256,
        48 => ring::hmac::HMAC_SHA384,
        _ => ring::hmac::HMAC_SHA512,
    };
    let mut candidate_list = vec![bmp_password(password)];
    if password.is_empty() {
        candidate_list.push(Vec::new());
    }
    for s in candidate_list {
        let key = kdf_pkcs12(h, v, 3, &s, &salt.content, iterations, h.output_len());
        let k = ring::hmac::Key::new(hmac, &key);
        if ring::hmac::verify(&k, data, &digest_alg.content).is_ok() {
            return Ok(());
        }
    }
    Err(WRONG_PASSWORD)
}

/// As `SafeContents` (`SEQUENCE OF SafeBag`).
fn read_bags(der: &[u8], password: &str, p: &mut Pkcs12, depth: usize) -> Result<(), &'static str> {
    if depth > 3 {
        return Err(PKCS12_INVALID);
    }
    let (seq, _) = wait_for(der, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
    for bag in elements(&seq.content).ok_or(PKCS12_INVALID)? {
        let (tipo, r) = wait_for(&bag.content, TAG_OID).ok_or(PKCS12_INVALID)?;
        let (value, r) = wait_for(r, 0xa0).ok_or(PKCS12_INVALID)?;
        let local_id = wait_for(r, TAG_SET).and_then(|(attributes, _)| local_key_id(&attributes.content));
        match tipo.content.as_ref() {
            oid::KEY_BAG => {
                if p.key.is_none() {
                    let (k, _) = wait_for(&value.content, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
                    let _ = k;
                    p.key = Some((value.content.to_vec(), local_id));
                }
            }
            oid::SHROUDED_KEY_BAG => {
                if p.key.is_none() {
                    p.key = Some((decrypt_pkcs8(&value.content, password)?, local_id));
                }
            }
            oid::CERT_BAG => {
                let (cb, _) = wait_for(&value.content, TAG_SEQUENCE).ok_or(PKCS12_INVALID)?;
                let (t, r) = wait_for(&cb.content, TAG_OID).ok_or(PKCS12_INVALID)?;
                if t.content.as_ref() != oid::X509_CERTIFICATE {
                    continue;
                }
                let (e, _) = wait_for(r, 0xa0).ok_or(PKCS12_INVALID)?;
                let (cert, _) = wait_for(&e.content, TAG_OCTETS).ok_or(PKCS12_INVALID)?;
                p.certificates.push((cert.content.into_owned(), local_id));
            }
            oid::SACO_DE_SACOS => read_bags(&value.content, password, p, depth + 1)?,
            _ => {}
        }
    }
    Ok(())
}

/// O `localKeyId` dos atributos de um saco.
fn local_key_id(attributes: &[u8]) -> Option<Vec<u8>> {
    for a in elements(attributes)? {
        let (o, r) = wait_for(&a.content, TAG_OID)?;
        if o.content.as_ref() == oid::LOCAL_KEY_ID {
            let (value_list, _) = wait_for(r, TAG_SET)?;
            let (v, _) = wait_for(&value_list.content, TAG_OCTETS)?;
            return Some(v.content.into_owned());
        }
    }
    None
}

impl Pkcs12 {
    /// `PKCS12_parse`: o certificado da chave (o de mesmo `localKeyId`, ou o
    /// primeiro) e os demais, na ordem do arquivo.
    fn certificate_and_rest(self) -> Vec<rustls::pki_types::CertificateDer<'static>> {
        let id = self.key.as_ref().and_then(|(_, id)| id.clone());
        let mut certs = self.certificates;
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
fn key_of(bytes: &[u8], password: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>, ()> {
    match pem_key(bytes, password)? {
        Some(k) => Ok(k),
        None => {
            let p = read_pkcs12(bytes, password).map_err(|_| ())?;
            let (k, _) = p.key.ok_or(())?;
            Ok(rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(k)))
        }
    }
}

/// Os certificados de `bytes` (`SetTrustedCertificatesBytes`,
/// `UseChainBytes`, `SetClientAuthoritiesBytes` da VM): os do PEM e, sem
/// linha de início, os do PKCS#12 (o da chave primeiro). O erro é o texto
/// do BoringSSL.
fn certificates_of(bytes: &[u8], password: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, String> {
    match pem_certificates(bytes)? {
        Some(c) => Ok(c),
        None => {
            let p = read_pkcs12(bytes, password).map_err(str::to_string)?;
            Ok(p.certificate_and_rest())
        }
    }
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    #[test]
    fn pkcs12_kdf_from_rfc() {
        // Vetor do OpenSSL (`test/pkcs12_format_test` / RFC 7292): senha
        // "smeg", sal 0A58CF64530D823F, 1 iteração, ID 1, 24 bytes.
        let salt = [0x0a, 0x58, 0xcf, 0x64, 0x53, 0x0d, 0x82, 0x3f];
        let k = kdf_pkcs12(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, 64, 1, &bmp_password("smeg"), &salt, 1, 24);
        let expected = [
            0x8a, 0xaa, 0xe6, 0x29, 0x7b, 0x6c, 0xb0, 0x46, 0x42, 0xab, 0x5b, 0x07, 0x78, 0x51, 0x28, 0x4e, 0xb7, 0x12, 0x8f, 0x1a, 0x2a, 0x7f, 0xbc,
            0xa3,
        ];
        assert_eq!(k, expected);
    }

    #[test]
    fn rc4_known_vector() {
        // O vetor clássico: chave "Key", texto "Plaintext".
        assert_eq!(rc4(b"Key", b"Plaintext"), [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3]);
    }

    #[test]
    fn base64_with_and_without_padding() {
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(base64_decode("TQ==").unwrap(), b"M");
        assert!(base64_decode("T@==").is_none());
    }

    #[test]
    fn ber_indefinite_and_segmented_octets() {
        // SEQUENCE indefinida com um INTEGER 5; OCTET STRING construída.
        let (t, r) = read_tlv(&[0x30, 0x80, 0x02, 0x01, 0x05, 0x00, 0x00, 0xff]).unwrap();
        assert_eq!((t.tag, t.content.as_ref(), r), (0x30, &[0x02, 0x01, 0x05][..], &[0xff][..]));
        let (t, _) = read_tlv(&[0x24, 0x80, 0x04, 0x01, 0xaa, 0x04, 0x01, 0xbb, 0x00, 0x00]).unwrap();
        assert_eq!((t.tag, t.content.as_ref()), (0x04, &[0xaa, 0xbb][..]));
    }
}

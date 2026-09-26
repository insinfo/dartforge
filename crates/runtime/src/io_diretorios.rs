// Runtime nativo: o `dart:io` da VM — diretórios (`runtime/bin/directory.cc`,
// `directory_linux.cc`…): os natives de `_Directory`, a listagem síncrona
// (`Directory_FillWithDirectoryListing`) e a assíncrona, que o IOService
// avança em lotes (`io_servico.rs`).

/// `Directory::ExistsResult`.
const DIRETORIO_NAO_EXISTE: i64 = 0;
const DIRETORIO_EXISTE: i64 = 1;

/// `Directory::Exists`: existe, não existe, ou erro — sem permissão ou
/// uma falha do sistema, em que não se sabe (o `UNKNOWN` da VM); os demais
/// erros (não existe, não é diretório, laço de links, nome longo) são "não
/// existe".
fn diretorio_existe(p: &std::path::Path) -> ResultadoIo<bool> {
    match std::fs::metadata(p) {
        Ok(m) => Ok(m.is_dir()),
        Err(e) => match e.kind() {
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::OutOfMemory => Err(ErroDoSo::de(&e)),
            _ => Ok(false),
        },
    }
}

/// `Directory::Create`: um diretório que já existe é sucesso.
fn criar_diretorio(p: &std::path::Path) -> ResultadoIo<()> {
    match std::fs::create_dir(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if diretorio_existe(p)? {
                Ok(())
            } else {
                Err(ErroDoSo::de(&e))
            }
        }
        Err(e) => Err(ErroDoSo::de(&e)),
    }
}

/// `Directory::CreateTemp`: o prefixo seguido de seis letras maiúsculas
/// aleatórias, até um nome livre.
fn criar_diretorio_temporario(prefixo: &[u8]) -> ResultadoIo<Vec<u8>> {
    loop {
        let mut nome = prefixo.to_vec();
        let mut aleatorio = entropia();
        for _ in 0..6 {
            nome.push(b'A' + (aleatorio % 26) as u8);
            aleatorio /= 26;
        }
        match std::fs::create_dir(caminho_de_bytes(&nome)) {
            Ok(()) => return Ok(nome),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(ErroDoSo::de(&e)),
        }
    }
}

/// `Directory::SystemTemp`: `TMPDIR`, `TMP` ou `/tmp`, sem a barra final
/// (Unix); `GetTempPath` sem a barra final (Windows).
fn diretorio_temporario_do_sistema() -> Vec<u8> {
    #[cfg(unix)]
    let mut b = {
        use std::os::unix::ffi::OsStrExt;
        std::env::var_os("TMPDIR")
            .or_else(|| std::env::var_os("TMP"))
            .map_or_else(|| b"/tmp".to_vec(), |v| v.as_bytes().to_vec())
    };
    #[cfg(windows)]
    let mut b = bytes_de_caminho(&std::env::temp_dir());
    if b.len() > 1 && matches!(b.last(), Some(b'/') | Some(b'\\')) {
        b.pop();
    }
    b
}

/// `Directory::Delete`. Sem `recursivo`: um link para diretório é apagado
/// como link, o resto com `rmdir`. Com `recursivo`: o `DeleteRecursively`
/// da VM — o que não é diretório (links inclusive) é apagado, e diretórios
/// são esvaziados antes.
fn apagar_diretorio(p: &std::path::Path, recursivo: bool) -> ResultadoIo<()> {
    if !recursivo {
        if tipo_do_caminho(p, false) == TIPO_LINK && tipo_do_caminho(p, true) == TIPO_DIRETORIO {
            #[cfg(windows)]
            return Ok(std::fs::remove_dir(p)?);
            #[cfg(unix)]
            return Ok(std::fs::remove_file(p)?);
        }
        return Ok(std::fs::remove_dir(p)?);
    }
    apagar_recursivamente(p)
}

fn apagar_recursivamente(p: &std::path::Path) -> ResultadoIo<()> {
    let m = std::fs::symlink_metadata(p)?;
    if !m.is_dir() {
        #[cfg(windows)]
        if m.file_type().is_symlink() && std::fs::metadata(p).is_ok_and(|d| d.is_dir()) {
            return Ok(std::fs::remove_dir(p)?);
        }
        return Ok(std::fs::remove_file(p)?);
    }
    for entrada in std::fs::read_dir(p)? {
        apagar_recursivamente(&entrada?.path())?;
    }
    Ok(std::fs::remove_dir(p)?)
}

/// `Directory::Rename`: a origem precisa ser um diretório.
fn renomear_diretorio(de: &std::path::Path, para: &std::path::Path) -> ResultadoIo<()> {
    if !diretorio_existe(de)? {
        return Err(ErroDoSo::do_codigo(codigo_do_so::NAO_E_DIRETORIO));
    }
    Ok(std::fs::rename(de, para)?)
}

/// `Directory::Current`.
fn diretorio_corrente() -> ResultadoIo<Vec<u8>> {
    Ok(bytes_de_caminho(&std::env::current_dir()?))
}

// ---------------------------------------------------------------------------
// A listagem (`DirectoryListing`/`DirectoryListingEntry` da VM).

/// `ListType` da VM.
const LISTA_ARQUIVO: i64 = 0;
const LISTA_DIRETORIO: i64 = 1;
const LISTA_LINK: i64 = 2;
const LISTA_ERRO: i64 = 3;
const LISTA_FIM: i64 = 4;

/// Uma entrada produzida pela listagem.
enum EntradaDeListagem {
    /// Arquivo, diretório ou link (`ListType`) e o caminho em bytes.
    Item(i64, Vec<u8>),
    /// O erro e o caminho em que ocorreu (o `CurrentPath` da VM).
    Erro(ErroDoSo, Vec<u8>),
    Fim,
}

/// A identidade de um link seguido (dispositivo e nó), para não entrar num
/// ciclo de links (`LinkList` da VM).
type IdentidadeDeNo = (u64, u64);

#[cfg(unix)]
fn identidade_de_no(m: &std::fs::Metadata) -> IdentidadeDeNo {
    use std::os::unix::fs::MetadataExt;
    (m.dev(), m.ino())
}
#[cfg(windows)]
fn identidade_de_no(_m: &std::fs::Metadata) -> IdentidadeDeNo {
    // No Windows, o `std` não expõe o índice do arquivo estável; ciclos de
    // links de diretório são detectados pelo caminho canônico.
    (0, 0)
}

/// Um nível da pilha: o diretório aberto, o prefixo dos caminhos das
/// entradas (com o separador final) e os links já seguidos até aqui.
struct NivelDeListagem {
    leitor: Option<std::fs::ReadDir>,
    prefixo: Vec<u8>,
    links: Vec<IdentidadeDeNo>,
}

/// Uma listagem em andamento (`DirectoryListing`).
struct Listagem {
    pilha: Vec<NivelDeListagem>,
    recursivo: bool,
    seguir_links: bool,
}

impl Listagem {
    /// `caminho` já traz o separador final (`_ensureTrailingPathSeparators`).
    fn nova(caminho: Vec<u8>, recursivo: bool, seguir_links: bool) -> Listagem {
        Listagem {
            pilha: vec![NivelDeListagem { leitor: None, prefixo: caminho, links: Vec::new() }],
            recursivo,
            seguir_links,
        }
    }

    fn vazia(&self) -> bool {
        self.pilha.is_empty()
    }

    /// A próxima entrada (o `ListNext` + `DirectoryListingEntry::Next` da
    /// VM): um diretório recursivo empilha o nível dele depois de ser
    /// entregue; ao fim de um nível, o de cima continua.
    fn proxima(&mut self) -> EntradaDeListagem {
        loop {
            let Some(nivel) = self.pilha.last_mut() else {
                return EntradaDeListagem::Fim;
            };
            if nivel.leitor.is_none() {
                match std::fs::read_dir(caminho_de_bytes(&nivel.prefixo)) {
                    Ok(r) => nivel.leitor = Some(r),
                    Err(e) => {
                        // O caminho do erro é o do diretório: o da raiz como
                        // veio; o de um subdiretório sem o separador que a
                        // VM só acrescenta depois de abri-lo.
                        let mut caminho = nivel.prefixo.clone();
                        if self.pilha.len() > 1 {
                            caminho.truncate(caminho.len() - SEPARADOR_DE_CAMINHO.len());
                        }
                        self.pilha.pop();
                        return EntradaDeListagem::Erro(ErroDoSo::de(&e), caminho);
                    }
                }
            }
            let prefixo = nivel.prefixo.clone();
            let entrada = nivel.leitor.as_mut().expect("aberto acima").next();
            let entrada = match entrada {
                None => {
                    self.pilha.pop();
                    continue;
                }
                Some(Err(e)) => {
                    self.pilha.pop();
                    return EntradaDeListagem::Erro(ErroDoSo::de(&e), prefixo);
                }
                Some(Ok(e)) => e,
            };
            let mut caminho = prefixo;
            caminho.extend_from_slice(&bytes_de_caminho(std::path::Path::new(&entrada.file_name())));
            let tipo = match self.classificar(&entrada, &caminho) {
                Ok(t) => t,
                Err(e) => return EntradaDeListagem::Erro(e, caminho),
            };
            if tipo == LISTA_DIRETORIO && self.recursivo {
                let nivel = self.pilha.last().expect("nível corrente");
                let mut links = nivel.links.clone();
                if let Ok(m) = std::fs::symlink_metadata(caminho_de_bytes(&caminho)) {
                    if m.file_type().is_symlink() {
                        if let Ok(alvo) = std::fs::metadata(caminho_de_bytes(&caminho)) {
                            links.push(identidade_de_no(&alvo));
                        }
                    }
                }
                let mut sub = caminho.clone();
                sub.extend_from_slice(SEPARADOR_DE_CAMINHO.as_bytes());
                self.pilha.push(NivelDeListagem { leitor: None, prefixo: sub, links });
            }
            return EntradaDeListagem::Item(tipo, caminho);
        }
    }

    /// O tipo de uma entrada: sem seguir links, um link é link; seguindo, o
    /// tipo do alvo — um link quebrado ou em ciclo continua link.
    fn classificar(&self, e: &std::fs::DirEntry, caminho: &[u8]) -> ResultadoIo<i64> {
        let t = e.file_type()?;
        if t.is_dir() {
            return Ok(LISTA_DIRETORIO);
        }
        if !t.is_symlink() {
            return Ok(LISTA_ARQUIVO);
        }
        if !self.seguir_links {
            return Ok(LISTA_LINK);
        }
        let p = caminho_de_bytes(caminho);
        let Ok(alvo) = std::fs::metadata(&p) else {
            return Ok(LISTA_LINK);
        };
        if alvo.is_dir() {
            #[cfg(unix)]
            {
                let id = identidade_de_no(&alvo);
                if self.pilha.last().is_some_and(|n| n.links.contains(&id)) {
                    return Ok(LISTA_LINK);
                }
            }
            #[cfg(windows)]
            {
                // Um link para um ancestral da listagem é um ciclo.
                if let Ok(c) = std::fs::canonicalize(&p) {
                    if self.pilha.iter().any(|n| std::fs::canonicalize(caminho_de_bytes(&n.prefixo)).is_ok_and(|a| a == c)) {
                        return Ok(LISTA_LINK);
                    }
                }
            }
            return Ok(LISTA_DIRETORIO);
        }
        Ok(LISTA_ARQUIVO)
    }

    /// Fecha tudo (`PopAll`).
    fn parar(&mut self) {
        self.pilha.clear();
    }
}

/// O separador de caminho da plataforma (`File::PathSeparator`).
#[cfg(unix)]
const SEPARADOR_DE_CAMINHO: &str = "/";
#[cfg(windows)]
const SEPARADOR_DE_CAMINHO: &str = "\\";

// ---------------------------------------------------------------------------
// Os natives de `_Directory`.

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_Current(_ns: i64) -> i64 {
    dart_ou_erro(diretorio_corrente(), |b| dart_texto_de_bytes(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_SetCurrent(_ns: i64, raw: i64) -> i64 {
    dart_verdadeiro_ou_erro(std::env::set_current_dir(caminho_raw(raw)).map_err(ErroDoSo::from))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_Exists(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(diretorio_existe(&caminho_raw(raw)), |e| {
        dart_int(if e { DIRETORIO_EXISTE } else { DIRETORIO_NAO_EXISTE })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_Create(_ns: i64, raw: i64) -> i64 {
    dart_verdadeiro_ou_erro(criar_diretorio(&caminho_raw(raw)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_SystemTemp(_ns: i64) -> i64 {
    dart_texto_de_bytes(&diretorio_temporario_do_sistema())
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_CreateTemp(_ns: i64, raw: i64) -> i64 {
    dart_ou_erro(criar_diretorio_temporario(&bytes_do_caminho(raw)), |b| dart_texto_de_bytes(&b))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_Delete(_ns: i64, raw: i64, recursivo: u8) -> i64 {
    dart_verdadeiro_ou_erro(apagar_diretorio(&caminho_raw(raw), recursivo != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_Rename(_ns: i64, raw: i64, novo: i64) -> i64 {
    dart_verdadeiro_ou_erro(renomear_diretorio(&caminho_raw(raw), &caminho_texto(novo)))
}

/// `Directory_FillWithDirectoryListing`: acrescenta a `lista` um
/// `File`/`Directory`/`Link` por entrada (`fromRawPath`); o primeiro erro
/// é lançado como `FileSystemException` ("Directory listing failed").
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_FillWithDirectoryListing(
    _ns: i64,
    lista: i64,
    raw: i64,
    recursivo: u8,
    seguir_links: u8,
) {
    let (Some(entrada), Some(erro)) = (ajudante("_dartforgeEntradaDeListagem"), ajudante("_dartforgeErroDeListagem")) else {
        panic!("bug do compilador: dart:io sem as funções de listagem da sobreposição");
    };
    // SAFETY: registradas pela biblioteca `dart:io` da sobreposição com
    // estas assinaturas.
    let entrada: extern "C" fn(i64, i64, i64) = unsafe { std::mem::transmute(entrada) };
    let erro: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(erro) };
    let mut listagem = Listagem::nova(bytes_do_caminho(raw), recursivo != 0, seguir_links != 0);
    loop {
        match listagem.proxima() {
            EntradaDeListagem::Fim => return,
            EntradaDeListagem::Item(tipo, caminho) => {
                let bytes = dart_bytes(caminho);
                com_raizes(&[bytes], || entrada(lista, tipo, bytes));
                if dartforge_exception_pending() != 0 {
                    return;
                }
            }
            EntradaDeListagem::Erro(e, caminho) => {
                let os = e.para_dart();
                let texto = com_raizes(&[os], || dart_texto_de_bytes(&caminho));
                let excecao = com_raizes(&[os, texto], || erro(os, texto));
                if dartforge_exception_pending() == 0 {
                    com_raizes(&[excecao], || dartforge_exception_throw(excecao, 3));
                }
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A listagem assíncrona: o objeto nativo que o IOService avança.

/// Uma listagem assíncrona (`AsyncDirectoryListing`), compartilhada entre o
/// objeto Dart e as threads do IOService por contagem de referências.
struct ListagemAssincrona {
    listagem: std::sync::Mutex<Listagem>,
}

impl ListagemAssincrona {
    fn nova(l: Listagem) -> i64 {
        std::sync::Arc::into_raw(std::sync::Arc::new(ListagemAssincrona { listagem: std::sync::Mutex::new(l) })) as i64
    }

    fn reter(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `nova`.
        unsafe { std::sync::Arc::increment_strong_count(p as *const ListagemAssincrona) };
    }

    fn liberar(p: i64) {
        // SAFETY: `p` é um ponteiro vivo de `nova`, com esta referência.
        unsafe { std::sync::Arc::decrement_strong_count(p as *const ListagemAssincrona) };
    }

    /// Toma a referência de `p` (a do pedido do IOService).
    ///
    /// # Safety
    /// `p` é um ponteiro de `nova` cuja referência passa a quem chama.
    unsafe fn tomar(p: i64) -> std::sync::Arc<ListagemAssincrona> {
        // SAFETY: garantido por quem chama.
        unsafe { std::sync::Arc::from_raw(p as *const ListagemAssincrona) }
    }

    /// Até `limite` posições da resposta de `ListNext`: pares
    /// `[tipo, caminho]` (o caminho em bytes; `null` no fim) e, num erro,
    /// `[kListError, [kListError, caminho, erro]]`.
    fn lote(&self, limite: usize) -> Vec<Portavel> {
        let mut l = self.listagem.lock().unwrap_or_else(|e| e.into_inner());
        let mut saida = Vec::new();
        if l.vazia() {
            return saida;
        }
        while saida.len() < limite {
            match l.proxima() {
                EntradaDeListagem::Item(tipo, caminho) => {
                    saida.push(Portavel::Int(tipo));
                    saida.push(Portavel::Bytes(caminho));
                }
                EntradaDeListagem::Erro(e, caminho) => {
                    let caminho = String::from_utf8_lossy(&caminho).into_owned();
                    saida.push(Portavel::Int(LISTA_ERRO));
                    saida.push(Portavel::Lista(vec![Portavel::Int(LISTA_ERRO), Portavel::Str(caminho), e.para_resposta()]));
                }
                EntradaDeListagem::Fim => {
                    saida.push(Portavel::Int(LISTA_FIM));
                    saida.push(Portavel::Nulo);
                    break;
                }
            }
        }
        saida
    }
}

/// O finalizador do objeto Dart dono de uma listagem.
fn liberar_listagem_do_objeto(par: usize) {
    ListagemAssincrona::liberar(par as i64);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_SetAsyncDirectoryListerPointer(this: i64, p: i64) {
    anexar_finalizador(this, liberar_listagem_do_objeto, p as usize);
    gravar_campo_nativo(this, p);
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Directory_GetAsyncDirectoryListerPointer(this: i64) -> i64 {
    let p = campo_nativo(this);
    if p != 0 {
        ListagemAssincrona::reter(p);
    }
    p
}

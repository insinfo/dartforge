//! Cache do SDK analisado (`sdk_cache.rs`): a carga pelo cache produz o mesmo
//! `Program` que a carga pelos arquivos e é mais rápida. Precisa do SDK 3.6.2;
//! sem ele o teste é pulado.

use dartforge_elements::load::{load_lenient, load_lenient_com_cache};
use dartforge_elements::sdk::SdkLayout;
use dartforge_elements::SdkCache;
use dartforge_intern::Interner;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::tempdir;

fn sdk_real() -> Option<SdkLayout> {
    let lib = SdkLayout::discover().or_else(|| {
        let p = PathBuf::from("C:/tools/dartsdk-3.6.2/lib");
        p.join("libraries.json").exists().then_some(p)
    })?;
    SdkLayout::load(&lib, "dartdevc").ok()
}

#[test]
fn cache_do_sdk_reproduz_o_programa_e_e_mais_rapido() {
    let Some(sdk) = sdk_real() else {
        eprintln!("SDK não encontrado; teste pulado");
        return;
    };
    let tmp = tempdir().unwrap();
    let entry = tmp.path().join("main.dart");
    fs::write(
        &entry,
        "import 'dart:async';\nimport 'dart:convert';\nimport 'dart:collection';\nimport 'dart:math';\nimport 'dart:typed_data';\n\
         part 'parte.dart';\nclass C { int x = 1; }\nvoid main() { print(json.encode([C().x, max(1, 2)])); }\n",
    )
    .unwrap();
    fs::write(tmp.path().join("parte.dart"), "part of 'main.dart';\nint dobro(int a) => a * 2;\n").unwrap();

    // 1. Pelos arquivos.
    let mut i1 = Interner::new();
    let (p1, d1) = load_lenient(&entry, &sdk, None, &mut i1);
    assert!(d1.is_empty(), "{d1:?}");

    // 2. Constrói o cache (uma vez) e carrega por ele.
    let caminho = tmp.path().join("sdk.bin");
    let t1 = Instant::now();
    SdkCache::construir_e_gravar(&sdk, &caminho).expect("constrói o cache");
    let t_construir = t1.elapsed();
    assert!(caminho.exists());
    let cache = SdkCache::abrir(&caminho).expect("abre o cache");
    let mut i2 = Interner::new();
    let (p2, d2) = load_lenient_com_cache(&entry, &sdk, None, &mut i2, Some(cache));
    assert!(d2.is_empty(), "{d2:?}");

    // 3. Tempo: o mínimo de N execuções de cada caminho, intercaladas (o
    // ruído do runner só soma; o mínimo é o custo do trabalho). No perfil
    // `dev` do teste, o cache com a decodificação em paralelo fica em ~45% dos
    // arquivos (75 ms × 165 ms na máquina de desenvolvimento; em série
    // empatava, 150 × 155 ms, e o teste reprovava ao acaso); em `release`,
    // 15 × 44 ms (em série, 22 ms). A margem exige que continue claramente mais rápido.
    const N: usize = 5;
    let mut t_arquivos = std::time::Duration::MAX;
    let mut t_cache = std::time::Duration::MAX;
    for _ in 0..N {
        let t = Instant::now();
        let _ = load_lenient(&entry, &sdk, None, &mut Interner::new());
        t_arquivos = t_arquivos.min(t.elapsed());
        let t = Instant::now();
        let cache = SdkCache::abrir(&caminho).expect("abre o cache");
        let _ = load_lenient_com_cache(&entry, &sdk, None, &mut Interner::new(), Some(cache));
        t_cache = t_cache.min(t.elapsed());
    }

    eprintln!(
        "mínimo de {N}: arquivos {t_arquivos:.1?}; abrir+carregar pelo cache {t_cache:.1?} ({:.0}%); construir cache {t_construir:.1?}; \
         unidades {} / bibliotecas {} / classes {} / funções {} / variáveis {}",
        100.0 * t_cache.as_secs_f64() / t_arquivos.as_secs_f64(),
        p2.units.len(), p2.libraries.len(), p2.classes.len(), p2.functions.len(), p2.variables.len()
    );

    // Mesmo programa: contagens, URIs e papéis das unidades na mesma ordem,
    // e os mesmos nomes de classes/funções na mesma ordem.
    assert_eq!(p1.libraries.len(), p2.libraries.len());
    assert_eq!(p1.units.len(), p2.units.len());
    assert_eq!(p1.classes.len(), p2.classes.len());
    assert_eq!(p1.functions.len(), p2.functions.len());
    assert_eq!(p1.variables.len(), p2.variables.len());
    assert_eq!(p1.extensions.len(), p2.extensions.len());
    assert_eq!(p1.typedefs.len(), p2.typedefs.len());
    for (u1, u2) in p1.units.iter().zip(&p2.units) {
        assert_eq!(u1.uri, u2.uri);
        assert_eq!(u1.role, u2.role);
        assert_eq!(u1.source, u2.source);
        assert_eq!(u1.ast.exprs.len(), u2.ast.exprs.len(), "{}", u1.uri);
        assert_eq!(u1.unit.declarations.len(), u2.unit.declarations.len(), "{}", u1.uri);
    }
    for (c1, c2) in p1.classes.iter().zip(&p2.classes) {
        assert_eq!(i1.resolve(c1.name), i2.resolve(c2.name));
        assert_eq!(c1.instance_members.len(), c2.instance_members.len());
    }
    for (f1, f2) in p1.functions.iter().zip(&p2.functions) {
        assert_eq!(i1.resolve(f1.name), i2.resolve(f2.name));
    }
    let sdk_units = p2.units.iter().filter(|u| p2.library(u.library).is_sdk).count();
    assert!(sdk_units > 100, "o SDK tem mais de 100 unidades: {sdk_units}");
    // A unidade do usuário e a sua parte continuam a vir dos arquivos.
    assert!(p2.units.iter().any(|u| u.uri.ends_with("/parte.dart") && !p2.library(u.library).is_sdk));

    assert!(
        t_cache.as_secs_f64() < 0.75 * t_arquivos.as_secs_f64(),
        "abrir+carregar pelo cache ({t_cache:?}, mínimo de {N}) deveria custar menos de 75% da carga pelos arquivos ({t_arquivos:?})"
    );
}

#[test]
fn cache_invalido_e_ignorado() {
    let Some(sdk) = sdk_real() else {
        return;
    };
    let tmp = tempdir().unwrap();
    let caminho = tmp.path().join("sdk.bin");
    fs::write(&caminho, b"lixo").unwrap();
    assert!(SdkCache::abrir(&caminho).is_none());
    assert!(!SdkCache::hash(&sdk, "dartdevc").is_empty());
    assert_ne!(SdkCache::hash(&sdk, "dartdevc"), SdkCache::hash(&sdk, "dart2js"));
}

//! Depuração: mostra como um construtor do SDK foi resolvido.
//! `cargo run -p dartforge-emit-js --example depura -- <arquivo.dart> Classe nome`
use dartforge_elements::load::load_lenient;
use dartforge_elements::model::{Element, FunctionRef};
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::ast::MemberKind;
use dartforge_intern::Interner;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entry = std::path::PathBuf::from(&args[0]);
    let sdk = SdkLayout::load(std::path::Path::new("C:/tools/dartsdk-3.6.2/lib"), "dartdevc").unwrap();
    let mut interner = Interner::new();
    let (program, diags) = load_lenient(&entry, &sdk, None, &mut interner);
    println!("diags: {}", diags.len());
    for d in diags.iter().take(20) {
        println!("  {d}");
    }
    let class_name = &args[1];
    let ctor_name = args.get(2).cloned().unwrap_or_default();
    for (i, c) in program.classes.iter().enumerate() {
        if interner.resolve(c.name) == class_name {
            let lib = program.library(c.library);
            println!("classe {} em {} (id {i}) supertype_class={:?} mixins={:?}", class_name, lib.uri, c.supertype_class, c.mixin_classes);
            for (sym, fid) in &c.static_members { let f = program.function(*fid); println!("  static {} kind={:?} node={:?}", interner.resolve(*sym), f.kind, f.node); }
            for (sym, fid) in &c.constructors {
                let f = program.function(*fid);
                let n = interner.resolve(*sym);
                if n != ctor_name && !ctor_name.is_empty() {
                    continue;
                }
                println!("  ctor '{n}' fid={:?} factory={} external={} patched_by={:?} node={:?}", fid, f.factory, f.external, f.patched_by, f.node);
                if let FunctionRef::Constructor { unit, member } = f.node {
                    let u = program.unit(unit);
                    let m = u.ast.member(member);
                    if let MemberKind::Constructor(ctor) = &m.kind {
                        println!("    unit={} redirect={:?} body={:?}", u.uri, ctor.redirect.as_ref().map(|r| &u.source[r.span.start..r.span.end]), matches!(ctor.body, dartforge_frontend::ast::FunctionBody::Empty));
                        if let Some(r) = &ctor.redirect {
                            let t = u.ast.ty(r.ty);
                            if let dartforge_frontend::ast::TypeKind::Named { name, .. } = &t.kind {
                                let sym = name[0].sym;
                                let b = program.lookup(u.library, sym);
                                println!("    alvo {:?} lookup={:?}", interner.resolve(sym), b.map(|b| matches!(b.getter, Some(Element::Class(_)))));
                            }
                        }
                    }
                }
            }
        }
    }
}

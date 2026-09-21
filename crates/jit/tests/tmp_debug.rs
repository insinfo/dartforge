//! temporario
use dartforge_jit::JitSession;

#[test]
fn a_sessao_viva_depois_parse_ruim() {
    eprintln!("A1 cria sessao");
    let mut s = JitSession::new().unwrap();
    eprintln!("A2 parse ruim");
    let e = s.add_ir_module("ruim", "nao e ir").unwrap_err();
    eprintln!("A3 {e}");
    drop(s);
    eprintln!("A4 fim");
}
#[test]
fn b_sessao_descartada_depois_parse_ruim() {
    eprintln!("B1 cria sessao");
    let s = JitSession::new().unwrap();
    drop(s);
    eprintln!("B2 sessao descartada; agora parse ruim numa nova sessao");
    let mut s2 = JitSession::new().unwrap();
    let e = s2.add_ir_module("ruim", "nao e ir").unwrap_err();
    eprintln!("B3 {e}");
    eprintln!("B4 fim");
}

//! Gerador de LLVM IR a partir da HIR nativa.

use crate::hir::*;
use std::fmt::Write;

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    out: String,
    string_constants: Vec<(String, usize)>,
}

impl<'a> LlvmEmitter<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            out: String::new(),
            string_constants: Vec::new(),
        }
    }

    pub fn emit_all(mut self) -> String {
        // Coleta literais de strings do módulo para declaração como constantes globais
        self.collect_string_constants();

        // 1. Cabeçalho de target
        self.emit_header();

        // 2. Declarações do runtime
        self.emit_runtime_decls();

        // 3. Constantes de strings globais
        self.emit_string_constants();

        // 4. Classes e vtables
        self.emit_vtables();

        // 5. Funções compiladas
        for func in &self.module.functions {
            self.emit_function(func);
        }

        // 6. Funções de despacho polimórfico
        self.emit_dispatch_functions();

        // 7. Entrada global @dartforge_entry
        self.emit_entry();

        self.out
    }

    fn collect_string_constants(&mut self) {
        for func in &self.module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    if let Instruction::Const(Constant::String(s)) = inst {
                        if !self.string_constants.iter().any(|(existing, _)| existing == s) {
                            let idx = self.string_constants.len();
                            self.string_constants.push((s.clone(), idx));
                        }
                    }
                }
            }
        }
        for class in &self.module.classes {
            if !self.string_constants.iter().any(|(existing, _)| existing == &class.name) {
                let idx = self.string_constants.len();
                self.string_constants.push((class.name.clone(), idx));
            }
        }
    }

    fn emit_header(&mut self) {
        self.out.push_str("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n");
        self.out.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
    }

    fn emit_runtime_decls(&mut self) {
        self.out.push_str("; Declarações do runtime nativo Rust\n");
        self.out.push_str("declare void @dartforge_print_i64(i64)\n");
        self.out.push_str("declare void @dartforge_print_f64(double)\n");
        self.out.push_str("declare void @dartforge_print_bool(i8)\n");
        self.out.push_str("declare void @dartforge_print_null()\n");
        self.out.push_str("declare void @dartforge_print_string(i64)\n");
        self.out.push_str("declare void @dartforge_print_list(i64)\n");
        self.out.push_str("declare void @dartforge_print_map(i64)\n");
        self.out.push_str("declare void @dartforge_print_set(i64)\n");
        self.out.push_str("declare void @dartforge_print_handle(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_new(ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_concat(i64, i64)\n");
        self.out.push_str("declare i8 @dartforge_string_equal(i64, i64)\n");
        self.out.push_str("declare i8 @dartforge_equal(i64, i64)\n");
        self.out.push_str("declare void @dartforge_register_class_name(i64, ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_object_new(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_object_get(i64, i64)\n");
        self.out.push_str("declare void @dartforge_object_set(i64, i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_object_class(i64)\n");
        self.out.push_str("declare i64 @dartforge_list_new(ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_len(i64)\n");
        self.out.push_str("declare i64 @dartforge_list_get_bits(i64, i64)\n");
        self.out.push_str("declare i8 @dartforge_list_get_tag(i64, i64)\n");
        self.out.push_str("declare void @dartforge_list_set(i64, i64, i64, i8)\n");
        self.out.push_str("declare void @dartforge_list_push(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_map_new(ptr, ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_map_len(i64)\n");
        self.out.push_str("declare i64 @dartforge_map_get_bits(i64, i64, i8)\n");
        self.out.push_str("declare i8 @dartforge_map_get_tag(i64, i64, i8)\n");
        self.out.push_str("declare void @dartforge_map_set(i64, i64, i8, i64, i8)\n");
        self.out.push_str("declare i8 @dartforge_map_contains(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_set_new(ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_set_len(i64)\n");
        self.out.push_str("declare i8 @dartforge_set_contains(i64, i64, i8)\n");
        self.out.push_str("declare i8 @dartforge_set_add(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_cell_new(i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_cell_get_bits(i64)\n");
        self.out.push_str("declare i8 @dartforge_cell_get_tag(i64)\n");
        self.out.push_str("declare void @dartforge_cell_set(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_env_new(ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_env_get(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_closure_new(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_closure_code(i64)\n");
        self.out.push_str("declare i64 @dartforge_closure_env(i64)\n");
        self.out.push_str("declare i64 @dartforge_tearoff(i64)\n");
        self.out.push_str("declare i64 @dartforge_gc_push_frame(i64)\n");
        self.out.push_str("declare void @dartforge_gc_set_root(i64, i64, i64)\n");
        self.out.push_str("declare void @dartforge_gc_root(i64, i64)\n");
        self.out.push_str("declare void @dartforge_gc_pop_frame(i64)\n");
        self.out.push_str("declare void @dartforge_gc_collect()\n");
        self.out.push_str("declare void @dartforge_null_assert_fail() noreturn\n");
        self.out.push_str("declare void @dartforge_exception_throw(i64, i8)\n");
        self.out.push_str("declare i8 @dartforge_exception_pending()\n");
        self.out.push_str("declare i64 @dartforge_exception_take_bits()\n");
        self.out.push_str("declare i8 @dartforge_exception_take_tag()\n");
        self.out.push_str("declare i64 @dartforge_exception_peek_bits()\n");
        self.out.push_str("declare i8 @dartforge_exception_peek_tag()\n");
        self.out.push_str("declare void @dartforge_exception_clear()\n");
        self.out.push_str("declare void @dartforge_register_subclass(i64, i64)\n");
        self.out.push_str("declare i8 @dartforge_is_subclass(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_stack_trace_get()\n");
        self.out.push_str("declare i64 @dartforge_stack_trace_empty()\n");
        self.out.push_str("declare i64 @dartforge_stack_trace_from_string(i64)\n");
        self.out.push_str("declare void @dartforge_throw_with_stack_trace(i64, i8, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_first(i64)\n");
        self.out.push_str("declare i64 @dartforge_list_last(i64)\n");
        self.out.push_str("declare i64 @dartforge_value_class(i64)\n");
        self.out.push_str("declare i64 @dartforge_to_string_i64(i64)\n");
        self.out.push_str("declare i64 @dartforge_to_string_f64(double)\n");
        self.out.push_str("declare i64 @dartforge_to_string_bool(i8)\n");
        self.out.push_str("declare i64 @dartforge_to_string_handle(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_len(i64)\n");
        self.out.push_str("declare i64 @dartforge_generic_len(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_code_unit_at(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_code_units(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_runes(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_to_upper(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_repeat(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_join(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_new_empty()\n");
        self.out.push_str("declare i64 @dartforge_map_get_to_string(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_record_new(ptr, i64)\n");
        self.out.push_str("declare i64 @dartforge_int_to_radix_string(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_int_parse(i64)\n");
        self.out.push_str("declare i64 @dartforge_int_try_parse(i64)\n");
        self.out.push_str("declare double @dartforge_double_parse(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_substring(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_from_char_code(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_from_char_codes(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_index_of(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_last_index_of(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_split(i64, i64)\n");
        self.out.push_str("declare i8 @dartforge_string_contains(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_replace_all(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_pad_left(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_pad_right(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_trim(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_trim_left(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_trim_right(i64)\n");
        self.out.push_str("declare i8 @dartforge_string_starts_with(i64, i64, i64)\n");
        self.out.push_str("declare i8 @dartforge_string_ends_with(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_to_lower(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_compare_to(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_replace_first(i64, i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_string_replace_range(i64, i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_reversed(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_buffer_new()\n");
        self.out.push_str("declare void @dartforge_string_buffer_write(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_regexp_new(i64)\n");
        self.out.push_str("declare i64 @dartforge_string_split_map_pieces(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_collection_mark_unmodifiable(i64)\n");
        self.out.push_str("declare i8 @dartforge_collection_is_unmodifiable(i64)\n");
        self.out.push_str("declare i64 @dartforge_exception_new(i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_format_exception_new(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_state_error_new(i64)\n");
        self.out.push_str("declare i64 @dartforge_argument_error_new(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_argument_error_value(i64, i8, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_argument_error_not_null(i64)\n");
        self.out.push_str("declare i64 @dartforge_range_error_new(i64)\n");
        self.out.push_str("declare i64 @dartforge_range_error_value(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_range_error_range(i64, i64, i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_range_error_index(i64, i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_unsupported_error_new(i64)\n");
        self.out.push_str("declare i64 @dartforge_unimplemented_error_new(i64)\n");
        self.out.push_str("declare i64 @dartforge_assertion_error_new(i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_concurrent_modification_error_new()\n");
        self.out.push_str("declare i64 @dartforge_type_error_new()\n");
        self.out.push_str("declare i64 @dartforge_no_such_method_error_new()\n");
        self.out.push_str("declare i64 @dartforge_error_get_message(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_name(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_invalid_value(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_start(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_end(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_source(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_offset(i64)\n");
        self.out.push_str("declare i64 @dartforge_error_get_stack_trace(i64)\n");
        self.out.push_str("declare i64 @dartforge_list_single(i64)\n");
        self.out.push_str("declare i64 @dartforge_list_sublist(i64, i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_remove_at(i64, i64)\n");
        self.out.push_str("declare i64 @dartforge_list_filled(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_map_remove(i64, i64, i8)\n");
        self.out.push_str("declare i64 @dartforge_map_keys(i64)\n");
        self.out.push_str("declare void @dartforge_iteration_begin(i64)\n");
        self.out.push_str("declare void @dartforge_iteration_end(i64)\n\n");
    }

    fn emit_string_constants(&mut self) {
        if self.string_constants.is_empty() {
            return;
        }
        self.out.push_str("; Constantes de string UTF-8\n");
        for (s, idx) in &self.string_constants {
            let bytes = s.as_bytes();
            let len = bytes.len();
            let mut escaped = String::new();
            for &b in bytes {
                if (b as char).is_ascii_alphanumeric() || b == b' ' || b == b'_' || b == b'.' {
                    escaped.push(b as char);
                } else {
                    write!(escaped, "\\{:02X}", b).unwrap();
                }
            }
            writeln!(
                self.out,
                "@.str.{idx} = private unnamed_addr constant [{len} x i8] c\"{escaped}\""
            ).unwrap();
        }
        self.out.push('\n');
    }

    fn string_const_index(&self, s: &str) -> Option<usize> {
        self.string_constants.iter().find(|(existing, _)| existing == s).map(|(_, idx)| *idx)
    }

    fn emit_vtables(&mut self) {
        // vtables globais se houver classes
        for class in &self.module.classes {
            writeln!(self.out, "; VTable da classe {} (id {})", class.name, class.id).unwrap();
        }
    }

    fn emit_function(&mut self, func: &Function) {
        let ret_ty = func.return_ty.llvm_ir();
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(vid, _, ty)| format!("{} %v{}", ty.llvm_ir(), vid.0))
            .collect();
        let params_str = params.join(", ");

        writeln!(self.out, "define {ret_ty} @{}({}) {{", func.symbol, params_str).unwrap();

        for block in &func.blocks {
            writeln!(self.out, "b{}:", block.id.0).unwrap();

            for (vid, inst, ty) in &block.instructions {
                let v = vid.0;
                match inst {
                    Instruction::Const(Constant::Int(n)) => {
                        writeln!(self.out, "  %v{v} = add i64 0, {n}").unwrap();
                    }
                    Instruction::Const(Constant::Double(d)) => {
                        let bits = d.to_bits();
                        writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap();
                    }
                    Instruction::Const(Constant::Bool(b)) => {
                        let bit = if *b { 1 } else { 0 };
                        writeln!(self.out, "  %v{v} = add i1 0, {bit}").unwrap();
                    }
                    Instruction::Const(Constant::Null) => {
                        writeln!(self.out, "  %v{v} = add i64 0, 0").unwrap();
                    }
                    Instruction::Const(Constant::String(s)) => {
                        let idx = self.string_const_index(s).unwrap_or(0);
                        let len = s.as_bytes().len();
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_string_new(ptr @.str.{idx}, i64 {len})"
                        ).unwrap();
                    }
                    Instruction::Add(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = add i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Sub(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = sub i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Mul(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = mul i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SDiv(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = sdiv i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SRem(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = srem i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Shl(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = shl i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::AShr(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = ashr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::And(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = and i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Or(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = or i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Xor(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Neg(a) => {
                        let sa = self.operand_str(a);
                        writeln!(self.out, "  %v{v} = sub i64 0, {sa}").unwrap();
                    }
                    Instruction::Not(a) => {
                        let sa = self.operand_str(a);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, -1").unwrap();
                    }
                    Instruction::FAdd(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = fadd double {sa}, {sb}").unwrap();
                    }
                    Instruction::FSub(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = fsub double {sa}, {sb}").unwrap();
                    }
                    Instruction::FMul(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = fmul double {sa}, {sb}").unwrap();
                    }
                    Instruction::FDiv(a, b) => {
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = fdiv double {sa}, {sb}").unwrap();
                    }
                    Instruction::FNeg(a) => {
                        let sa = self.operand_str(a);
                        writeln!(self.out, "  %v{v} = fneg double {sa}").unwrap();
                    }
                    Instruction::ICmp(op, a, b) => {
                        let op_str = match op {
                            ICmpOp::Eq => "eq",
                            ICmpOp::Ne => "ne",
                            ICmpOp::Slt => "slt",
                            ICmpOp::Sle => "sle",
                            ICmpOp::Sgt => "sgt",
                            ICmpOp::Sge => "sge",
                        };
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = icmp {op_str} i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::FCmp(op, a, b) => {
                        let op_str = match op {
                            FCmpOp::Eq => "oeq",
                            FCmpOp::Ne => "one",
                            FCmpOp::Lt => "olt",
                            FCmpOp::Le => "ole",
                            FCmpOp::Gt => "ogt",
                            FCmpOp::Ge => "oge",
                        };
                        let sa = self.operand_str(a);
                        let sb = self.operand_str(b);
                        writeln!(self.out, "  %v{v} = fcmp {op_str} double {sa}, {sb}").unwrap();
                    }
                    Instruction::LNot(a) => {
                        let sa = self.operand_str(a);
                        writeln!(self.out, "  %v{v} = xor i1 {sa}, true").unwrap();
                    }
                    Instruction::IntToDouble(a) => {
                        let sa = self.operand_str(a);
                        writeln!(self.out, "  %v{v} = sitofp i64 {sa} to double").unwrap();
                    }
                    Instruction::DoubleToInt(a) => {
                        let sa = self.operand_str(a);
                        writeln!(self.out, "  %v{v} = fptosi double {sa} to i64").unwrap();
                    }
                    Instruction::ZExt { op, from, to } => {
                        let sop = self.operand_str(op);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = zext {f} {sop} to {t}").unwrap();
                    }
                    Instruction::Trunc { op, from, to } => {
                        let sop = self.operand_str(op);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = trunc {f} {sop} to {t}").unwrap();
                    }
                    Instruction::AllocObject { class_id, fields } => {
                        let count = fields.len();
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_new(i64 {class_id}, i64 {count})"
                        ).unwrap();
                        for (idx, field) in fields.iter().enumerate() {
                            let sf = self.operand_str(field);
                            // tag de ref: 0 por enquanto
                            writeln!(
                                self.out,
                                "  call void @dartforge_object_set(i64 %v{v}, i64 {idx}, i64 {sf}, i8 0)"
                            ).unwrap();
                        }
                    }
                    Instruction::GetField { object, index } => {
                        let so = self.operand_str(object);
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_get(i64 {so}, i64 {index})"
                        ).unwrap();
                    }
                    Instruction::SetField { object, index, value } => {
                        let so = self.operand_str(object);
                        let sv = self.operand_str(value);
                        writeln!(
                            self.out,
                            "  call void @dartforge_object_set(i64 {so}, i64 {index}, i64 {sv}, i8 0)"
                        ).unwrap();
                    }
                    Instruction::CallStatic { symbol, args, ret_ty } => {
                        let target_func = self.module.functions.iter().find(|f| f.symbol == *symbol);
                        let args_str: Vec<String> = args
                            .iter()
                            .enumerate()
                            .map(|(idx, a)| {
                                let s = self.operand_str(a);
                                let t = target_func
                                    .and_then(|f| f.params.get(idx))
                                    .map(|p| p.2.llvm_ir())
                                    .unwrap_or("i64");
                                format!("{t} {s}")
                            })
                            .collect();
                        let joined = args_str.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{symbol}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{symbol}({joined})").unwrap();
                        }
                    }
                    Instruction::CallRuntime { name, args, ret_ty } => {
                        let mut args_formatted = Vec::new();
                        for (a, ty) in args {
                            let s = self.operand_str(a);
                            let t = ty.llvm_ir();
                            args_formatted.push(format!("{t} {s}"));
                        }
                        let joined = args_formatted.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{name}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{name}({joined})").unwrap();
                        }
                    }
                    Instruction::AllocList { elements } => {
                        // Alloca temporário para pares (bits, tag)
                        let count = elements.len();
                        let alloca_id = format!("list_buf_{v}");
                        writeln!(self.out, "  %{alloca_id} = alloca [{} x i64]", count * 2).unwrap();
                        for (idx, (elem, tag)) in elements.iter().enumerate() {
                            let se = self.operand_str(elem);
                            let off_bits = idx * 2;
                            let off_tag = idx * 2 + 1;
                            writeln!(self.out, "  %ptr_{v}_{off_bits} = getelementptr [{} x i64], ptr %{alloca_id}, i64 0, i64 {off_bits}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {se}, ptr %ptr_{v}_{off_bits}").unwrap();
                            writeln!(self.out, "  %ptr_{v}_{off_tag} = getelementptr [{} x i64], ptr %{alloca_id}, i64 0, i64 {off_tag}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {tag}, ptr %ptr_{v}_{off_tag}").unwrap();
                        }
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_list_new(ptr %{alloca_id}, i64 {count})"
                        ).unwrap();
                    }
                    Instruction::AllocMap { entries } => {
                        let count = entries.len();
                        let k_buf = format!("map_k_{v}");
                        let v_buf = format!("map_v_{v}");
                        writeln!(self.out, "  %{k_buf} = alloca [{} x i64]", count * 2).unwrap();
                        writeln!(self.out, "  %{v_buf} = alloca [{} x i64]", count * 2).unwrap();
                        for (idx, ((k, k_tag), (val, v_tag))) in entries.iter().enumerate() {
                            let sk = self.operand_str(k);
                            let sv = self.operand_str(val);
                            let off_bits = idx * 2;
                            let off_tag = idx * 2 + 1;
                            writeln!(self.out, "  %kptr_{v}_{off_bits} = getelementptr [{} x i64], ptr %{k_buf}, i64 0, i64 {off_bits}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {sk}, ptr %kptr_{v}_{off_bits}").unwrap();
                            writeln!(self.out, "  %kptr_{v}_{off_tag} = getelementptr [{} x i64], ptr %{k_buf}, i64 0, i64 {off_tag}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {k_tag}, ptr %kptr_{v}_{off_tag}").unwrap();

                            writeln!(self.out, "  %vptr_{v}_{off_bits} = getelementptr [{} x i64], ptr %{v_buf}, i64 0, i64 {off_bits}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {sv}, ptr %vptr_{v}_{off_bits}").unwrap();
                            writeln!(self.out, "  %vptr_{v}_{off_tag} = getelementptr [{} x i64], ptr %{v_buf}, i64 0, i64 {off_tag}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {v_tag}, ptr %vptr_{v}_{off_tag}").unwrap();
                        }
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_map_new(ptr %{k_buf}, ptr %{v_buf}, i64 {count})"
                        ).unwrap();
                    }
                    Instruction::AllocRecord { elements } => {
                        let count = elements.len();
                        let total_i64 = count * 2;
                        let buf_name = format!("rec_buf_{v}");
                        writeln!(self.out, "  %{buf_name} = alloca [{total_i64} x i64]").unwrap();
                        for (i, (elem, tag)) in elements.iter().enumerate() {
                            let sop = self.operand_str(elem);
                            let ptr_bits = format!("ptr_rec_{v}_{i}_bits");
                            let ptr_tag = format!("ptr_rec_{v}_{i}_tag");
                            let off_bits = i * 2;
                            let off_tag = i * 2 + 1;
                            writeln!(self.out, "  %{ptr_bits} = getelementptr [{total_i64} x i64], ptr %{buf_name}, i64 0, i64 {off_bits}").unwrap();
                            writeln!(self.out, "  store i64 {sop}, ptr %{ptr_bits}").unwrap();
                            writeln!(self.out, "  %{ptr_tag} = getelementptr [{total_i64} x i64], ptr %{buf_name}, i64 0, i64 {off_tag}").unwrap();
                            writeln!(self.out, "  store i64 {tag}, ptr %{ptr_tag}").unwrap();
                        }
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_record_new(ptr %{buf_name}, i64 {count})"
                        ).unwrap();
                    }
                    Instruction::Alloca(ty) => {
                        writeln!(self.out, "  %v{v} = alloca {}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::Load { ptr, ty } => {
                        let sp = self.operand_str(ptr);
                        writeln!(self.out, "  %v{v} = load {}, ptr {sp}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::Store { ptr, val } => {
                        let sp = self.operand_str(ptr);
                        let sv = self.operand_str(val);
                        writeln!(self.out, "  store i64 {sv}, ptr {sp}").unwrap();
                    }
                    Instruction::Phi { incoming, ty } => {
                        let t = ty.llvm_ir();
                        let in_strs: Vec<String> = incoming
                            .iter()
                            .map(|(b, op)| {
                                let sop = self.operand_str(op);
                                format!("[ {sop}, %b{} ]", b.0)
                            })
                            .collect();
                        let joined = in_strs.join(", ");
                        writeln!(self.out, "  %v{v} = phi {t} {joined}").unwrap();
                    }
                    _ => {
                        // Outras instruções serão expandidas conforme necessário
                        writeln!(self.out, "  ; inst pendente {:?}", inst).unwrap();
                    }
                }
            }

            match &block.terminator {
                Terminator::Return(Some(op)) => {
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        let sop = self.operand_str(op);
                        writeln!(self.out, "  ret {} {sop}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Return(None) => {
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        writeln!(self.out, "  ret {} 0", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Branch(target) => {
                    writeln!(self.out, "  br label %b{}", target.0).unwrap();
                }
                Terminator::CondBranch { cond, then_block, else_block } => {
                    let sc = self.operand_str(cond);
                    writeln!(self.out, "  br i1 {sc}, label %b{}, label %b{}", then_block.0, else_block.0).unwrap();
                }
                Terminator::Switch { val, default, cases } => {
                    let sv = self.operand_str(val);
                    write!(self.out, "  switch i64 {sv}, label %b{} [", default.0).unwrap();
                    for (c, b) in cases {
                        write!(self.out, " i64 {c}, label %b{}", b.0).unwrap();
                    }
                    writeln!(self.out, " ]").unwrap();
                }
                Terminator::Throw(op) => {
                    let sop = self.operand_str(op);
                    writeln!(self.out, "  call void @dartforge_exception_throw(i64 {sop}, i8 3)").unwrap();
                    writeln!(self.out, "  unreachable").unwrap();
                }
                Terminator::Unreachable => {
                    writeln!(self.out, "  unreachable").unwrap();
                }
            }
        }

        writeln!(self.out, "}}\n").unwrap();
    }

    fn emit_dispatch_functions(&mut self) {
        self.out.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n");
        self.out.push_str("b0:\n");
        self.out.push_str("  %is_null = icmp eq i64 %obj, 0\n");
        self.out.push_str("  br i1 %is_null, label %ret_null, label %check_obj\n");
        self.out.push_str("ret_null:\n");
        self.out.push_str("  %null_s = call i64 @dartforge_to_string_handle(i64 0)\n");
        self.out.push_str("  ret i64 %null_s\n");
        self.out.push_str("check_obj:\n");
        self.out.push_str("  %cls = call i64 @dartforge_value_class(i64 %obj)\n");

        let mut cases = Vec::new();
        for class in &self.module.classes {
            if let Some(sym) = &class.to_string_symbol {
                cases.push((class.id, sym.clone()));
            }
        }

        if cases.is_empty() {
            self.out.push_str("  br label %fallback\n");
        } else {
            write!(self.out, "  switch i64 %cls, label %fallback [").unwrap();
            for (cid, _) in &cases {
                write!(self.out, " i64 {cid}, label %case_{cid}").unwrap();
            }
            writeln!(self.out, " ]").unwrap();
            for (cid, sym) in &cases {
                writeln!(self.out, "case_{cid}:").unwrap();
                writeln!(self.out, "  %res_{cid} = call i64 @{sym}(i64 %obj)").unwrap();
                writeln!(self.out, "  ret i64 %res_{cid}").unwrap();
            }
        }
        self.out.push_str("fallback:\n");
        self.out.push_str("  %fb = call i64 @dartforge_to_string_handle(i64 %obj)\n");
        self.out.push_str("  ret i64 %fb\n");
        self.out.push_str("}\n\n");
    }

    fn emit_entry(&mut self) {
        writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
        // Registra classes
        for class in &self.module.classes {
            let idx = self.string_const_index(&class.name).unwrap_or(0);
            let len = class.name.as_bytes().len();
            writeln!(
                self.out,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{}, i64 {})",
                class.id, idx, len
            ).unwrap();
        }

        // Registra grafo de subtipagem
        for (sub, sup) in &self.module.subtyping_edges {
            writeln!(
                self.out,
                "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})"
            ).unwrap();
        }

        if let Some(entry) = &self.module.entry_symbol {
            writeln!(self.out, "  call void @{entry}()").unwrap();
        }
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}\n").unwrap();
    }

    fn operand_str(&self, op: &Operand) -> String {
        match op {
            Operand::Val(v) => format!("%v{}", v.0),
            Operand::Constant(Constant::Int(n)) => n.to_string(),
            Operand::Constant(Constant::Double(d)) => {
                let _bits = d.to_bits();
                // Em instrução com double imediato, LLVM aceita ponto ou notação científica
                format!("{d}")
            }
            Operand::Constant(Constant::Bool(b)) => if *b { "true" } else { "false" }.to_string(),
            Operand::Constant(Constant::Null) => "0".to_string(),
            Operand::Constant(Constant::String(_)) => {
                panic!("string deve ser carregada via Instruction::Const");
            }
        }
    }
}


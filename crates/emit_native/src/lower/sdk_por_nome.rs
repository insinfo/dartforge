//! Membros do SDK casados **pelo nome**, sem olhar o tipo do receptor.
//!
//! É o runtime escrito à mão do backend nativo: `length`, `add`,
//! `substring`… viram externs do runtime pelo nome do membro, e
//! `Exception(…)`, `StringBuffer()`… pelo nome da classe. **Congelado**
//! (docs/NATIVO-PLANO.md, rodada 2, decisão 1): nenhum caso novo entra
//! aqui. O arquivo inteiro morre em P5, quando o SDK vem da fonte e toda
//! chamada a membro do SDK é baixada pelo elemento resolvido.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_frontend::ast::{self, ExprId, ExprKind, FunctionBody};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `StackTrace.current` e `StackTrace.empty`; `None` para o resto.
    pub(super) fn propriedade_estatica_sdk_por_nome(
        &mut self,
        ast: &ast::Ast,
        target: &ExprId,
        prop_name: &str,
    ) -> Option<Operand> {
        if self.ctx.sdk_da_fonte {
            return None;
        }
        if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
            if self.ctx.symbol_name(id.sym) == "StackTrace" {
                if prop_name == "current" {
                    return Some(self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_stack_trace_get".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ));
                } else if prop_name == "empty" {
                    return Some(self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_stack_trace_empty".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ));
                }
            }
        }
        None
    }

    /// Propriedade de valor do SDK (`length`, `isEmpty`, `first`, `message`…),
    /// ou o diagnóstico `membro`.
    pub(super) fn propriedade_sdk_por_nome(
        &mut self,
        target_op: Operand,
        prop_name: &str,
        expr_id: ExprId,
        span: dartforge_diagnostics::Span,
    ) -> Operand {
        if self.ctx.sdk_da_fonte {
            // SDK da fonte (P5c): o membro pela classe dinâmica.
            return self.chamar_por_nome(target_op, super::sdk_fonte::Tipo::Ler, prop_name, &[]);
        }
        if prop_name == "length" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_generic_len".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            )
        } else if prop_name == "isEmpty" {
            let len_op = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_generic_len".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            self.emit(
                Instruction::ICmp(ICmpOp::Eq, len_op, Operand::Constant(Constant::Int(0))),
                Type::I1,
            )
        } else if prop_name == "isNotEmpty" {
            let len_op = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_generic_len".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            self.emit(
                Instruction::ICmp(ICmpOp::Ne, len_op, Operand::Constant(Constant::Int(0))),
                Type::I1,
            )
        } else if prop_name == "last" {
            self.ler_extremo_lista(target_op, "last", expr_id)
        } else if prop_name == "first" {
            self.ler_extremo_lista(target_op, "first", expr_id)
        } else if prop_name == "single" {
            self.ler_extremo_lista(target_op, "single", expr_id)
        } else if prop_name == "codeUnits" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_code_units".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "runes" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_runes".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "reversed" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_list_reversed".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "isOdd" {
            // O receptor é `int`: um `Ref` (parâmetro de função local,
            // `int?` promovido) é a CAIXA — a paridade do handle não é
            // a do número (R3).
            let target_op = self.coagir(target_op, Type::I64);
            let rem = self.emit(
                Instruction::And(target_op, Operand::Constant(Constant::Int(1))),
                Type::I64,
            );
            self.emit(
                Instruction::ICmp(ICmpOp::Ne, rem, Operand::Constant(Constant::Int(0))),
                Type::I1,
            )
        } else if prop_name == "isEven" {
            // O receptor é `int`: um `Ref` (parâmetro de função local,
            // `int?` promovido) é a CAIXA — a paridade do handle não é
            // a do número (R3).
            let target_op = self.coagir(target_op, Type::I64);
            let rem = self.emit(
                Instruction::And(target_op, Operand::Constant(Constant::Int(1))),
                Type::I64,
            );
            self.emit(
                Instruction::ICmp(ICmpOp::Eq, rem, Operand::Constant(Constant::Int(0))),
                Type::I1,
            )
        } else if prop_name == "message" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_message".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "name" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_name".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "invalidValue" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_invalid_value".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "start" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_start".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            )
        } else if prop_name == "end" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_end".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            )
        } else if prop_name == "source" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_source".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "offset" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_offset".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            )
        } else if prop_name == "stackTrace" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_error_get_stack_trace".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "runtimeType" {
            // RTI: o objeto `Type` canônico do tipo do valor (o mesmo caso de
            // antes, agora com o tipo inteiro em vez do id da classe).
            let v = self.coagir(target_op, Type::Ref);
            let t = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_do_valor".to_string(),
                    args: vec![(v, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_objeto_tipo".to_string(),
                    args: vec![(t, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else if prop_name == "keys" {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_map_keys".to_string(),
                    args: vec![(target_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            )
        } else {
            self.nao_suportado(&format!("membro `{prop_name}`"), span)
        }
    }

    /// `print(x)` e os construtores do SDK pelo nome da classe (`Object()`,
    /// `StringBuffer()`, `RegExp(p)`, `Exception(m)`…) e `identical`; `None`
    /// quando o alvo não é nenhum deles.
    pub(super) fn funcao_sdk_por_nome(
        &mut self,
        ast: &ast::Ast,
        target: &ExprId,
        arguments: &ast::Arguments,
    ) -> Option<Operand> {
        if self.ctx.sdk_da_fonte {
            return None;
        }
        if let ExprKind::Identifier(name) = &ast.expr(*target).kind {
            if self.ctx.symbol_name(name.sym) == "print" {
                if let Some(first_arg) = arguments.args.first() {
                    let arg_op = self.lower_expr(ast, first_arg.value);
                    // R5: a representação do operando É o tipo. Um
                    // `Ref` é impresso pelo `toString` dele (o do
                    // usuário, pelo despacho; o do runtime para
                    // coleções, strings, caixas e null).
                    return Some(match self.operand_type(&arg_op) {
                        Type::F64 => self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_print_f64".to_string(),
                                args: vec![(arg_op, Type::F64)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                        Type::I64 => self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_print_i64".to_string(),
                                args: vec![(arg_op, Type::I64)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                        Type::I1 | Type::I8 => self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_print_bool".to_string(),
                                args: vec![(arg_op, Type::I8)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                        _ => {
                            let arg_op = self.coagir(arg_op, Type::Ref);
                            let texto = self.emit_call_with_check(
                                Instruction::CallStatic {
                                    symbol: "dartforge_dispatch_toString".to_string(),
                                    args: vec![arg_op],
                                    ret_ty: Type::Ref,
                                },
                                Type::Ref,
                            );
                            self.emit(
                                Instruction::CallRuntime {
                                    name: "dartforge_print_handle".to_string(),
                                    args: vec![(texto, Type::Ref)],
                                    ret_ty: Type::Void,
                                },
                                Type::Void,
                            )
                        }
                    });
                }
            }
        }

        if let ExprKind::Identifier(id) = &ast.expr(*target).kind {
            let id_str = self.ctx.symbol_name(id.sym);
            if id_str == "Object" {
                return Some(self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_object_new".to_string(),
                        args: vec![
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                            (Operand::Constant(Constant::Int(0)), Type::I64),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
            } else if id_str == "StringBuffer" {
                return Some(self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_string_buffer_new".to_string(),
                        args: Vec::new(),
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
            } else if id_str == "RegExp" {
                let pat_op = self.lower_expr(ast, arguments.args[0].value);
                return Some(self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_regexp_new".to_string(),
                        args: vec![(pat_op, Type::Ref)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                ));
            } else if id_str == "identical" && arguments.args.len() == 2 {
                let a_op = self.lower_expr(ast, arguments.args[0].value);
                let b_op = self.lower_expr(ast, arguments.args[1].value);
                return Some(self.identicos(a_op, b_op));
            } else if let Some(err_cid) = match id_str {
                "Exception" => Some(1000),
                "FormatException" => Some(1001),
                "StateError" => Some(1002),
                "ArgumentError" => Some(1003),
                "RangeError" => Some(1004),
                "UnsupportedError" => Some(1005),
                "UnimplementedError" => Some(1008),
                _ => None,
            } {
                let msg_op = if let Some(first_arg) = arguments.args.first() {
                    self.lower_expr(ast, first_arg.value)
                } else {
                    self.emit(
                        Instruction::Const(Constant::String("".to_string())),
                        Type::Ref,
                    )
                };
                return Some(self.emit(
                    Instruction::AllocObject {
                        class_id: err_cid,
                        fields: vec![msg_op],
                    },
                    Type::Ref,
                ));
            }
        }
        None
    }

    /// `String.fromCharCode(c)` e `String.fromCharCodes(l)`; `None` para o
    /// resto.
    pub(super) fn estatica_sdk_por_nome(
        &mut self,
        ast: &ast::Ast,
        target: &ExprId,
        arguments: &ast::Arguments,
    ) -> Option<Operand> {
        if self.ctx.sdk_da_fonte {
            return None;
        }
        // Verifica métodos estáticos/construtores nomeados de String (String.fromCharCode, String.fromCharCodes)
        if let ExprKind::Property {
            target: inner_target,
            name: method_name,
            ..
        } = &ast.expr(*target).kind
        {
            let m_name = self.ctx.symbol_name(method_name.sym);
            if let ExprKind::Identifier(id) = &ast.expr(*inner_target).kind {
                let id_str = self.ctx.symbol_name(id.sym);
                if id_str == "String" {
                    if m_name == "fromCharCode" {
                        let code_op = self.lower_expr(ast, arguments.args[0].value);
                        let code_op = self.coagir(code_op, Type::I64);
                        return Some(self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_from_char_code".to_string(),
                                args: vec![(code_op, Type::I64)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ));
                    } else if m_name == "fromCharCodes" {
                        let list_op = self.lower_expr(ast, arguments.args[0].value);
                        return Some(self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_string_from_char_codes".to_string(),
                                args: vec![(list_op, Type::Ref)],
                                ret_ty: Type::Ref,
                            },
                            Type::Ref,
                        ));
                    }
                }
            }
        }
        None
    }

    /// Método do SDK sobre um receptor que não é classe do usuário
    /// (`add`, `substring`, `join`, `map` com closure literal…); o que não
    /// casa vira o diagnóstico de chamada.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn metodo_sdk_por_nome(
        &mut self,
        ast: &ast::Ast,
        expr: &ast::Expr,
        target: &ExprId,
        inner_target: &ExprId,
        m_name: &str,
        recv_op: Operand,
        arguments: &ast::Arguments,
    ) -> Operand {
        if self.ctx.sdk_da_fonte {
            // SDK da fonte (P5c): o membro pela classe dinâmica.
            let _ = (expr, target, inner_target);
            let av = self.avaliar_args(ast, &arguments.args);
            // Argumentos de tipo escritos (`d.m<int>(…)`): a tupla vai no
            // slot da chamada por seletor (o método genérico, ou o
            // `Invocation.typeArguments` de um `noSuchMethod`).
            if !arguments.type_args.is_empty() {
                let args = self.receitas_dos_argumentos_de_tipo(&arguments.type_args);
                let tupla = self.rti_da_receita(&super::rti::Receita {
                    texto: format!("L<{}>", args.texto),
                    variaveis: args.variaveis,
                });
                let lib = self.ctx.program.unit(self.unit_id).library;
                let s = super::sdk_fonte::texto_seletor(self.ctx, super::sdk_fonte::Tipo::Chamar, m_name, lib);
                return self.chamar_por_seletor_com_tupla(recv_op, s, &av, tupla);
            }
            return self.chamar_por_nome(recv_op, super::sdk_fonte::Tipo::Chamar, m_name, &av);
        }
        if m_name == "add" {
            if let Some(first_arg) = arguments.args.first() {
                let val_op = self.lower_expr(ast, first_arg.value);
                let tag = self.operand_tag(&val_op);
                return self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_list_push".to_string(),
                        args: vec![
                            (recv_op, Type::Ref),
                            (val_op, Type::I64),
                            (Operand::Constant(Constant::Int(tag as i64)), Type::I8),
                        ],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
            }
        } else if m_name == "toUpperCase" {
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_to_upper".to_string(),
                    args: vec![(recv_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "codeUnitAt" {
            let idx_op = if let Some(first_arg) = arguments.args.first() {
                self.lower_expr(ast, first_arg.value)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_code_unit_at".to_string(),
                    args: vec![(recv_op, Type::Ref), (idx_op, Type::I64)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
        } else if m_name == "toRadixString" {
            let recv_op = self.coagir(recv_op, Type::I64);
            let radix_op = if let Some(first_arg) = arguments.args.first() {
                self.lower_expr(ast, first_arg.value)
            } else {
                Operand::Constant(Constant::Int(10))
            };
            let radix_op = self.coagir(radix_op, Type::I64);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_int_to_radix_string".to_string(),
                    args: vec![(recv_op, Type::I64), (radix_op, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "substring" {
            let start_op = if let Some(first_arg) = arguments.args.first() {
                self.lower_expr(ast, first_arg.value)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            let end_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                Operand::Constant(Constant::Int(-1))
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_substring".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (start_op, Type::I64),
                        (end_op, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "indexOf" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            let start_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_index_of".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (pat_op, Type::Ref),
                        (start_op, Type::I64),
                    ],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
        } else if m_name == "lastIndexOf" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            let start_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                Operand::Constant(Constant::Int(-1))
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_last_index_of".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (pat_op, Type::Ref),
                        (start_op, Type::I64),
                    ],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
        } else if m_name == "split" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_split".to_string(),
                    args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "contains" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            let start_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            let c_i8 = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_contains".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (pat_op, Type::Ref),
                        (start_op, Type::I64),
                    ],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            return self.emit(
                Instruction::Trunc {
                    op: c_i8,
                    from: Type::I8,
                    to: Type::I1,
                },
                Type::I1,
            );
        } else if m_name == "replaceAll" {
            let from_op = self.lower_expr(ast, arguments.args[0].value);
            let to_op = self.lower_expr(ast, arguments.args[1].value);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_replace_all".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (from_op, Type::Ref),
                        (to_op, Type::Ref),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "padLeft" {
            let width_op = self.lower_expr(ast, arguments.args[0].value);
            let pad_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                self.emit(
                    Instruction::Const(Constant::String(" ".to_string())),
                    Type::Ref,
                )
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_pad_left".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (width_op, Type::I64),
                        (pad_op, Type::Ref),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "padRight" {
            let width_op = self.lower_expr(ast, arguments.args[0].value);
            let pad_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                self.emit(
                    Instruction::Const(Constant::String(" ".to_string())),
                    Type::Ref,
                )
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_pad_right".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (width_op, Type::I64),
                        (pad_op, Type::Ref),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "startsWith" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            let start_op = if arguments.args.len() > 1 {
                self.lower_expr(ast, arguments.args[1].value)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            let c_i8 = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_starts_with".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (pat_op, Type::Ref),
                        (start_op, Type::I64),
                    ],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            return self.emit(
                Instruction::Trunc {
                    op: c_i8,
                    from: Type::I8,
                    to: Type::I1,
                },
                Type::I1,
            );
        } else if m_name == "endsWith" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            let c_i8 = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_ends_with".to_string(),
                    args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            return self.emit(
                Instruction::Trunc {
                    op: c_i8,
                    from: Type::I8,
                    to: Type::I1,
                },
                Type::I1,
            );
        } else if m_name == "trim" {
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_trim".to_string(),
                    args: vec![(recv_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "trimLeft" {
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_trim_left".to_string(),
                    args: vec![(recv_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "trimRight" {
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_trim_right".to_string(),
                    args: vec![(recv_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "toLowerCase" {
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_to_lower".to_string(),
                    args: vec![(recv_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "compareTo" {
            let other_op = self.lower_expr(ast, arguments.args[0].value);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_compare_to".to_string(),
                    args: vec![(recv_op, Type::Ref), (other_op, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
        } else if m_name == "replaceFirst" {
            let from_op = self.lower_expr(ast, arguments.args[0].value);
            let to_op = self.lower_expr(ast, arguments.args[1].value);
            let start_op = if arguments.args.len() > 2 {
                self.lower_expr(ast, arguments.args[2].value)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_replace_first".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (from_op, Type::Ref),
                        (to_op, Type::Ref),
                        (start_op, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "replaceRange" {
            let start_op = self.lower_expr(ast, arguments.args[0].value);
            let end_op = self.lower_expr(ast, arguments.args[1].value);
            let rep_op = self.lower_expr(ast, arguments.args[2].value);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_replace_range".to_string(),
                    args: vec![
                        (recv_op, Type::Ref),
                        (start_op, Type::I64),
                        (end_op, Type::I64),
                        (rep_op, Type::Ref),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "write" {
            let arg_op = self.lower_expr(ast, arguments.args[0].value);
            let str_op = self.texto_de(arg_op);
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_buffer_write".to_string(),
                    args: vec![(recv_op, Type::Ref), (str_op, Type::Ref)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        } else if m_name == "toString" {
            return self.texto_de(recv_op);
        } else if m_name == "join" {
            let sep_op = if let Some(first_arg) = arguments.args.first() {
                self.lower_expr(ast, first_arg.value)
            } else {
                self.emit(
                    Instruction::Const(Constant::String("".to_string())),
                    Type::Ref,
                )
            };
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_list_join".to_string(),
                    args: vec![(recv_op, Type::Ref), (sep_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "toList" {
            return recv_op;
        } else if m_name == "map" {
            if let Some(first_arg) = arguments.args.first() {
                if let ExprKind::FunctionExpression(func_id) = ast.expr(first_arg.value).kind {
                    let closure =
                        &self.ctx.program.unit(self.unit_id).ast.functions[func_id.0 as usize];
                    let param_sym = closure
                        .parameters
                        .as_ref()
                        .and_then(|p| p.first())
                        .and_then(|p| p.name.as_ref())
                        .map(|n| n.sym);

                    let len_op = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_len".to_string(),
                            args: vec![(recv_op.clone(), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    );
                    let res_list = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_new_empty".to_string(),
                            args: Vec::new(),
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    );

                    let pre_block = self.current_block;
                    let loop_header = self.new_block();
                    let loop_body = self.new_block();
                    let exit_block = self.new_block();

                    self.terminate(Terminator::Branch(loop_header));
                    self.set_block(loop_header);

                    let phi_vid = ValueId(self.next_value);
                    self.next_value += 1;
                    self.value_types.insert(phi_vid, Type::I64);
                    let phi_op = Operand::Val(phi_vid);

                    let header_idx = self
                        .func
                        .blocks
                        .iter()
                        .position(|b| b.id == loop_header)
                        .unwrap();
                    let phi_inst_idx = self.func.blocks[header_idx].instructions.len();
                    self.func.blocks[header_idx].instructions.push((
                        phi_vid,
                        Instruction::Phi {
                            incoming: Vec::new(),
                            ty: Type::I64,
                        },
                        Type::I64,
                    ));

                    let cmp = self.emit(
                        Instruction::ICmp(ICmpOp::Slt, phi_op.clone(), len_op),
                        Type::I1,
                    );
                    self.terminate(Terminator::CondBranch {
                        cond: cmp,
                        then_block: loop_body,
                        else_block: exit_block,
                    });

                    self.set_block(loop_body);
                    // O elemento na representação do tipo de
                    // elemento da lista (R5).
                    let repr_item = self
                        .ctx
                        .get_type(self.unit_id, *inner_target)
                        .and_then(|t| self.tipo_elemento(t))
                        .map_or(Type::Ref, |t| self.repr(t));
                    let item_val =
                        self.ler_elemento_lista(recv_op.clone(), phi_op.clone(), repr_item);

                    if let Some(sym) = param_sym {
                        self.ligar_local(sym, item_val);
                    }

                    let mapped_val = match &closure.body {
                        FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                        _ => self.nao_suportado("closure com corpo de bloco", expr.span),
                    };

                    let mapped_tag = i64::from(self.operand_tag(&mapped_val));
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_list_push".to_string(),
                            args: vec![
                                (res_list.clone(), Type::Ref),
                                (mapped_val, Type::I64),
                                (Operand::Constant(Constant::Int(mapped_tag)), Type::I8),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    );

                    let next_idx = self.emit(
                        Instruction::Add(phi_op, Operand::Constant(Constant::Int(1))),
                        Type::I64,
                    );
                    let body_end_block = self.current_block;
                    self.terminate(Terminator::Branch(loop_header));

                    let header_idx = self
                        .func
                        .blocks
                        .iter()
                        .position(|b| b.id == loop_header)
                        .unwrap();
                    self.func.blocks[header_idx].instructions[phi_inst_idx].1 = Instruction::Phi {
                        incoming: vec![
                            (pre_block, Operand::Constant(Constant::Int(0))),
                            (body_end_block, next_idx),
                        ],
                        ty: Type::I64,
                    };

                    self.set_block(exit_block);
                    return res_list;
                }
            }
        } else if m_name == "characters" {
            let empty_str = self.emit(
                Instruction::Const(Constant::String("".to_string())),
                Type::Ref,
            );
            return self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_split".to_string(),
                    args: vec![(recv_op, Type::Ref), (empty_str, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        } else if m_name == "splitMapJoin" {
            let pat_op = self.lower_expr(ast, arguments.args[0].value);
            let on_match_arg = arguments.args.iter().find(|a| {
                a.name
                    .map_or(false, |n| self.ctx.symbol_name(n.sym) == "onMatch")
            });
            let on_non_match_arg = arguments.args.iter().find(|a| {
                a.name
                    .map_or(false, |n| self.ctx.symbol_name(n.sym) == "onNonMatch")
            });

            let pieces_list = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_split_map_pieces".to_string(),
                    args: vec![(recv_op, Type::Ref), (pat_op, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );

            let res_buf = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_buffer_new".to_string(),
                    args: Vec::new(),
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );

            let total_len = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_list_len".to_string(),
                    args: vec![(pieces_list.clone(), Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );

            let loop_header = self.new_block();
            let loop_body = self.new_block();
            let loop_update = self.new_block();
            let exit_block = self.new_block();

            let entry_block = self.current_block;
            self.terminate(Terminator::Branch(loop_header));

            self.set_block(loop_header);
            let phi_vid = ValueId(self.next_value);
            self.next_value += 1;
            let phi_op = Operand::Val(phi_vid);
            self.value_types.insert(phi_vid, Type::I64);

            let header_idx = self
                .func
                .blocks
                .iter()
                .position(|b| b.id == self.current_block)
                .unwrap();
            let phi_inst_idx = self.func.blocks[header_idx].instructions.len();
            self.func.blocks[header_idx].instructions.push((
                phi_vid,
                Instruction::Phi {
                    incoming: Vec::new(),
                    ty: Type::I64,
                },
                Type::I64,
            ));

            let cmp = self.emit(
                Instruction::ICmp(ICmpOp::Slt, phi_op.clone(), total_len),
                Type::I1,
            );
            self.terminate(Terminator::CondBranch {
                cond: cmp,
                then_block: loop_body,
                else_block: exit_block,
            });

            self.set_block(loop_body);
            let is_match_bits = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_list_get_bits".to_string(),
                    args: vec![
                        (pieces_list.clone(), Type::Ref),
                        (phi_op.clone(), Type::I64),
                    ],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            let is_match_cond = self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    is_match_bits,
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I1,
            );

            let part_idx = self.emit(
                Instruction::Add(phi_op.clone(), Operand::Constant(Constant::Int(1))),
                Type::I64,
            );
            let part_val = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_list_get_bits".to_string(),
                    args: vec![(pieces_list.clone(), Type::Ref), (part_idx, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );

            let match_block = self.new_block();
            let non_match_block = self.new_block();
            let write_block = self.new_block();

            self.terminate(Terminator::CondBranch {
                cond: is_match_cond,
                then_block: match_block,
                else_block: non_match_block,
            });

            // Match block
            self.set_block(match_block);
            let match_res = if let Some(arg) = on_match_arg {
                if let ExprKind::FunctionExpression(func_id) = ast.expr(arg.value).kind {
                    let closure =
                        &self.ctx.program.unit(self.unit_id).ast.functions[func_id.0 as usize];
                    let param_sym = closure
                        .parameters
                        .as_ref()
                        .and_then(|p| p.first())
                        .and_then(|p| p.name.as_ref())
                        .map(|n| n.sym);
                    if let Some(sym) = param_sym {
                        self.ligar_local(sym, part_val.clone());
                    }
                    match &closure.body {
                        FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                        _ => part_val.clone(),
                    }
                } else {
                    part_val.clone()
                }
            } else {
                part_val.clone()
            };
            let match_end_block = self.current_block;
            self.terminate(Terminator::Branch(write_block));

            // Non-match block
            self.set_block(non_match_block);
            let non_match_res = if let Some(arg) = on_non_match_arg {
                if let ExprKind::FunctionExpression(func_id) = ast.expr(arg.value).kind {
                    let closure =
                        &self.ctx.program.unit(self.unit_id).ast.functions[func_id.0 as usize];
                    let param_sym = closure
                        .parameters
                        .as_ref()
                        .and_then(|p| p.first())
                        .and_then(|p| p.name.as_ref())
                        .map(|n| n.sym);
                    if let Some(sym) = param_sym {
                        self.ligar_local(sym, part_val.clone());
                    }
                    match &closure.body {
                        FunctionBody::Expression(eid) => self.lower_expr(ast, *eid),
                        _ => part_val.clone(),
                    }
                } else {
                    part_val.clone()
                }
            } else {
                part_val.clone()
            };
            let non_match_end_block = self.current_block;
            self.terminate(Terminator::Branch(write_block));

            // Write block
            self.set_block(write_block);
            let piece_res = self.emit(
                Instruction::Phi {
                    incoming: vec![
                        (match_end_block, match_res),
                        (non_match_end_block, non_match_res),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            );
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_string_buffer_write".to_string(),
                    args: vec![(res_buf.clone(), Type::Ref), (piece_res, Type::Ref)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
            self.terminate(Terminator::Branch(loop_update));

            // Update block
            self.set_block(loop_update);
            let next_i = self.emit(
                Instruction::Add(phi_op.clone(), Operand::Constant(Constant::Int(2))),
                Type::I64,
            );
            let update_block_id = self.current_block;
            self.terminate(Terminator::Branch(loop_header));

            let header_idx = self
                .func
                .blocks
                .iter()
                .position(|b| b.id == loop_header)
                .unwrap();
            self.func.blocks[header_idx].instructions[phi_inst_idx].1 = Instruction::Phi {
                incoming: vec![
                    (entry_block, Operand::Constant(Constant::Int(0))),
                    (update_block_id, next_i),
                ],
                ty: Type::I64,
            };

            self.set_block(exit_block);
            let final_str = self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_to_string_handle".to_string(),
                    args: vec![(res_buf, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            return final_str;
        }
        self.chamada_nao_suportada(ast, target, expr)
    }
}

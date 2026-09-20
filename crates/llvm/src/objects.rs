//! Layout nominal, despacho e handles rastreados. Referências temporárias ficam
//! em slots por ponto estático de expressão e por local. Laços sobrescrevem os
//! slots de expressões; raízes separadas de locais preservam valores de iterações
//! anteriores. Cada ativação recursiva possui seu próprio frame. A quantidade de
//! slots por ativação depende da IR, não da quantidade de iterações. Slots não
//! são limpos na saída lexical: ainda pode haver retenção até sobrescrita/retorno.
//! O SDK 3.6.2 runtime/vm/compiler/backend/flow_graph_compiler.cc usa stack maps;
//! esta implementação conservadora não reproduz sua análise de vivacidade.
//! Handles não expõem endereços, zero representa null e campos de referência são
//! marcados precisamente. Int?/bool? ocupam presença e payload em slots separados.
//! Strings usam UTF-8 interno somente no subconjunto de escalares Unicode aceito
//! pelo frontend; isso não implementa indexação UTF-16 de Dart. Referência consultada:
//! SDK 3.6.2 sdk/lib/core/string.dart. Nenhum código do SDK foi copiado.
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Contrato C do runtime: handles e índices i64, flags i8 e bytes UTF-8.
pub(super) const DECLARATIONS: &str = "
declare i64 @dartforge_gc_push_frame(i64)
declare void @dartforge_gc_set_root(i64, i64, i64)
declare void @dartforge_gc_pop_frame(i64)
declare i64 @dartforge_object_new(i64, i64)
declare i64 @dartforge_object_get(i64, i64)
declare void @dartforge_object_set(i64, i64, i64, i8)
declare i64 @dartforge_object_class(i64)
declare i64 @dartforge_string_new(ptr, i64)
declare i64 @dartforge_string_concat(i64, i64)
declare i8 @dartforge_string_equal(i64, i64)
declare void @dartforge_print_string(i64)
";

#[derive(Clone)]
/// Tipo e posição física de um campo, incluindo campos herdados.
pub(super) struct Field {
    ty: Ty,
    offset: usize,
}
#[derive(Clone)]
/// Implementação concreta e assinatura escolhidas para um método nominal.
struct Method {
    signature: Signature,
    owner: u32,
    index: usize,
}
/// Layout completo de uma classe após incorporar a cadeia de bases.
struct Layout {
    parent: Option<u32>,
    fields: BTreeMap<String, Field>,
    methods: BTreeMap<String, Method>,
    slots: usize,
}
/// Metadados nominais compartilhados e constantes de strings do módulo.
pub(super) struct Objects {
    layouts: BTreeMap<u32, Layout>,
    pub(super) globals: RefCell<Vec<String>>,
}
impl Objects {
    /// Monta bases antes das derivadas; diagnostica ciclos em HIR externa.
    pub(super) fn new(module: &Module<'_>) -> Result<Self, Diagnostic> {
        let mut layouts: BTreeMap<u32, Layout> = BTreeMap::new();
        while layouts.len() < module.classes.len() {
            let before = layouts.len();
            for class in &module.classes {
                if layouts.contains_key(&class.id)
                    || class
                        .superclass
                        .is_some_and(|id| !layouts.contains_key(&id))
                {
                    continue;
                }
                let parent = class.superclass.and_then(|id| layouts.get(&id));
                let mut fields = parent.map(|p| p.fields.clone()).unwrap_or_default();
                let mut methods = parent.map(|p| p.methods.clone()).unwrap_or_default();
                let mut slots = parent.map_or(0, |p| p.slots);
                for field in &class.fields {
                    validate_expression(&field.initializer)?;
                    let ty = value_ty(field.ty, field.span)?;
                    fields.insert(field.name.into(), Field { ty, offset: slots });
                    slots += if matches!(ty, Ty::NullableInt | Ty::NullableBool) {
                        2
                    } else {
                        1
                    };
                }
                for (index, method) in class.methods.iter().enumerate() {
                    validate_statements(&method.body)?;
                    methods.insert(
                        method.name.into(),
                        Method {
                            owner: class.id,
                            index,
                            signature: Signature {
                                symbol: format!("df_method_{}_{}", class.id, index),
                                result: ty(method.return_type, method.span)?,
                                parameters: method
                                    .parameters
                                    .iter()
                                    .map(|p| value_ty(p.ty, p.span))
                                    .collect::<Result<_, _>>()?,
                            },
                        },
                    );
                }
                layouts.insert(
                    class.id,
                    Layout {
                        parent: class.superclass,
                        fields,
                        methods,
                        slots,
                    },
                );
            }
            if before == layouts.len() {
                return Err(error(
                    module.classes[0].span,
                    "hierarquia cíclica ou base ausente",
                ));
            }
        }
        Ok(Self {
            layouts,
            globals: RefCell::new(vec![]),
        })
    }
    /// Compatibilidade nominal preserva a identidade concreta do objeto.
    pub(super) fn assignable(&self, actual: Ty, expected: Ty) -> bool {
        if actual == expected {
            return true;
        }
        if let (Ty::Class(mut actual), Ty::Class(expected)) = (actual, expected) {
            while let Some(parent) = self.layouts.get(&actual).and_then(|l| l.parent) {
                if parent == expected {
                    return true;
                }
                actual = parent;
            }
        }
        false
    }
    /// Resolve o slot pelo tipo estático do receiver, preservando diagnóstico de origem.
    pub(super) fn field(&self, receiver: Ty, name: &str, span: Span) -> Result<Field, Diagnostic> {
        let Ty::Class(id) = receiver.base() else {
            return Err(error(span, "campo neste tipo"));
        };
        self.layouts
            .get(&id)
            .and_then(|l| l.fields.get(name))
            .cloned()
            .ok_or_else(|| error(span, "campo ausente"))
    }
    /// Inicializadores seguem Dart: campos derivados antes dos campos das bases.
    pub(super) fn emit(
        &self,
        module: &Module<'_>,
        signatures: &HashMap<String, Signature>,
    ) -> Result<String, Diagnostic> {
        let mut output = String::new();
        for class in &module.classes {
            let mut emitter = FunctionEmitter::new(signatures, self, Ty::Class(class.id));
            let object = emitter.register();
            emitter.line(format!(
                "{object} = call i64 @dartforge_object_new(i64 {}, i64 {})",
                class.id, self.layouts[&class.id].slots
            ));
            let receiver = Value {
                ty: Ty::Class(class.id),
                text: object,
            };
            emitter.root(&receiver);
            let mut current = Some(class.id);
            while let Some(id) = current {
                let declaration = module.classes.iter().find(|c| c.id == id).unwrap();
                for field in &declaration.fields {
                    let value = emitter.expression(&field.initializer)?;
                    emitter.store_field(
                        &receiver,
                        &self.layouts[&class.id].fields[field.name],
                        value,
                        field.span,
                    )?;
                }
                current = declaration.superclass;
            }
            emitter.end_frame();
            emitter.line(format!("ret i64 {}", receiver.text));
            emitter.terminated = true;
            output.push_str(&emitter.finish(&format!("df_new_{}", class.id), ""));
            for (index, method) in class.methods.iter().enumerate() {
                let info = &self.layouts[&class.id].methods[method.name];
                let mut emitter = FunctionEmitter::new(signatures, self, info.signature.result);
                emitter.this_class = Some(class.id);
                emitter.root(&Value {
                    ty: Ty::Class(class.id),
                    text: "%this".into(),
                });
                let mut params = vec!["i64 %this".into()];
                for (i, p) in method.parameters.iter().enumerate() {
                    let t = info.signature.parameters[i];
                    params.push(format!("{} %a{i}", t.ir()));
                    emitter.root(&Value {
                        ty: t,
                        text: format!("%a{i}"),
                    });
                    let ptr = emitter.local(p.name, t);
                    emitter.root_local(
                        &ptr,
                        &Value {
                            ty: t,
                            text: format!("%a{i}"),
                        },
                    );
                    emitter.line(format!("store {} %a{i}, ptr {ptr}", t.ir()));
                }
                emitter.block(&method.body)?;
                output.push_str(&emitter.finish(
                    &format!("df_method_{}_{index}", class.id),
                    &params.join(", "),
                ));
            }
        }
        // Adaptadores preservam contravariancia dos parametros e retorno covariante.
        for (&id, layout) in &self.layouts {
            for (name, method) in &layout.methods {
                let sig = &method.signature;
                let mut emitter = FunctionEmitter::new(signatures, self, sig.result);
                let mut params = vec!["i64 %this".into()];
                emitter.root(&Value {
                    ty: Ty::Class(id),
                    text: "%this".into(),
                });
                for (i, t) in sig.parameters.iter().enumerate() {
                    params.push(format!("{} %a{i}", t.ir()));
                    emitter.root(&Value {
                        ty: *t,
                        text: format!("%a{i}"),
                    });
                }
                emitter.line("%class = call i64 @dartforge_object_class(i64 %this)".into());
                emitter.line("switch i64 %class, label %invalid [".into());
                let descendants = self
                    .layouts
                    .iter()
                    .filter(|(child, _)| self.assignable(Ty::Class(**child), Ty::Class(id)))
                    .collect::<Vec<_>>();
                for (child, _) in &descendants {
                    emitter.line(format!("  i64 {child}, label %case{child}"));
                }
                emitter.line("]".into());
                emitter.terminated = true;
                emitter.start("invalid");
                emitter.line("call void @dartforge_null_assert_fail()".into());
                emitter.line("unreachable".into());
                emitter.terminated = true;
                for (child, descendant) in descendants {
                    emitter.start(&format!("case{child}"));
                    let target = &descendant.methods[name].signature;
                    let mut args = vec!["i64 %this".into()];
                    for (i, (actual, expected)) in
                        sig.parameters.iter().zip(&target.parameters).enumerate()
                    {
                        let value = emitter.coerce(
                            Value {
                                ty: *actual,
                                text: format!("%a{i}"),
                            },
                            *expected,
                            Span { start: 0, end: 0 },
                        )?;
                        args.push(format!("{} {}", value.ty.ir(), value.text));
                    }
                    let value = if target.result == Ty::Void {
                        emitter.line(format!("call void @{}({})", target.symbol, args.join(", ")));
                        Value {
                            ty: Ty::Void,
                            text: String::new(),
                        }
                    } else {
                        let r = emitter.register();
                        emitter.line(format!(
                            "{r} = call {} @{}({})",
                            target.result.ir(),
                            target.symbol,
                            args.join(", ")
                        ));
                        Value {
                            ty: target.result,
                            text: r,
                        }
                    };
                    let value = emitter.coerce(value, sig.result, Span { start: 0, end: 0 })?;
                    emitter.end_frame();
                    emitter.line(if sig.result == Ty::Void {
                        "ret void".into()
                    } else {
                        format!("ret {} {}", value.ty.ir(), value.text)
                    });
                    emitter.terminated = true;
                }
                output.push_str(&emitter.finish(
                    &format!("df_dispatch_{id}_{}_{}", method.owner, method.index),
                    &params.join(", "),
                ));
            }
        }
        Ok(output)
    }
}

impl FunctionEmitter<'_> {
    /// Reserva um slot estático, compartilhado apenas por reexecuções deste ponto.
    pub(super) fn reserve_root(&mut self) -> usize {
        self.has_roots = true;
        let slot = self.root_slots;
        self.root_slots += 1;
        slot
    }
    /// Atualiza o slot de um local independentemente do temporário que o originou.
    pub(super) fn root_local(&mut self, pointer: &str, value: &Value) {
        if let Some(slot) = self.local_roots.get(pointer).copied() {
            self.line(format!(
                "call void @dartforge_gc_set_root(i64 %gcframe, i64 {slot}, i64 {})",
                value.text
            ));
        }
    }
    /// Mantém temporários vivos; cada ponto de emissão possui slot próprio reutilizável.
    pub(super) fn root(&mut self, value: &Value) {
        if value.ty.reference() {
            let slot = self.reserve_root();
            self.line(format!(
                "call void @dartforge_gc_set_root(i64 %gcframe, i64 {slot}, i64 {})",
                value.text
            ));
        }
    }
    /// Literais usam bytes UTF-8 escapados: nenhum texto Dart vira sintaxe LLVM.
    pub(super) fn string(&mut self, text: &str) -> Value {
        let id = self.objects.globals.borrow().len();
        let len = text.len();
        let bytes = text
            .as_bytes()
            .iter()
            .map(|b| format!("\\{b:02X}"))
            .collect::<String>();
        self.objects.globals.borrow_mut().push(format!(
            "@df_string_{id} = private constant [{len} x i8] c\"{bytes}\""
        ));
        let r = self.register();
        self.line(format!(
            "{r} = call i64 @dartforge_string_new(ptr @df_string_{id}, i64 {len})"
        ));
        Value {
            ty: Ty::String,
            text: r,
        }
    }
    /// Grava payload e presença; informa ao GC quais slots contêm referências.
    pub(super) fn store_field(
        &mut self,
        receiver: &Value,
        field: &Field,
        value: Value,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let value = self.coerce(value, field.ty, span)?;
        let mut offset = field.offset;
        let value = if matches!(value.ty, Ty::NullableInt | Ty::NullableBool) {
            let tag = self.present(&value);
            let r = self.register();
            self.line(format!("{r} = zext i1 {tag} to i64"));
            self.line(format!(
                "call void @dartforge_object_set(i64 {}, i64 {offset}, i64 {r}, i8 0)",
                receiver.text
            ));
            offset += 1;
            self.payload(&value)
        } else {
            value
        };
        let bits = if value.ty == Ty::Bool {
            let r = self.register();
            self.line(format!("{r} = zext i1 {} to i64", value.text));
            r
        } else {
            value.text
        };
        self.line(format!(
            "call void @dartforge_object_set(i64 {}, i64 {offset}, i64 {bits}, i8 {})",
            receiver.text,
            u8::from(field.ty.reference())
        ));
        Ok(())
    }
    /// Reconstrói o valor tipado dos slots; a expressão chamadora enraíza referências.
    pub(super) fn load_field(&mut self, receiver: &Value, field: &Field) -> Value {
        let r = self.register();
        self.line(format!(
            "{r} = call i64 @dartforge_object_get(i64 {}, i64 {})",
            receiver.text, field.offset
        ));
        if matches!(field.ty, Ty::NullableInt | Ty::NullableBool) {
            let tag = self.register();
            self.line(format!("{tag} = trunc i64 {r} to i1"));
            let payload = self.register();
            self.line(format!(
                "{payload} = call i64 @dartforge_object_get(i64 {}, i64 {})",
                receiver.text,
                field.offset + 1
            ));
            let payload = if field.ty == Ty::NullableBool {
                let b = self.register();
                self.line(format!("{b} = trunc i64 {payload} to i1"));
                b
            } else {
                payload
            };
            let a = self.register();
            let b = self.register();
            self.line(format!(
                "{a} = insertvalue {} zeroinitializer, i1 {tag}, 0",
                field.ty.ir()
            ));
            self.line(format!(
                "{b} = insertvalue {} {a}, {} {payload}, 1",
                field.ty.ir(),
                field.ty.base().ir()
            ));
            Value {
                ty: field.ty,
                text: b,
            }
        } else if field.ty == Ty::Bool {
            let b = self.register();
            self.line(format!("{b} = trunc i64 {r} to i1"));
            Value {
                ty: Ty::Bool,
                text: b,
            }
        } else {
            Value {
                ty: field.ty,
                text: r,
            }
        }
    }
    /// Avalia receiver e argumentos em ordem antes do despacho pela classe concreta.
    pub(super) fn method_call(
        &mut self,
        receiver: Value,
        name: &str,
        args: &[Expr<'_>],
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let Ty::Class(id) = receiver.ty.base() else {
            return Err(error(span, "metodo neste tipo"));
        };
        let receiver = self.coerce(receiver, Ty::Class(id), span)?;
        let method = self
            .objects
            .layouts
            .get(&id)
            .and_then(|l| l.methods.get(name))
            .cloned()
            .ok_or_else(|| error(span, "metodo ausente"))?;
        if args.len() != method.signature.parameters.len() {
            return Err(error(span, "aridade de metodo"));
        }
        let mut values = vec![format!("i64 {}", receiver.text)];
        for (arg, t) in args.iter().zip(&method.signature.parameters) {
            let v = self.expression(arg)?;
            let v = self.coerce(v, *t, arg.span)?;
            values.push(format!("{} {}", t.ir(), v.text));
        }
        let symbol = format!("df_dispatch_{id}_{}_{}", method.owner, method.index);
        let result = method.signature.result;
        let text = if result == Ty::Void {
            self.line(format!("call void @{symbol}({})", values.join(", ")));
            String::new()
        } else {
            let r = self.register();
            self.line(format!(
                "{r} = call {} @{symbol}({})",
                result.ir(),
                values.join(", ")
            ));
            r
        };
        Ok(Value { ty: result, text })
    }
}

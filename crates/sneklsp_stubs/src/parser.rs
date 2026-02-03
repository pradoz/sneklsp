use crate::{ParameterKind, StubClass, StubFunction, StubModule, StubParameter, TypeAnnotation};
use rustc_hash::FxHashMap;
use sneklsp_ast::*;

pub struct StubParser;

impl StubParser {
    pub fn parse_module(name: &str, source: &str) -> StubModule {
        let arena = AstArena::with_capacity(source.len() * 50);
        let mut module = StubModule::new(name.to_string());

        let ast = match sneklsp_parser::parse(source, &arena) {
            Ok(ast) => ast,
            Err(e) => {
                tracing::warn!(module = name, ?e, "failed to parse stub");
                return module;
            }
        };

        if let Some(Statement::Expr(expr)) = ast.body.first() {
            if let Expression::String(s) = &expr.value {
                module.docstring = Some(s.value.to_string());
            }
        }

        for stmt in ast.body {
            Self::process_statement(&mut module, stmt, source);
        }

        module
    }

    fn process_statement(module: &mut StubModule, stmt: &Statement<'_>, source: &str) {
        match stmt {
            Statement::FunctionDef(func) => {
                let stub = Self::convert_function(func, source);
                module.functions.insert(stub.name.clone(), stub);
            }
            Statement::ClassDef(class) => {
                let stub = Self::convert_class(class, source);
                module.classes.insert(stub.name.clone(), stub);
            }
            Statement::Assign(assign) => {
                for target in assign.targets {
                    if let Expression::Name(name) = target {
                        let type_ann = Self::infer_type_from_value(&assign.value);
                        module.variables.insert(name.id.to_string(), type_ann);
                    }
                }
            }
            Statement::Import(s) => {
                for alias in s.names {
                    module.submodules.push(alias.name.to_string());
                }
            }
            _ => {}
        }
    }

    fn convert_function(func: &FunctionDef<'_>, source: &str) -> StubFunction {
        todo!()
    }

    fn convert_class(class: &ClassDef<'_>, source: &str) -> StubClass {
        let mut stub = StubClass::new(class.name.to_string());

        for base in class.bases {
            stub.bases.push(Self::expr_to_string(base, source));
        }

        if let Some(Statement::Expr(expr)) = class.body.first() {
            if let Expression::String(s) = &expr.value {
                stub.docstring = Some(s.value.to_string());
            }
        }

        for stmt in class.body {
            match stmt {
                Statement::FunctionDef(func) => {
                    let method = Self::convert_function(func, source);
                    stub.methods.insert(method.name.clone(), method);
                }
                Statement::Assign(assign) => {
                    for target in assign.targets {
                        if let Expression::Name(name) = target {
                            let type_ann = Self::infer_type_from_value(&assign.value);
                            stub.attributes.insert(name.id.to_string(), type_ann);
                        }
                    }
                }
                _ => {}
            }
        }

        stub
    }

    fn convert_type_annotation(expr: &Expression<'_>, source: &str) -> TypeAnnotation {
        match expr {
            Expression::Name(name) => TypeAnnotation::parse(name.id),
            // union type
            Expression::BinOp(binop) => {
                let left = Self::convert_type_annotation(&binop.left, source);
                let right = Self::convert_type_annotation(&binop.right, source);
                TypeAnnotation::Union(vec![left, right])
            },
            Expression::Subscript(sub) => todo!(),
            Expression::None(_) => TypeAnnotation::None,
            // fall back to string representation
            _ => TypeAnnotation::Unknown(Self::expr_to_string(expr, source)),
        }
    }

    fn infer_type_from_value(expr: &Expression<'_>) -> TypeAnnotation {
        match expr {
            Expression::Int(_) => TypeAnnotation::Name("int".to_string()),
            Expression::Float(_) => TypeAnnotation::Name("float".to_string()),
            Expression::String(_) => TypeAnnotation::Name("str".to_string()),
            Expression::Bool(_) => TypeAnnotation::Name("bool".to_string()),
            Expression::None(_) => TypeAnnotation::None,
            Expression::List(_) => {
                TypeAnnotation::Subscript("list".to_string(), vec![TypeAnnotation::Any])
            }
            Expression::Dict(_) => TypeAnnotation::Subscript(
                "dict".to_string(),
                vec![TypeAnnotation::Any, TypeAnnotation::Any],
            ),
            Expression::Tuple(_) => TypeAnnotation::Name("tuple".to_string()),
            _ => TypeAnnotation::Any,
        }
    }

    fn expr_to_string(expr: &Expression<'_>, source: &str) -> String {
        let range = expr.range();
        let start = range.start().to_usize();
        let end = range.end().to_usize();

        if end <= source.len() {
            source[start..end].to_string()
        } else {
            "...".to_string()
        }
    }
}

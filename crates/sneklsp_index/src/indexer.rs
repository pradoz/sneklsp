use crate::{ModuleIndex, ScopeId, ScopeKind, SymbolKind};

use sneklsp_ast::*;
use sneklsp_text::{TextRange, TextSize};

pub struct Indexer<'src> {
    source: &'src str,
    index: ModuleIndex<'src>,
    current_scope: ScopeId,
}

impl<'src> Indexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            index: ModuleIndex::new(),
            current_scope: ScopeId::ROOT,
        }
    }

    pub fn index(mut self, module: &Module<'src>) -> ModuleIndex<'src> {
        self.index.add_module_scope(module.range);
        self.visit_stmts(module.body);
        self.index.finish();
        self.index
    }

    fn visit_stmts(&mut self, stmts: &[Statement<'src>]) {
        for stmt in stmts {
            self.visit_stmt(*stmt);
        }
    }

    fn visit_exprs(&mut self, exprs: &[Expression<'src>]) {
        for expr in exprs {
            self.visit_expr(*expr);
        }
    }

    fn visit_stmt(&mut self, stmt: Statement<'src>) {
        match stmt {
            Statement::FunctionDef(f) => self.visit_function_def(f),
            Statement::ClassDef(c) => self.visit_class_def(c),
            Statement::Return(r) => self.visit_return(r),
            Statement::Assign(a) => self.visit_assign(a),
            Statement::AugAssign(a) => self.visit_aug_assign(a),
            Statement::If(i) => self.visit_if(i),
            Statement::For(f) => self.visit_for(f),
            Statement::While(w) => self.visit_while(w),
            Statement::Import(i) => self.visit_import(i),
            Statement::ImportFrom(i) => self.visit_import_from(i),
            Statement::Expr(e) => self.visit_expr_stmt(e),
            Statement::Pass(_) | Statement::Break(_) | Statement::Continue(_) => {}
        }
    }

    fn visit_function_def(&mut self, func: &FunctionDef<'src>) {
        let kind = if self.is_in_class_scope() {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        };

        let name_range = self.name_range(func.name, func.range);
        let _symbol_id =
            self.index
                .add_symbol(func.name, kind, func.range, name_range, self.current_scope);

        let func_scope = self
            .index
            .add_scope(ScopeKind::Function, self.current_scope, func.range);

        self.with_scope(func_scope, |this| {
            for p in func.params {
                this.visit_parameter(p);
            }

            if let Some(returns) = func.returns {
                this.visit_expr(returns);
            }

            this.visit_stmts(func.body);
        });
    }

    fn with_scope<F, R>(&mut self, scope: ScopeId, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let parent_scope = self.current_scope;
        self.current_scope = scope;
        let result = f(self); // execute in dirrent scope
        self.current_scope = parent_scope; // restore scope
        result
    }

    fn visit_parameter(&mut self, param: &Parameter<'src>) {
        let name_range = self.name_range(param.name, param.range);
        self.index.add_symbol(
            param.name,
            SymbolKind::Parameter,
            param.range,
            name_range,
            self.current_scope,
        );

        if let Some(annotation) = param.annotation {
            self.visit_expr(annotation);
        }

        if let Some(default) = param.default {
            self.visit_expr(default);
        }
    }

    fn visit_class_def(&mut self, class: &ClassDef<'src>) {
        let name_range = self.name_range(class.name, class.range);
        let _symbol_id = self.index.add_symbol(
            class.name,
            SymbolKind::Class,
            class.range,
            name_range,
            self.current_scope,
        );

        self.visit_exprs(class.bases);

        let inner_scope = self
            .index
            .add_scope(ScopeKind::Class, self.current_scope, class.range);

        self.with_scope(inner_scope, |this| {
            this.visit_stmts(class.body);
        });
    }

    fn visit_return(&mut self, ret: &ReturnStmt<'src>) {
        if let Some(value) = ret.value {
            self.visit_expr(value);
        }
    }

    fn visit_assign(&mut self, assign: &AssignStmt<'src>) {
        self.visit_expr(assign.value);

        for target in assign.targets {
            self.visit_assign_target(*target);
        }
    }

    fn visit_aug_assign(&mut self, aug: &AugAssignStmt<'src>) {
        self.visit_expr(aug.target);
        self.visit_expr(aug.value);
    }

    fn visit_assign_target(&mut self, target: Expression<'src>) {
        match target {
            Expression::Name(name) => {
                // if already defined in scope, it's a reassignment - no new symbol
                let existing = self.index.resolve_name(name.id, self.current_scope);
                let is_in_new_scope = existing.map_or(true, |sym| {
                    self.index.symbol(sym).scope != self.current_scope
                });

                if is_in_new_scope {
                    self.index.add_symbol(
                        name.id,
                        SymbolKind::Variable,
                        name.range,
                        name.range,
                        self.current_scope,
                    );
                }
            }
            Expression::List(list) => self.visit_assign_targets(list.elts),
            Expression::Tuple(tuple) => self.visit_assign_targets(tuple.elts),
            Expression::Attribute(_) | Expression::Subscript(_) => self.visit_expr(target),
            _ => self.visit_expr(target),
        }
    }

    fn visit_assign_targets(&mut self, targets: &[Expression<'src>]) {
        for e in targets {
            self.visit_assign_target(*e);
        }
    }

    fn visit_if(&mut self, if_stmt: &IfStmt<'src>) {
        self.visit_expr(if_stmt.test);
        self.visit_stmts(if_stmt.body);
        self.visit_stmts(if_stmt.orelse);
    }

    fn visit_for(&mut self, for_stmt: &ForStmt<'src>) {
        self.visit_expr(for_stmt.iter);
        self.visit_assign_target(for_stmt.target);
        self.visit_stmts(for_stmt.body);
        self.visit_stmts(for_stmt.orelse);
    }

    fn visit_while(&mut self, while_stmt: &WhileStmt<'src>) {
        self.visit_expr(while_stmt.test);
        self.visit_stmts(while_stmt.body);
        self.visit_stmts(while_stmt.orelse);
    }

    fn visit_import(&mut self, import: &ImportStmt<'src>) {
        for alias in import.names {
            self.add_import_symbol(alias, SymbolKind::Import);
        }
    }

    fn visit_import_from(&mut self, import: &ImportFromStmt<'src>) {
        for alias in import.names {
            self.add_import_symbol(alias, SymbolKind::ImportedSymbol);
        }
    }

    fn add_import_symbol(&mut self, alias: &Alias<'src>, kind: SymbolKind) {
        let name = alias.asname.unwrap_or(alias.name);
        let range = self.name_range(name, alias.range);
        self.index
            .add_symbol(name, kind, alias.range, range, self.current_scope);
    }

    fn visit_expr_stmt(&mut self, expr_stmt: &ExprStmt<'src>) {
        self.visit_expr(expr_stmt.value);
    }

    fn visit_expr(&mut self, expr: Expression<'src>) {
        match expr {
            Expression::Name(name) => self.visit_name(name),
            Expression::BinOp(binop) => {
                self.visit_expr(binop.left);
                self.visit_expr(binop.right);
            }
            Expression::UnaryOp(unary) => {
                self.visit_expr(unary.operand);
            }
            Expression::Compare(compare) => {
                self.visit_expr(compare.left);
                self.visit_exprs(compare.comparators);
            }
            Expression::Call(call) => {
                self.visit_expr(call.func);
                self.visit_exprs(call.args);
            }
            Expression::Attribute(attr) => {
                self.visit_expr(attr.value);
                // NOTE: not tracking attr.attr as a reference because it requires type information to resolve
            }
            Expression::Subscript(sub) => {
                self.visit_expr(sub.value);
                self.visit_expr(sub.slice);
            }
            Expression::List(list) => self.visit_exprs(list.elts),
            Expression::Tuple(tuple) => self.visit_exprs(tuple.elts),
            Expression::Dict(dict) => {
                for key in dict.keys {
                    if let Some(k) = key {
                        self.visit_expr(*k);
                    }
                }
                self.visit_exprs(dict.values);
            }
            Expression::Int(_)
            | Expression::Float(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::None(_) => {}
        }
    }

    fn visit_name(&mut self, name: &NameExpr<'src>) {
        let resolved = self.index.resolve_name(name.id, self.current_scope);
        self.index.add_reference(name.id, name.range, resolved);
    }

    fn is_in_class_scope(&self) -> bool {
        if self.current_scope.is_root() {
            return false;
        }
        self.index.scope(self.current_scope).kind == ScopeKind::Class
    }

    fn name_range(&self, name: &str, container: TextRange) -> TextRange {
        let start = container.start().to_usize();
        let end = container.end().to_usize();
        let slice = &self.source[start..end];

        if let Some(offset) = slice.find(name) {
            let name_start = start + offset;
            let name_end = name_start + name.len();
            TextRange::new(
                TextSize::new(name_start as u32),
                TextSize::new(name_end as u32),
            )
        } else {
            // fallback to container start if name not in slice
            TextRange::new(
                container.start(),
                TextSize::new(container.start().to_u32() + name.len() as u32),
            )
        }
    }
}

pub fn index_module<'src>(source: &'src str, module: &Module<'src>) -> ModuleIndex<'src> {
    Indexer::new(source).index(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sneklsp_ast::AstArena;

    #[test]
    fn simple_assignment() {
        let source = "x = 1";
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let index = index_module(source, &module);

        assert_eq!(index.symbols().len(), 1);
        assert_eq!(index.symbols()[0].name, "x");
        assert_eq!(index.symbols()[0].kind, SymbolKind::Variable);
    }

    #[test]
    fn function_def() {
        let source = "def foo(a, b):\n    return a + b";
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let index = index_module(source, &module);

        // should have: foo, a, b
        assert_eq!(index.symbols().len(), 3);

        let foo = &index.symbols()[0];
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.kind, SymbolKind::Function);
        let a = &index.symbols()[1];
        assert_eq!(a.name, "a");
        assert_eq!(a.kind, SymbolKind::Parameter);
        let b = &index.symbols()[2];
        assert_eq!(b.name, "b");
        assert_eq!(b.kind, SymbolKind::Parameter);
    }

    #[test]
    fn class_def() {
        let source = "class Foo:\n    def bar(self):\n        pass";
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let index = index_module(source, &module);

        // should have: Foo, bar, self
        assert_eq!(index.symbols().len(), 3);

        let foo = &index.symbols()[0];
        assert_eq!(foo.name, "Foo");
        assert_eq!(foo.kind, SymbolKind::Class);
        let bar = &index.symbols()[1];
        assert_eq!(bar.name, "bar");
        assert_eq!(bar.kind, SymbolKind::Method);
    }

    #[test]
    fn reference_resolution() {
        let source = "x = 1\ny = x";
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let index = index_module(source, &module);

        let refs: Vec<_> = index
            .references()
            .iter()
            .filter(|r| r.name == "x")
            .collect();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].is_resolved());
    }

    #[test]
    fn scope_hierarchy() {
        let source = "def outer():\n    def inner():\n        pass";
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let index = index_module(source, &module);

        // should have 3 scopes: module, outer, inner
        assert_eq!(index.scopes().len(), 3);

        let module_scope = &index.scopes()[0];
        assert_eq!(module_scope.kind, ScopeKind::Module);
        assert_eq!(module_scope.children.len(), 1);

        let outer_scope = &index.scopes()[1];
        assert_eq!(outer_scope.kind, ScopeKind::Function);
        assert_eq!(outer_scope.children.len(), 1);
    }

    #[test]
    fn import() {
        let source = "import os\nfrom sys import path";
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let index = index_module(source, &module);

        assert_eq!(index.symbols().len(), 2);

        let os_sym = &index.symbols()[0];
        assert_eq!(os_sym.name, "os");
        assert_eq!(os_sym.kind, SymbolKind::Import);

        let path_sym = &index.symbols()[1];
        assert_eq!(path_sym.name, "path");
        assert_eq!(path_sym.kind, SymbolKind::ImportedSymbol);
    }
}

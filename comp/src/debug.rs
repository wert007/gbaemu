use crate::{
    BoundId, Compiler, HasLocation, StringInterner,
    bind::BoundBinaryOperator,
    syntax_tree::{Parsed, SyntaxNode, SyntaxNodeKind, SyntaxTree},
    typing::{TypeId, Types},
    variables::{VariableId, Variables},
};

impl Variables {
    pub fn dump(&self, strings: &StringInterner, types: &Types) {
        for scope in &self.all_scopes {
            eprintln!(
                " -- Scope {} ({} Variables) -- ",
                scope.id.as_raw(),
                &self.variables[&scope.id].len()
            );
            for variable in &self.variables[&scope.id] {
                let mut name = String::new();
                for namespace in &variable.namespaces {
                    name.push_str(&strings[*namespace]);
                    name.push_str("::");
                }
                name.push_str(&strings[variable.name]);

                eprintln!("    {name}: {}", types.to_string(variable.type_, strings));
            }
        }
    }
}

pub(crate) fn dump_bound_tree(node: BoundId, compiler: &Compiler) {
    // panic!();
    dump_bound_tree_recursive(node, compiler, 0)
}

fn dump_bound_tree_recursive(node: BoundId, compiler: &Compiler, indent: usize) {
    let n = |v: VariableId| &compiler.strings[v];
    let t = |v: TypeId| compiler.types.to_string(v, &compiler.strings);
    emit_indent(indent);
    // dbg!(&compiler.nodes[node]);
    let stage = &compiler.nodes[node].stage;
    match &compiler.nodes[node].kind {
        SyntaxNodeKind::Error => println!("Error"),
        SyntaxNodeKind::Program(program_node) => {
            println!("Program:");
            for statement in &program_node.top_level_statements {
                dump_bound_tree_recursive(*statement, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::ConstDeclaration(const_declaration_node) => {
            let ti = compiler
                .variables
                .type_of(const_declaration_node.identifier);
            println!(
                "const {}: {}= {:?};",
                n(const_declaration_node.identifier),
                t(ti),
                const_declaration_node.expr
            );
        }
        SyntaxNodeKind::Literal(_) => {
            println!("Literal {:?}: {}", &stage.constant_value, t(stage.type_));
        }
        SyntaxNodeKind::Identifier(token) => {
            print!("{}: {}", n(*token), t(stage.type_));
        }
        SyntaxNodeKind::Binary(binary_node) => {
            match binary_node.op {
                BoundBinaryOperator::Addition => println!("Binary +"),
                BoundBinaryOperator::Subtraction => println!("Binary -"),
                BoundBinaryOperator::Multiplication => println!("Binary *"),
                BoundBinaryOperator::Division => println!("Binary /"),
                BoundBinaryOperator::Modulo => println!("Binary %"),
            }
            dump_bound_tree_recursive(binary_node.lhs, compiler, indent + 1);
            dump_bound_tree_recursive(binary_node.rhs, compiler, indent + 1);
        }
        SyntaxNodeKind::CommaedExpression((expr, _)) => {
            dump_bound_tree_recursive(*expr, compiler, 0)
        }
        SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
            println!("Array: {}", t(stage.type_));
            for entry in &array_literal_node.entries {
                dump_bound_tree_recursive(*entry, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::ExpressionStatement(expression_statement_node) => {
            dump_bound_tree_recursive(expression_statement_node.expression, compiler, indent);
        }
        SyntaxNodeKind::BlockExpression(block_expression_node) => {
            println!("Block: {}", t(stage.type_));
            for s in &block_expression_node.body {
                dump_bound_tree_recursive(*s, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::FunctionDeclaration(function_declaration_node) => {
            println!("fn {}", n(function_declaration_node.identifier));
            // function_declaration_node.head.
            dump_bound_tree_recursive(function_declaration_node.body, compiler, indent + 1);
        }
        SyntaxNodeKind::FunctionCall(function_call_node) => {
            println!("FunctionCall");
            println!("Base");
            dump_bound_tree_recursive(function_call_node.base, compiler, indent + 1);
            println!("Args");
            for arg in &function_call_node.arguments {
                dump_bound_tree_recursive(*arg, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::AssignmentStatement(assignment_statement_node) => {
            println!("Assignment");
            dump_bound_tree_recursive(assignment_statement_node.lhs, compiler, indent + 1);
            dump_bound_tree_recursive(assignment_statement_node.value, compiler, indent + 1);
        }
        SyntaxNodeKind::Conversion(conversion_node) => {
            println!("Convert to {}", t(stage.type_));
            dump_bound_tree_recursive(conversion_node.base, compiler, indent + 1);
        }
        SyntaxNodeKind::StructDeclaration(struct_declaration_node) => {
            println!("struct {}", n(struct_declaration_node.identifier));
            for field in &struct_declaration_node.fields {
                emit_indent(indent + 1);
                println!("{}: {}", n(field.identifier), t(field.type_))
            }
        }
        SyntaxNodeKind::StructLiteral(struct_literal_node) => {
            println!("{}", n(struct_literal_node.identifier));
            for field in &struct_literal_node.fields {
                emit_indent(indent + 1);
                print!("{}:", &compiler.strings[field.identifier]);
                dump_bound_tree_recursive(field.expression, compiler, 0);
                println!();
            }
        }
        SyntaxNodeKind::FieldAccess(field_access_node) => todo!(),
        SyntaxNodeKind::ImplBlock(impl_block_node) => todo!(),
    }
}

fn emit_indent(indent: usize) {
    for _ in 0..indent {
        print!("    ")
    }
}

pub(crate) fn dump_parse_tree(tree: &SyntaxTree<Parsed>, compiler: &mut Compiler) {
    dump_parse_node(&tree.node, compiler, 0);
}

fn dump_parse_node(node: &SyntaxNode<Parsed>, compiler: &mut Compiler, indent: usize) {
    emit_indent(indent);
    match &node.kind {
        SyntaxNodeKind::Error => println!("#Error"),
        SyntaxNodeKind::Program(program_node) => {
            println!("Program:");
            for node in &program_node.top_level_statements {
                dump_parse_node(node, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::ConstDeclaration(const_declaration_node) => {
            println!(
                "Const {} =",
                &compiler[const_declaration_node.identifier.location()]
            );
            dump_parse_node(&const_declaration_node.expr, compiler, indent + 1);
        }
        SyntaxNodeKind::Literal(token) => {
            println!("Lit {}", &compiler[token.location()]);
        }
        SyntaxNodeKind::Identifier(token) => {
            println!("Var {}", &compiler[token.location()]);
        }
        SyntaxNodeKind::Binary(binary_node) => {
            println!("Binary {}", &compiler[binary_node.op.location()]);
            dump_parse_node(&binary_node.lhs, compiler, indent + 1);
            dump_parse_node(&binary_node.rhs, compiler, indent + 1);
        }
        SyntaxNodeKind::CommaedExpression((expr, _)) => {
            println!("Commaed:");
            dump_parse_node(&*expr, compiler, indent + 1);
        }
        SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
            println!("Array Literal:");
            for entry in &array_literal_node.entries {
                dump_parse_node(entry, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::ExpressionStatement(expression_statement_node) => {
            println!(
                "Expression Statement (Semicolon? {:?})",
                expression_statement_node.semicolon.is_some()
            );
            dump_parse_node(&expression_statement_node.expression, compiler, indent + 1);
        }
        SyntaxNodeKind::BlockExpression(block_expression_node) => {
            println!("Block expression");
            for node in &block_expression_node.body {
                dump_parse_node(node, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::FunctionDeclaration(function_declaration_node) => {
            println!(
                "Function Declaration {}:",
                &compiler[function_declaration_node.identifier.location()]
            );
            emit_indent(indent);
            println!("Head");
            for parameter in &function_declaration_node.head.parameters {
                emit_indent(indent + 1);
                println!(
                    "> {}: {}",
                    &compiler[parameter.identifier.location()],
                    &compiler[parameter.type_.location()]
                );
            }
            if let Some((_, r)) = &function_declaration_node.head.return_type {
                emit_indent(indent + 1);
                println!("return {}", &compiler[r.location()])
            }
            emit_indent(indent);
            println!("Body");
            dump_parse_node(&function_declaration_node.body, compiler, indent + 1);
        }
        SyntaxNodeKind::FunctionCall(function_call_node) => {
            println!("Function call");
            dump_parse_node(&function_call_node.base, compiler, indent + 1);
            emit_indent(indent);
            println!("Arguments");
            for argument in &function_call_node.arguments {
                dump_parse_node(argument, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::AssignmentStatement(assignment_statement_node) => {
            println!("Assignment");
            dump_parse_node(&assignment_statement_node.lhs, compiler, indent + 1);
            dump_parse_node(&assignment_statement_node.value, compiler, indent + 1);
        }
        SyntaxNodeKind::Conversion(conversion_node) => {
            println!("Cast {:?}", conversion_node.conversion_kind);
            dump_parse_node(&conversion_node.base, compiler, indent + 1);
        }
        SyntaxNodeKind::StructDeclaration(struct_declaration_node) => {
            println!(
                "Struct {}",
                &compiler[struct_declaration_node.identifier.location()]
            );
            for field in &struct_declaration_node.fields {
                emit_indent(indent + 1);
                println!(
                    "{}: {}",
                    &compiler[field.identifier.location()],
                    &compiler[field.type_.location()]
                );
            }
        }
        SyntaxNodeKind::StructLiteral(struct_literal_node) => {
            println!(
                "Struct literal {}",
                &compiler[struct_literal_node.identifier.location()]
            );
            for field in &struct_literal_node.fields {
                emit_indent(indent);
                println!("  {} =", &compiler[field.identifier.location()]);
                dump_parse_node(&field.expression, compiler, indent + 1);
            }
        }
        SyntaxNodeKind::FieldAccess(field_access_node) => {
            println!(
                "Access field {} of",
                &compiler[field_access_node.field.location()]
            );
            dump_parse_node(&field_access_node.base, compiler, indent + 1);
        }
        SyntaxNodeKind::ImplBlock(impl_block_node) => {
            println!("Impl {}", &compiler[impl_block_node.identifier.location()]);
            for node in &impl_block_node.body {
                dump_parse_node(node, compiler, indent + 1);
            }
        }
    }
}

//! Validate lexical loop targets, without allowing a closure or function to capture one.
use nu_protocol::{
    DeclId, ModuleId, ParseError,
    ast::{
        Argument, Block, Expr, Expression, ExternalArgument, MatchPattern, Pattern,
        PipelineRedirection, RecordItem,
    },
    engine::{CommandType, StateWorkingSet},
};

pub(crate) fn check(working_set: &StateWorkingSet, block: &Block) -> Vec<ParseError> {
    let mut errors = vec![];
    visit_block(working_set, block, false, &mut errors);
    // Module definitions are registered declarations and need not remain in the
    // enclosing AST. Validate their bodies independently as function boundaries.
    for index in working_set.permanent_state.num_decls()..working_set.num_decls() {
        if let Some(id) = working_set.get_decl(DeclId::new(index)).block_id() {
            visit_block(working_set, working_set.get_block(id), false, &mut errors);
        }
    }
    // Module environment initializers execute independently during use/overlay,
    // and may only be reachable through the module registry.
    for index in working_set.permanent_state.num_modules()..working_set.num_modules() {
        if let Some(id) = working_set.get_module(ModuleId::new(index)).env_block {
            visit_block(working_set, working_set.get_block(id), false, &mut errors);
        }
    }
    let mut spans = std::collections::HashSet::new();
    errors.retain(|error| {
        let span = error.span();
        spans.insert((span.start, span.end))
    });
    errors
}

fn visit_block(ws: &StateWorkingSet, block: &Block, in_loop: bool, errors: &mut Vec<ParseError>) {
    for pipeline in &block.pipelines {
        for element in &pipeline.elements {
            visit(ws, &element.expr, in_loop, errors);
            if let Some(redirection) = &element.redirection {
                match redirection {
                    PipelineRedirection::Single { target, .. } => {
                        if let Some(expr) = target.expr() {
                            visit(ws, expr, in_loop, errors);
                        }
                    }
                    PipelineRedirection::Separate { out, err } => {
                        for target in [out, err] {
                            if let Some(expr) = target.expr() {
                                visit(ws, expr, in_loop, errors);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn visit(ws: &StateWorkingSet, expr: &Expression, in_loop: bool, errors: &mut Vec<ParseError>) {
    match &expr.expr {
        Expr::Call(call) => {
            let decl = ws.get_decl(call.decl_id);
            let keyword = decl.command_type() == CommandType::Keyword;
            let name = decl.name();
            if keyword
                && matches!(name, "break" | "continue")
                && !in_loop
                && !call.named_iter().any(|(name, _, _)| name.item == "help")
            {
                errors.push(ParseError::LabeledError(
                    format!("{name} requires an enclosing loop"),
                    "no loop target in this function or closure".into(),
                    call.head,
                ));
            }
            let body_index = if keyword {
                match name {
                    "loop" => Some(0),
                    "while" => Some(1),
                    "for" => Some(2),
                    _ => None,
                }
            } else {
                None
            };
            let definition = keyword
                && matches!(
                    name,
                    "def" | "export def" | "module" | "export module" | "export-env"
                );
            let mut positional = 0;
            for arg in &call.arguments {
                let (arg, is_body, is_condition, is_catch) = match arg {
                    Argument::Positional(expr)
                    | Argument::Unknown(expr)
                    | Argument::Spread(expr) => {
                        let body = body_index == Some(positional);
                        // compile_while establishes its target before the condition.
                        // For for-loops, the iterable is evaluated before begin_loop.
                        let condition = keyword && name == "while" && positional == 0;
                        let catch = keyword && name == "try" && positional == 1;
                        positional += 1;
                        (Some(expr), body, condition, catch)
                    }
                    Argument::Named((_, _, expr)) => (expr.as_ref(), false, false, false),
                };
                if let Some(arg) = arg {
                    // compile_try inlines a literal catch handler into the current
                    // loop. A handler supplied as a closure value remains a boundary.
                    if is_catch {
                        if let Expr::Keyword(keyword) = &arg.expr {
                            if let Expr::Block(id) | Expr::Closure(id) = keyword.expr.expr {
                                visit_block(ws, ws.get_block(id), in_loop, errors);
                                continue;
                            }
                        }
                    }
                    if is_body {
                        if let Expr::Block(id) | Expr::Closure(id) = arg.expr {
                            visit_block(ws, ws.get_block(id), true, errors);
                            continue;
                        }
                    }
                    visit(ws, arg, (in_loop || is_condition) && !definition, errors);
                }
            }
            for expr in call.parser_info.values() {
                visit(ws, expr, in_loop && !definition, errors);
            }
        }
        Expr::Block(id) | Expr::Subexpression(id) => {
            visit_block(ws, ws.get_block(*id), in_loop, errors)
        }
        Expr::Closure(id) | Expr::RowCondition(id) => {
            visit_block(ws, ws.get_block(*id), false, errors)
        }
        Expr::BinaryOp(lhs, op, rhs) => {
            visit(ws, lhs, in_loop, errors);
            visit(ws, op, in_loop, errors);
            visit(ws, rhs, in_loop, errors);
        }
        Expr::UnaryNot(expr) | Expr::Collect(_, expr) => visit(ws, expr, in_loop, errors),
        Expr::Keyword(kw) => visit(ws, &kw.expr, in_loop, errors),
        Expr::FullCellPath(path) => visit(ws, &path.head, in_loop, errors),
        Expr::List(items) => {
            for item in items {
                visit(ws, item.expr(), in_loop, errors);
            }
        }
        Expr::Record(items) => {
            for item in items {
                match item {
                    RecordItem::Pair(k, v) => {
                        visit(ws, k, in_loop, errors);
                        visit(ws, v, in_loop, errors);
                    }
                    RecordItem::Spread(_, v) => visit(ws, v, in_loop, errors),
                }
            }
        }
        Expr::Table(table) => {
            for col in &table.columns {
                visit(ws, col, in_loop, errors);
            }
            for row in &table.rows {
                for col in row {
                    visit(ws, col, in_loop, errors);
                }
            }
        }
        Expr::StringInterpolation(items) | Expr::GlobInterpolation(items, _) => {
            for item in items {
                visit(ws, item, in_loop, errors);
            }
        }
        Expr::ExternalCall(head, args) => {
            visit(ws, head, in_loop, errors);
            for ExternalArgument::Regular(arg) | ExternalArgument::Spread(arg) in args.iter() {
                visit(ws, arg, in_loop, errors);
            }
        }
        Expr::MatchBlock(arms) => {
            for (pattern, expr) in arms {
                visit_pattern(ws, pattern, in_loop, errors);
                visit(ws, expr, in_loop, errors);
            }
        }
        Expr::Range(range) => {
            for expr in [&range.from, &range.next, &range.to].into_iter().flatten() {
                visit(ws, expr, in_loop, errors);
            }
        }
        Expr::ValueWithUnit(value) => visit(ws, &value.expr, in_loop, errors),
        Expr::AttributeBlock(block) => {
            for attribute in &block.attributes {
                visit(ws, &attribute.expr, in_loop, errors);
            }
            visit(ws, &block.item, in_loop, errors);
        }
        // These nodes contain values/metadata rather than executable AST children.
        Expr::Bool(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Binary(_)
        | Expr::Var(_)
        | Expr::VarDecl(_)
        | Expr::Operator(_)
        | Expr::DateTime(_)
        | Expr::Filepath(_, _)
        | Expr::Directory(_, _)
        | Expr::GlobPattern(_, _)
        | Expr::String(_)
        | Expr::RawString(_)
        | Expr::CellPath(_)
        | Expr::ImportPattern(_)
        | Expr::Overlay(_)
        | Expr::Signature(_)
        | Expr::Nothing
        | Expr::Garbage => {}
    }
}

fn visit_pattern(
    ws: &StateWorkingSet,
    pattern: &MatchPattern,
    in_loop: bool,
    errors: &mut Vec<ParseError>,
) {
    if let Some(guard) = &pattern.guard {
        visit(ws, guard, in_loop, errors);
    }
    match &pattern.pattern {
        Pattern::Expression(expr) => visit(ws, expr, in_loop, errors),
        Pattern::Record(fields) => {
            for (_, pattern) in fields {
                visit_pattern(ws, pattern, in_loop, errors);
            }
        }
        Pattern::List(patterns) | Pattern::Or(patterns) => {
            for pattern in patterns {
                visit_pattern(ws, pattern, in_loop, errors);
            }
        }
        Pattern::Value(_)
        | Pattern::Variable(_)
        | Pattern::Rest(_)
        | Pattern::IgnoreRest
        | Pattern::IgnoreValue
        | Pattern::Garbage => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nu_protocol::{
        Module, PipelineData, ShellError, Signature, Span, Type,
        ast::{Call, Pipeline},
        engine::{Command, EngineState, Stack},
    };
    use std::sync::Arc;

    #[derive(Clone)]
    struct Break;
    impl Command for Break {
        fn name(&self) -> &str {
            "break"
        }
        fn signature(&self) -> Signature {
            Signature::build("break")
        }
        fn description(&self) -> &str {
            "test loop control"
        }
        fn command_type(&self) -> CommandType {
            CommandType::Keyword
        }
        fn run(
            &self,
            _: &EngineState,
            _: &mut Stack,
            _: &nu_protocol::engine::Call<'_>,
            _: PipelineData,
        ) -> Result<PipelineData, ShellError> {
            unreachable!("validation must not execute commands")
        }
    }

    #[test]
    fn stored_module_initializer_is_checked_without_an_enclosing_ast_call() {
        let engine = EngineState::new();
        let mut ws = StateWorkingSet::new(&engine);
        let decl = ws.add_decl(Box::new(Break));
        let span = Span::new(10, 15);
        let mut call = Call::new(span);
        call.decl_id = decl;
        let expr = Expression::new(&mut ws, Expr::Call(Box::new(call)), span, Type::Nothing);
        let mut env = Block::new();
        env.pipelines.push(Pipeline::from_vec(vec![expr]));
        let env = ws.add_block(Arc::new(env));
        let mut module = Module::new(b"stored".to_vec());
        module.add_env_block(env);
        ws.add_module("stored", module, vec![]);
        let errors = check(&ws, &Block::new());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span(), span);
        assert!(
            errors[0]
                .to_string()
                .contains("break requires an enclosing loop")
        );
    }
}

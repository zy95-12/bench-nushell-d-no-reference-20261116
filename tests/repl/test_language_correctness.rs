use nu_test_support::nu;

#[test]
fn negative_string_operators() {
    for (source, expected) in [
        (r#"'abc' not-starts-with 'ab'"#, "false"),
        (r#"'abc' not-starts-with 'bc'"#, "true"),
        (r#"'abc' not-ends-with 'bc'"#, "false"),
        (r#"'abc' not-ends-with 'ab'"#, "true"),
        (
            r#"[abc ax bc] | where $it not-starts-with a | str join ','"#,
            "bc",
        ),
        (
            r#"[abc ax bc] | where $it not-ends-with c | str join ','"#,
            "ax",
        ),
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
        assert_eq!(result.out, expected, "{source}");
    }
    for source in ["1 not-starts-with 'a'", "'a' not-ends-with 1"] {
        assert!(!nu!(source).status.success());
    }
    let help = nu!("help operators | where operator in [not-starts-with not-ends-with] | length");
    assert_eq!(help.out, "2", "{}", help.err);
}

#[test]
fn lexical_loop_targets() {
    for source in [
        "break",
        "continue",
        "if false { break }",
        "loop { let c = { break }; break }",
        "loop { def bad [] { continue }; break }",
        "def bad [] { break }",
        "module bad { export def f [] { break } }",
    ] {
        let result = nu!(source);
        assert!(
            result.err.contains("requires an enclosing loop"),
            "{source}: {}",
            result.err
        );
    }
    for source in [
        "loop { if true { break } }",
        "for x in 1..3 { continue }",
        "let c = { loop { break } }; do $c",
        "def f [] { for x in [1 2] { if $x == 1 { continue }; break } }; f",
        "while false { continue }",
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
    }
}

#[test]
fn literals_and_expression_boundaries() {
    for (source, expected) in [
        ("2 ** 3 ** 2", "512"),
        (
            "const n = null; const xs = [1 ...$n 2]; $xs | to nuon",
            "[1, 2]",
        ),
        (
            "const n = null; const r = {a: 1 ...$n}; $r | to nuon",
            "{a: 1}",
        ),
        ("const x = ('abc' not-starts-with 'z'); $x", "true"),
        ("2 * 3 ** 2 + 1", "19"),
        ("let n = null; [1 ...$n 2] | to nuon", "[1, 2]"),
        ("let n = null; {a: 1 ...$n b: 2} | to nuon", "{a: 1, b: 2}"),
        ("(1)foo(2)", "1foo2"),
        ("let col = 'name'; [[$col]; [42]] | get name.0", "42"),
        ("[1 'x' 2] | describe", "list<any>"),
        ("[[a]; [1] ['x'] [2]] | describe", "list<any>"),
        ("for x in 1..3 { $x + 1 }", ""),
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
        assert_eq!(result.out, expected, "{source}");
    }
}

#[test]
fn external_parenthesized_arguments() {
    for (source, expected) in [
        (r#"^nu --testbin cococo pre(1 + 2)post end"#, "pre3post end"),
        (r#"^nu --testbin cococo (1 + 2)post end"#, "3post end"),
        (r#"^nu --testbin cococo (1)mid(2) end"#, "1mid2 end"),
        (
            r#"^nu --testbin cococo pre((1 + 2) * 3)post end"#,
            "pre9post end",
        ),
        (
            r#"^nu --testbin cococo 'pre(1 + 2)post' end"#,
            "pre(1 + 2)post end",
        ),
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
        assert_eq!(result.out, expected, "{source}");
    }
}

#[test]
fn annotated_values_are_checked_only_when_enabled() {
    let option = "enforce-runtime-type-annotations".to_string();
    for source in [
        "let x: int = (if true { 'bad' } else { 1 }); $x",
        "mut x: int = 1; $x = (if true { 'bad' } else { 1 }); $x",
        "let x: list<int> = (if true { [1 'bad'] } else { [1] }); $x",
        "mut x: record<a: int> = {a: 1}; $x.a = (if true { 'bad' } else { 1 }); $x",
    ] {
        let result = nu!(experimental: vec![option.clone()], source);
        assert!(
            result.err.contains("cant_convert"),
            "{source}: {}",
            result.err
        );
        assert!(nu!(source).status.success(), "ordinary mode: {source}");
    }
    for source in [
        "let x: int = (if true { 2 } else { 1 }); $x",
        "mut x: int = 1; $x = (if true { 2 } else { 1 }); $x",
        "mut x = 1; $x = (if true { 'hello' } else { 1 }); $x",
        "let x: glob = ('abc' | into string); $x",
        "let x: string = ('abc' | into glob); $x",
    ] {
        let result = nu!(experimental: vec![option.clone()], source);
        assert!(result.status.success(), "{source}: {}", result.err);
    }
}

#[test]
fn definition_scope_and_diagnostics() {
    let result = nu!("export def f [parameter: int] { $parameter }; $parameter");
    assert!(result.err.contains("Variable not found"), "{}", result.err);
    let result = nu!("def def [] {}; def f [] {}");
    assert!(!result.status.success());
    assert!(!result.err.contains("panicked"), "{}", result.err);
    assert!(
        result.err.contains("Cannot redefine the builtin def"),
        "{}",
        result.err
    );
    let result = nu!(r#""escaped\"=value""#);
    assert_eq!(result.out, "escaped\"=value", "{}", result.err);
}

#[test]
fn branch_output_redirection() {
    nu_test_support::playground::Playground::setup("language_branch_redirect", |dirs, _| {
        for condition in ["true", "false"] {
            let source = format!(
                "if {condition} {{ echo yes }} else {{ echo no }} o> result.txt; open --raw result.txt | str trim"
            );
            let result = nu!(cwd: dirs.test(), source);
            assert!(result.status.success(), "{}", result.err);
            assert_eq!(result.out, if condition == "true" { "yes" } else { "no" });
        }
    });
}

#[test]
fn row_conditions_report_type_errors_before_execution() {
    let result = nu!("print 'must not run'; [1 2] | where { 1 | str length }");
    assert!(!result.status.success());
    assert!(result.out.is_empty(), "{}", result.out);
    assert!(result.err.contains("input_type_mismatch"), "{}", result.err);
}

#[test]
fn range_loop_variable_is_typed_before_parsing_body() {
    let result = nu!("for x in 1..3 { print 'must not run'; $x + 'bad' }");
    assert!(!result.status.success());
    assert!(result.out.is_empty(), "{}", result.out);
    assert!(
        result.err.contains("operator_incompatible_types"),
        "{}",
        result.err
    );
}

#[test]
fn match_guards_validate_loop_targets_before_execution() {
    for control in ["break", "continue"] {
        for source in [
            format!("print 'EXECUTED'; match 1 {{ _ if ({control}) => 2 }}"),
            format!("print 'EXECUTED'; def bad [] {{ match 1 {{ _ if ({control}) => 2 }} }}; bad"),
            format!(
                "print 'EXECUTED'; loop {{ match 1 {{ _ if (do {{ {control} }}) => 2 }}; break }}"
            ),
        ] {
            let result = nu!(&source);
            assert!(result.out.is_empty(), "{source}: {}", result.out);
            assert!(
                result.err.contains("requires an enclosing loop"),
                "{source}: {}",
                result.err
            );
            assert!(!result.err.contains("panicked"), "{source}: {}", result.err);
        }
    }
    for (source, expected) in [
        ("loop { match 1 { _ if (break) => 2 } }; print OK", "OK"),
        (
            "for x in 1..2 { match $x { _ if (continue) => 2 }; print BAD }; print OK",
            "OK",
        ),
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
        assert_eq!(result.out, expected);
    }
}

#[test]
fn module_environment_blocks_have_independent_loop_targets() {
    for control in ["break", "continue"] {
        for source in [
            format!(
                "print 'EXECUTED'; loop {{ module bad {{ export-env {{ {control} }} }}; use bad; break }}"
            ),
            format!(
                "print 'EXECUTED'; loop {{ module outer {{ export module bad {{ export-env {{ {control} }} }} }}; use outer bad; break }}"
            ),
        ] {
            let result = nu!(&source);
            assert!(result.out.is_empty(), "{source}: {}", result.out);
            assert!(
                result.err.contains("requires an enclosing loop"),
                "{source}: {}",
                result.err
            );
            assert!(!result.err.contains("panicked"), "{source}: {}", result.err);
        }
    }
    let result = nu!(
        "loop { module good { export-env { for x in 1..2 { continue }; loop { break }; $env.LOOP_CHECK = 'OK' } }; use good; break }; print $env.LOOP_CHECK"
    );
    assert!(result.status.success(), "{}", result.err);
    assert_eq!(result.out, "OK");
}

#[test]
fn while_conditions_share_the_while_loop_target() {
    for source in [
        "while (if true { break } else { true }) { print BAD }; print OK",
        "mut n = 0; while (if $n == 0 { $n = 1; continue } else { false }) { print BAD }; print OK",
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
        assert_eq!(result.out, "OK");
    }
    for control in ["break", "continue"] {
        let source = format!("print 'EXECUTED'; while (do {{ {control}; true }}) {{ break }}");
        let result = nu!(&source);
        assert!(result.out.is_empty(), "{source}: {}", result.out);
        assert!(
            result.err.contains("requires an enclosing loop"),
            "{source}: {}",
            result.err
        );
    }
}

#[test]
fn loop_validation_visits_nested_expression_children() {
    for source in [
        "print 'EXECUTED'; ^nu --testbin cococo (break)",
        "print 'EXECUTED'; echo x o> (continue)",
        "print 'EXECUTED'; let xs = [(break)]; $xs",
        "print 'EXECUTED'; let r = {a: (continue)}; $r",
        "print 'EXECUTED'; $'before(break)after'",
        "print 'EXECUTED'; for x in (break) { print BAD }",
    ] {
        let result = nu!(source);
        assert!(result.out.is_empty(), "{source}: {}", result.out);
        assert!(
            result.err.contains("requires an enclosing loop"),
            "{source}: {}",
            result.err
        );
    }
}

#[test]
fn literal_catch_handlers_keep_the_enclosing_loop_target() {
    for source in [
        "loop { try { error make {msg: 'stop'} } catch { break }; print BAD }; print OK",
        "for x in 1..2 { try { error make {msg: 'stop'} } catch { continue }; print BAD }; print OK",
    ] {
        let result = nu!(source);
        assert!(result.status.success(), "{source}: {}", result.err);
        assert_eq!(result.out, "OK");
    }
    let result = nu!(
        "print 'EXECUTED'; loop { let handler = { break }; try { error make {msg: 'stop'} } catch $handler; break }"
    );
    assert!(result.out.is_empty(), "{}", result.out);
    assert!(
        result.err.contains("requires an enclosing loop"),
        "{}",
        result.err
    );
}

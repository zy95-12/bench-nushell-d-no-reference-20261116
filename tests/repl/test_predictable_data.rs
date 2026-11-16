use nu_test_support::nu;

fn output(source: &str, expected: &str) {
    let result = nu!(collapse_output: false, source);
    assert!(result.status.success(), "{source}: {}", result.err);
    assert_eq!(result.out.trim_end(), expected, "{source}");
}

#[test]
fn unicode_length_modes_in_runtime_and_const() {
    for (mode, expected) in [
        ("", "6"),
        ("--chars", "5"),
        ("-c", "5"),
        ("--grapheme-clusters", "5"),
    ] {
        output(&format!("'hällo' | str length {mode}"), expected);
        output(
            &format!("const n = ('hällo' | str length {mode}); $n"),
            expected,
        );
    }
    output("'é' | str length -c", "2");
    output("'é' | str length -g", "1");
    for source in [
        "'a' | str length -c -g",
        "const n = ('a' | str length -c -g)",
    ] {
        let result = nu!(source);
        assert!(
            result.err.contains("incompatible_parameters"),
            "{}",
            result.err
        );
    }
}

#[test]
fn case_insensitive_access_keeps_optional_and_exact_defaults() {
    output("{Name: {Value: 42}} | get --ignore-case name.value", "42");
    output("const x = ({Name: 42} | get --ignore-case name); $x", "42");
    output(
        "const x = ({} | get --optional missing); $x == null",
        "true",
    );
    output(
        "{Name: 42} | get --ignore-case missing? | $in == null",
        "true",
    );
    output("{Name: 42} | select --ignore-case name | get name", "42");
    output(
        "{Name: 42 Other: 1} | reject --ignore-case name | columns | str join ','",
        "Other",
    );
    output(
        "{Name: 42} | reject --ignore-case missing? | get Name",
        "42",
    );
    assert!(!nu!("{Name: 42} | get name").status.success());
    assert!(!nu!("{Name: 42} | select name").status.success());
    assert!(!nu!("{Name: 42} | reject name").status.success());
}

#[test]
fn flatten_only_demands_the_requested_closure_results() {
    // Collecting the inner infinite range before yielding would never finish.
    output(
        "[1] | each --flatten { 1.. } | first 3 | to nuon",
        "[1, 2, 3]",
    );
    output("null | each { error make {msg: unexpected} } | length", "0");
    output(
        "[1 2] | each --flatten {|n| [$n $n] } | to nuon",
        "[1, 1, 2, 2]",
    );
    let result = nu!("[1] | each { error make {msg: nested_failure} } | each { 42 } | each { 99 }");
    assert!(result.err.contains("nested_failure"), "{}", result.err);
    let nested = nu!("[1] | each { [2] | each { error make {msg: deeply_nested} } } | each { 99 }");
    assert!(nested.err.contains("deeply_nested"), "{}", nested.err);
}

#[test]
fn loop_control_removes_try_handlers() {
    for keyword in ["break", "continue"] {
        output(
            &format!(
                "try {{ for i in 1..2 {{ try {{ {keyword} }} catch {{ error make {{msg: stale_handler}} }} }}; error make {{msg: outer}} }} catch {{ true }}"
            ),
            "true",
        );
    }
}

#[test]
fn overlays_include_hidden_and_scoped_modules() {
    output(
        "module sample { export def answer [] { 42 } }; overlay use sample; overlay hide sample; overlay list | where name == sample | get 0.active",
        "false",
    );
    output(
        "module sample { export def answer [] { 42 } }; do { overlay use sample; answer }",
        "42",
    );
    output(
        "module sample { export def answer [] { 42 } }; overlay use sample; overlay list | where name == sample | get 0.active",
        "true",
    );
    output(
        "do { module scoped {}; overlay use scoped; overlay hide scoped; overlay list | where name == scoped | get 0.active }",
        "false",
    );
}

#[test]
fn duplicate_columns_and_command_namespaces() {
    output(
        "\"a a a_1\\n1 2 3\" | detect columns | columns | uniq | length",
        "3",
    );
    output("which | where command == 'str length' | length", "1");
    output("def 'orphan leaf' [] { 42 }; orphan leaf", "42");
    output(
        "def real [] { 1 }; def 'real leaf' [] { 2 }; [ (real) (real leaf) ] | to nuon",
        "[1, 2]",
    );
    output(
        "def 'orphan leaf' [] { 42 }; scope commands | where name == orphan | length",
        "1",
    );
    output(
        "module m { export def 'imported leaf' [] { 42 } }; use m *; scope commands | where name == imported | length",
        "1",
    );
    output(
        "def imported [] { 1 }; module m { export def 'imported leaf' [] { 42 } }; use m *; imported",
        "1",
    );
}

#[test]
fn serialization_and_metadata() {
    output("'a=b' | to nuon", "\"a=b\"");
    output(
        "1 | into binary --endian big | encode hex",
        "0000000000000001",
    );
    output(
        "1 | into binary --endian little | encode hex",
        "0100000000000000",
    );
    output("1 | into binary --endian big --compact | encode hex", "01");
    output("$.a.0 | into string", "$.a.0");
    output("[{a: 'x|y'}] | to md", "|a|\n|---|\n|x\\|y|");
    output("[{a: \"x\\ny\"}] | to md", "|a|\n|---|\n|x<br>y|");
    output("[{a: '*x*'}] | to md --escape", "|a|\n|---|\n|\\*x\\*|");
    output(
        "[1 2] | metadata set --content-type application/json | first 1 | metadata | get content_type? | $in == null",
        "true",
    );
    output(
        "$env.config.table.stream_page_size = 2; $env.config.table.stream_batch_duration = 10ms; $env.config.table.stream_page_size",
        "2",
    );
    output(
        "scope commands | where name == get | get 0.examples.0.result | metadata | get span.start | $in < 10000",
        "true",
    );
}

#[test]
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn relative_paths_remain_case_sensitive() {
    assert!(!nu!("'/Foo/bar' | path relative-to '/foo'").status.success());
    output("'/foo/Bar' | path relative-to '/foo'", "Bar");
}

#[test]
fn partial_byte_streams_reset_document_metadata() {
    nu_test_support::playground::Playground::setup("partial_stream_metadata", |dirs, sandbox| {
        sandbox.with_files(&[nu_test_support::fs::Stub::FileWithContent(
            "input.bin",
            "abcdef",
        )]);
        for filter in ["first 1", "skip 1", "take 1"] {
            let source = format!(
                "open --raw input.bin | metadata set --content-type application/json | {filter} | metadata | get content_type? | $in == null"
            );
            let result = nu!(cwd: dirs.test(), &source);
            assert!(result.status.success(), "{source}: {}", result.err);
            assert_eq!(result.out, "true", "{source}");
        }
    });
}

#[test]
#[cfg(any(windows, target_os = "macos"))]
fn relative_paths_fall_back_after_exact_matching() {
    output("'Foo/Bar/Baz' | path relative-to 'foo/bar'", "Baz");
    output("'Foo/Bar/Baz' | path relative-to 'Foo/Bar'", "Baz");
}

#[test]
#[cfg(windows)]
fn windows_devices_work_without_globbing_or_canonicalizing() {
    output(r"open --raw '\\.\NUL' | bytes length", "0");
    output(r"'test' | save --append '\\.\NUL'; true", "true");
    output(r"source '\\.\NUL'; true", "true");
}

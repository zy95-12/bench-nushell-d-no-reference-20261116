use crate::Query;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{Category, LabeledError, Record, Signature, Span, Spanned, SyntaxShape, Value};
use sxd_document::parser;
use sxd_xpath::{Context, Factory, nodeset::Node};

pub struct QueryXml;

impl SimplePluginCommand for QueryXml {
    type Plugin = Query;

    fn name(&self) -> &str {
        "query xml"
    }

    fn description(&self) -> &str {
        "execute xpath query on xml"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required("query", SyntaxShape::String, "xpath query")
            .named(
                "namespaces",
                SyntaxShape::Record(vec![]),
                "map of prefixes to namespace URIs",
                Some('n'),
            )
            .switch("output-string-value", "Include node string values", None)
            .switch("output-type", "Include node types", None)
            .switch(
                "output-names",
                "Include local names, namespaces and prefixed names",
                None,
            )
            .category(Category::Filters)
    }

    fn run(
        &self,
        _plugin: &Query,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: &Value,
    ) -> Result<Value, LabeledError> {
        let query: Option<Spanned<String>> = call.opt(0)?;
        let namespaces: Option<Record> = call.get_flag::<Record>("namespaces")?;

        execute_xpath_query(call, input, query, namespaces)
    }
}

pub fn execute_xpath_query(
    call: &EvaluatedCall,
    input: &Value,
    query: Option<Spanned<String>>,
    namespaces: Option<Record>,
) -> Result<Value, LabeledError> {
    let (query_string, span) = match &query {
        Some(v) => (&v.item, v.span),
        None => {
            return Err(
                LabeledError::new("problem with input data").with_label("query missing", call.head)
            );
        }
    };

    let output_type = call.has_flag("output-type")?;
    let output_names = call.has_flag("output-names")?;
    let output_string_value =
        call.has_flag("output-string-value")? || !(output_type || output_names);

    let xpath = build_xpath(query_string, span)?;
    let input_string = input.coerce_str()?;
    let package = parser::parse(&input_string);

    if let Err(err) = package {
        return Err(
            LabeledError::new("Invalid XML document").with_label(err.to_string(), input.span())
        );
    }

    let package = package.expect("invalid xml document");

    let document = package.as_document();
    let mut context = Context::new();

    for (prefix, uri) in namespaces.unwrap_or_default().into_iter() {
        context.set_namespace(prefix.as_str(), uri.into_string()?.as_str());
    }

    // The reserved XML prefix is available even without an explicit mapping.
    context.set_namespace("xml", "http://www.w3.org/XML/1998/namespace");

    let result = xpath.evaluate(&context, document.root()).map_err(|err| {
        LabeledError::new("xpath query error").with_label(err.to_string(), call.head)
    })?;
    let optional_name = |name: Option<&str>| {
        name.map(|name| Value::string(name, call.head))
            .unwrap_or_else(|| Value::nothing(call.head))
    };

    Ok(match result {
        sxd_xpath::Value::Boolean(value) => Value::bool(value, call.head),
        sxd_xpath::Value::Number(value) => Value::float(value, call.head),
        sxd_xpath::Value::String(value) => Value::string(value, call.head),
        sxd_xpath::Value::Nodeset(nodes) => Value::list(
            nodes
                .document_order()
                .into_iter()
                .map(|node| {
                    let mut record = Record::new();
                    if output_string_value {
                        record.push(
                            "string_value",
                            Value::string(node.string_value(), call.head),
                        );
                    }
                    if output_type {
                        let node_type = match node {
                            Node::Element(_) => "element",
                            Node::Attribute(_) => "attribute",
                            Node::Text(_) => "text",
                            Node::Comment(_) => "comment",
                            Node::ProcessingInstruction(_) => "processing_instruction",
                            Node::Root(_) => "root",
                            Node::Namespace(_) => "namespace",
                        };
                        record.push("type", Value::string(node_type, call.head));
                    }
                    if output_names {
                        let name = node.expanded_name();
                        let namespace = match node {
                            Node::Namespace(namespace) => Some(namespace.uri()),
                            _ => name.and_then(|name| name.namespace_uri()),
                        };
                        record.push(
                            "local_name",
                            optional_name(name.map(|name| name.local_part())),
                        );
                        record.push("namespace", optional_name(namespace));
                        record.push(
                            "prefixed_name",
                            optional_name(node.prefixed_name().as_deref()),
                        );
                    }
                    Value::record(record, call.head)
                })
                .collect(),
            call.head,
        ),
    })
}

fn build_xpath(xpath_str: &str, span: Span) -> Result<sxd_xpath::XPath, LabeledError> {
    let factory = Factory::new();

    match factory.build(xpath_str) {
        Ok(xpath) => xpath.ok_or_else(|| {
            LabeledError::new("invalid xpath query").with_label("the query must not be empty", span)
        }),
        Err(err) => Err(LabeledError::new("invalid xpath query").with_label(err.to_string(), span)),
    }
}

#[cfg(test)]
mod tests {
    use super::execute_xpath_query as query;
    use nu_plugin::EvaluatedCall;
    use nu_protocol::{Span, Spanned, Value, record};

    #[test]
    fn position_function_in_predicate() {
        let call = EvaluatedCall {
            head: Span::test_data(),
            positional: vec![],
            named: vec![],
        };

        let text = Value::string(
            r#"<?xml version="1.0" encoding="UTF-8"?><a><b/><b/></a>"#,
            Span::test_data(),
        );

        let spanned_str: Spanned<String> = Spanned {
            item: "count(//a/*[position() = 2])".to_string(),
            span: Span::test_data(),
        };

        let actual = query(&call, &text, Some(spanned_str), None).expect("test should not fail");

        let expected = Value::test_float(1.0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn functions_implicitly_coerce_argument_types() {
        let call = EvaluatedCall {
            head: Span::test_data(),
            positional: vec![],
            named: vec![],
        };

        let text = Value::string(
            r#"<?xml version="1.0" encoding="UTF-8"?><a>true</a>"#,
            Span::test_data(),
        );

        let spanned_str: Spanned<String> = Spanned {
            item: "count(//*[contains(., true)])".to_string(),
            span: Span::test_data(),
        };

        let actual = query(&call, &text, Some(spanned_str), None).expect("test should not fail");

        let expected = Value::test_float(1.0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn namespaces_are_used() {
        let call = EvaluatedCall {
            head: Span::test_data(),
            positional: vec![],
            named: vec![],
        };

        // document uses `dp` ("document prefix") as a prefix
        let text = Value::string(
            r#"<?xml version="1.0" encoding="UTF-8"?><a xmlns:dp="http://example.com/ns"><dp:b>yay</dp:b></a>"#,
            Span::test_data(),
        );

        // but query uses `qp` ("query prefix") as a prefix
        let namespaces = record! {
            "qp" => Value::string("http://example.com/ns", Span::test_data()),
        };

        let spanned_str: Spanned<String> = Spanned {
            item: "//qp:b/text()".to_string(),
            span: Span::test_data(),
        };

        let actual =
            query(&call, &text, Some(spanned_str), Some(namespaces)).expect("test should not fail");

        let expected = Value::list(
            vec![Value::test_record(record! {
                "string_value" => Value::string("yay", Span::test_data()),
            })],
            Span::test_data(),
        );

        // and yet it should work regardless
        assert_eq!(actual, expected);
    }
    fn evaluate(xml: &str, xpath: &str, flags: &[&str]) -> Value {
        let mut call = EvaluatedCall::new(Span::test_data());
        for flag in flags {
            call = call.with_flag(Spanned {
                item: *flag,
                span: Span::test_data(),
            });
        }
        query(
            &call,
            &Value::test_string(xml),
            Some(Spanned {
                item: xpath.into(),
                span: Span::test_data(),
            }),
            None,
        )
        .expect("valid XPath")
    }

    #[test]
    fn scalar_results_are_native_even_with_output_flags() {
        for flags in [vec![], vec!["output-type", "output-names"]] {
            assert_eq!(evaluate("<a/>", "true()", &flags), Value::test_bool(true));
            assert_eq!(evaluate("<a/>", "false()", &flags), Value::test_bool(false));
            assert_eq!(evaluate("<a/>", "1 div 2", &flags), Value::test_float(0.5));
            assert_eq!(
                evaluate("<a/>", "string('hello')", &flags),
                Value::test_string("hello")
            );
        }
    }

    #[test]
    fn nodesets_have_fixed_columns_and_document_order() {
        let expected = Value::test_list(vec![
            Value::test_record(record! { "string_value" => Value::test_string("one") }),
            Value::test_record(record! { "string_value" => Value::test_string("two") }),
        ]);
        for xpath in [
            "//b",
            "/a/child::b[position() >= 1]",
            "//b[ contains('éééééééééé', 'é') ]",
        ] {
            assert_eq!(
                evaluate("<a><b>one</b><b>two</b></a>", xpath, &[]),
                expected
            );
        }
        assert_eq!(evaluate("<a/>", "//missing", &[]), Value::test_list(vec![]));
    }

    #[test]
    fn output_flags_select_only_requested_columns_in_fixed_order() {
        // Exhaust every nonempty combination, including names without string values.
        for mask in 1..8 {
            let mut flags = vec![];
            let mut expected = nu_protocol::Record::new();
            if mask & 1 != 0 {
                flags.push("output-string-value");
                expected.push("string_value", Value::test_string("hello"));
            }
            if mask & 2 != 0 {
                flags.push("output-type");
                expected.push("type", Value::test_string("element"));
            }
            if mask & 4 != 0 {
                flags.push("output-names");
                expected.push("local_name", Value::test_string("b"));
                expected.push("namespace", Value::test_string("urn:test"));
                expected.push("prefixed_name", Value::test_string("p:b"));
            }
            flags.reverse();
            let actual = evaluate("<a xmlns:p='urn:test'><p:b>hello</p:b></a>", "/a/*", &flags);
            let row = actual.as_list().unwrap()[0].as_record().unwrap();
            assert_eq!(
                row.columns().collect::<Vec<_>>(),
                expected.columns().collect::<Vec<_>>()
            );
            assert_eq!(actual, Value::test_list(vec![Value::test_record(expected)]));
        }
    }

    #[test]
    fn node_types_and_optional_names() {
        let xml = "<a xmlns:p='urn:test' id='x'>text<!--comment--><?target data?></a>";
        for (xpath, kind, local, namespace, prefixed) in [
            ("/", "root", None, None, None),
            ("/a", "element", Some("a"), None, Some("a")),
            ("/a/@id", "attribute", Some("id"), None, Some("id")),
            ("/a/text()", "text", None, None, None),
            ("/a/comment()", "comment", None, None, None),
            (
                "/a/processing-instruction()",
                "processing_instruction",
                Some("target"),
                None,
                Some("target"),
            ),
            (
                "/a/namespace::p",
                "namespace",
                Some("p"),
                Some("urn:test"),
                Some("p"),
            ),
        ] {
            let name = |value: Option<&str>| {
                value
                    .map(Value::test_string)
                    .unwrap_or_else(Value::test_nothing)
            };
            assert_eq!(
                evaluate(xml, xpath, &["output-names", "output-type"]),
                Value::test_list(vec![Value::test_record(record! {
                    "type" => Value::test_string(kind),
                    "local_name" => name(local),
                    "namespace" => name(namespace),
                    "prefixed_name" => name(prefixed),
                })]),
                "{xpath}"
            );
        }
    }

    #[test]
    fn xml_namespace_is_automatically_available() {
        assert_eq!(
            evaluate("<a xml:lang='en'/>", "string(/a/@xml:lang)", &[]),
            Value::test_string("en")
        );
        assert_eq!(
            evaluate("<a xml:lang='en'/>", "/a/@xml:lang", &["output-names"]),
            Value::test_list(vec![Value::test_record(record! {
                "local_name" => Value::test_string("lang"),
                "namespace" => Value::test_string("http://www.w3.org/XML/1998/namespace"),
                "prefixed_name" => Value::test_string("xml:lang"),
            })])
        );
    }
}

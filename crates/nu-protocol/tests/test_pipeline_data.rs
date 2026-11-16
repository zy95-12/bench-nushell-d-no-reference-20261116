use nu_protocol::{IntoPipelineData, Span, Value};

#[test]
fn test_convert_pipeline_data_to_value() {
    // Setup PipelineData
    let value_val = 10;
    let value = Value::int(value_val, Span::new(1, 3));
    let pipeline_data = value.into_pipeline_data();

    // Test that conversion into Value is correct
    let new_span = Span::new(5, 6);
    let converted_value = pipeline_data.into_value(new_span);

    assert_eq!(converted_value, Ok(Value::int(value_val, new_span)));
}

// Exercise the public API as an external caller, including const evaluation.
const EMPTY: nu_protocol::PipelineData = nu_protocol::PipelineData::empty();

#[test]
fn empty_constructor_is_const_and_has_no_metadata_or_values() {
    use nu_protocol::PipelineData;
    assert!(matches!(EMPTY, PipelineData::Empty));
    assert_eq!(EMPTY.metadata(), None);
    assert_eq!(EMPTY.span(), None);
    assert_eq!(EMPTY.into_iter().count(), 0);
}

#[test]
fn constructors_preserve_payload_span_and_all_metadata_forms() {
    use nu_protocol::{
        ByteStream, ByteStreamType, DataSource, ListStream, PipelineData, PipelineMetadata, Signals,
    };
    let span = Span::new(12, 24);
    let metadata = PipelineMetadata::default()
        .with_data_source(DataSource::FilePath("input.txt".into()))
        .with_content_type(Some("text/plain".into()));
    let value = || Value::string("payload", span);
    let list = || ListStream::new(vec![value()].into_iter(), span, Signals::empty());
    let bytes = || {
        ByteStream::from_iter(
            [&b"pay"[..], &b"load"[..]],
            span,
            Signals::empty(),
            ByteStreamType::String,
        )
    };

    // Each constructor must accept an unannotated None, Some, and bare metadata.
    let cases = [
        (PipelineData::value(value(), None), None),
        (
            PipelineData::value(value(), Some(metadata.clone())),
            Some(metadata.clone()),
        ),
        (
            PipelineData::value(value(), metadata.clone()),
            Some(metadata.clone()),
        ),
        (PipelineData::list_stream(list(), None), None),
        (
            PipelineData::list_stream(list(), Some(metadata.clone())),
            Some(metadata.clone()),
        ),
        (
            PipelineData::list_stream(list(), metadata.clone()),
            Some(metadata.clone()),
        ),
        (PipelineData::byte_stream(bytes(), None), None),
        (
            PipelineData::byte_stream(bytes(), Some(metadata.clone())),
            Some(metadata.clone()),
        ),
        (
            PipelineData::byte_stream(bytes(), metadata.clone()),
            Some(metadata.clone()),
        ),
    ];
    for (index, (data, expected_metadata)) in cases.into_iter().enumerate() {
        assert_eq!(data.metadata(), expected_metadata);
        assert_eq!(data.span(), Some(span));
        // Public variants remain usable by downstream pattern matching.
        match (&data, index / 3) {
            (PipelineData::Value(_, md), 0)
            | (PipelineData::ListStream(_, md), 1)
            | (PipelineData::ByteStream(_, md), 2) => assert_eq!(md, &expected_metadata),
            _ => panic!("constructor changed the payload kind"),
        }
        let expected = if index / 3 == 1 {
            Value::list(vec![value()], span)
        } else {
            value()
        };
        assert_eq!(data.into_value(span).unwrap(), expected);
    }
}

#[test]
fn value_constructor_accepts_into_value() {
    struct Wrapped(Value);
    impl From<Wrapped> for Value {
        fn from(value: Wrapped) -> Self {
            value.0
        }
    }
    let expected = Value::test_int(42);
    let data = nu_protocol::PipelineData::value(Wrapped(expected.clone()), None);
    assert_eq!(data.into_value(Span::test_data()).unwrap(), expected);
}

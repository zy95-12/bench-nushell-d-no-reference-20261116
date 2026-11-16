use nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct OverlayList;

impl Command for OverlayList {
    fn name(&self) -> &str {
        "overlay list"
    }

    fn description(&self) -> &str {
        "List all overlays and their active status."
    }

    fn signature(&self) -> nu_protocol::Signature {
        Signature::build("overlay list")
            .category(Category::Core)
            .input_output_types(vec![(Type::Nothing, Type::table())])
    }

    fn extra_description(&self) -> &str {
        "The overlays are listed in registration order, including hidden overlays."
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let mut names = engine_state
            .scope
            .overlays
            .iter()
            .map(|(name, _)| String::from_utf8_lossy(name).into_owned())
            .collect::<Vec<_>>();
        for name in &stack.overlay_names {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        let overlays = names
            .into_iter()
            .map(|name| {
                let active = stack.active_overlays.contains(&name);
                Value::record(
                    record! {
                        "name" => Value::string(name, call.head),
                        "active" => Value::bool(active, call.head),
                    },
                    call.head,
                )
            })
            .collect();

        Ok(Value::list(overlays, call.head).into_pipeline_data())
    }

    fn examples(&self) -> Vec<Example> {
        vec![Example {
            description: "Get the last activated overlay",
            example: r#"module spam { export def foo [] { "foo" } }
    overlay use spam
    overlay list | last | get name"#,
            result: Some(Value::test_string("spam")),
        }]
    }
}

use crate::*;
use nu_protocol::engine::{EngineState, StateWorkingSet};

pub fn create_default_context() -> EngineState {
    create_default_context_with(&[])
}

/// Create the language context, then apply additional command contexts in order.
///
/// Callers choose their layers so tests and embedded engines can retain their
/// existing command sets without depending on the full shell.
pub fn create_default_context_with(contexts: &[fn(EngineState) -> EngineState]) -> EngineState {
    let mut engine_state = EngineState::new();

    let delta = {
        let mut working_set = StateWorkingSet::new(&engine_state);

        macro_rules! bind_command {
            ( $( $command:expr ),* $(,)? ) => {
                $( working_set.add_decl(Box::new($command)); )*
            };
        }

        // Core
        bind_command! {
            Alias,
            AttrCategory,
            AttrDeprecated,
            AttrExample,
            AttrSearchTerms,
            Break,
            Collect,
            Const,
            Continue,
            Def,
            Describe,
            Do,
            Echo,
            ErrorMake,
            ExportAlias,
            ExportCommand,
            ExportConst,
            ExportDef,
            ExportExtern,
            ExportUse,
            ExportModule,
            Extern,
            For,
            Hide,
            HideEnv,
            If,
            Ignore,
            Overlay,
            OverlayUse,
            OverlayList,
            OverlayNew,
            OverlayHide,
            Let,
            Loop,
            Match,
            Module,
            Mut,
            Return,
            Scope,
            ScopeAliases,
            ScopeCommands,
            ScopeEngineStats,
            ScopeExterns,
            ScopeModules,
            ScopeVariables,
            Try,
            Use,
            Version,
            While,
        };

        working_set.render()
    };

    if let Err(err) = engine_state.merge_delta(delta) {
        eprintln!("Error creating default context: {err:?}");
    }

    contexts
        .iter()
        .fold(engine_state, |state, add_context| add_context(state))
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use nu_protocol::Value;

    #[test]
    fn context_layers_share_language_commands_and_run_in_order() {
        fn first(mut state: EngineState) -> EngineState {
            assert!(state.find_decl(b"echo", &[]).is_some());
            state.add_env_var("CONTEXT_LAYER".into(), Value::test_int(1));
            state
        }
        fn second(mut state: EngineState) -> EngineState {
            assert_eq!(
                state.get_env_var("CONTEXT_LAYER"),
                Some(&Value::test_int(1))
            );
            state.add_env_var("CONTEXT_LAYER".into(), Value::test_int(2));
            state
        }
        let plain = create_default_context();
        let empty_layers = create_default_context_with(&[]);
        assert_eq!(
            plain.get_decls_sorted(true),
            empty_layers.get_decls_sorted(true)
        );
        let layered = create_default_context_with(&[first, second]);
        assert_eq!(layered.get_decls_sorted(true), plain.get_decls_sorted(true));
        assert_eq!(
            layered.get_env_var("CONTEXT_LAYER"),
            Some(&Value::test_int(2))
        );
    }
}

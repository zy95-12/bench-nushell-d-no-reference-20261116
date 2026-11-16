use crate::{ShellError, Span, Type, Value};

#[derive(Clone, Debug)]
pub struct Variable {
    pub declaration_span: Span,
    pub ty: Type,
    pub type_annotation: Option<Type>,
    pub mutable: bool,
    pub const_val: Option<Value>,
}

impl Variable {
    pub fn new(declaration_span: Span, ty: Type, mutable: bool) -> Variable {
        Self {
            declaration_span,
            ty,
            type_annotation: None,
            mutable,
            const_val: None,
        }
    }
}

impl Variable {
    /// Validate only explicit annotations, leaving ordinary inferred variables unchanged.
    pub fn validate_assignment(&self, value: &Value, span: Span) -> Result<(), ShellError> {
        if nu_experimental::ENFORCE_RUNTIME_TYPE_ANNOTATIONS.get() {
            if let Some(ty) = &self.type_annotation {
                if !value.is_subtype_of(ty) {
                    return Err(ShellError::CantConvert {
                        to_type: ty.to_string(),
                        from_type: value.get_type().to_string(),
                        span,
                        help: Some(format!(
                            "Value does not match the variable's {ty} annotation"
                        )),
                    });
                }
            }
        }
        Ok(())
    }
}

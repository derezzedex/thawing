#[derive(Debug, Clone)]
pub struct Id(pub(crate) iced_core::widget::Id);

impl Id {
    pub fn new(id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self(iced_core::widget::Id::new(id))
    }

    pub fn unique() -> Self {
        Self(iced_core::widget::Id::unique())
    }
}

impl From<iced_core::widget::Id> for Id {
    fn from(id: iced_core::widget::Id) -> Self {
        Self(id)
    }
}

impl From<Id> for iced_core::widget::Id {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl From<&'static str> for Id {
    fn from(value: &'static str) -> Self {
        Id::new(value)
    }
}

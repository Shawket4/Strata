use vault_format::FrontmatterError;

/// Why an item could not be rendered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// The existing frontmatter cannot be edited (invalid YAML or an unsupported layout).
    #[error("the frontmatter cannot be edited: {0}")]
    Unreadable(FrontmatterError),
    /// A property could not be set.
    #[error("the `{key}` property could not be set: {source}")]
    Property {
        /// The property.
        key: String,
        /// Why.
        source: FrontmatterError,
    },
}

impl RenderError {
    pub(crate) fn property(key: &str) -> impl FnOnce(FrontmatterError) -> Self + '_ {
        move |source| Self::Property {
            key: key.to_owned(),
            source,
        }
    }
}

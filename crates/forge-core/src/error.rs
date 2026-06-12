use snafu::Snafu;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("{source}"))]
    ComponentGenerator {
        source: forge_component_generator::Error,
    },

    #[snafu(display("{source}"))]
    ProjectGenerator {
        source: forge_project_generator::Error,
    },
}

impl From<forge_component_generator::Error> for Error {
    fn from(source: forge_component_generator::Error) -> Self {
        Self::ComponentGenerator { source }
    }
}

impl From<forge_project_generator::Error> for Error {
    fn from(source: forge_project_generator::Error) -> Self {
        Self::ProjectGenerator { source }
    }
}

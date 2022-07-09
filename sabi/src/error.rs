#[derive(Debug, Clone)]
pub enum SabiError {
    NoSocketAddr,
}

impl std::error::Error for SabiError {}

impl std::fmt::Display for SabiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            &Self::NoSocketAddr => write!(f, "no socket addr found"),
        }
    }
}

/// Configuration for a bot connection.
#[derive(Debug, Clone)]
pub struct Config {
    pub address:  String,
    pub username: String,
    pub password: String,
    pub lang:     String,
}

impl Config {
    pub fn new(
        address:  impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            address:  address.into(),
            username: username.into(),
            password: password.into(),
            lang:     "en".into(),
        }
    }
}

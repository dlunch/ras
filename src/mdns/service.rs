pub struct Service {
    pub r#type: &'static str,
    pub name: &'static str,
    pub port: u16,
    pub txt: Vec<&'static str>,
}

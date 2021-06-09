use log::debug;

pub struct Service {
    pub(super) r#type: String,
    pub(super) name: String,
    pub(super) port: u16,
    pub(super) txt: Vec<String>,
}

impl Service {
    pub fn new(r#type: &str, name: &str, port: u16, txt: Vec<&str>) -> Self {
        let r#type = format!("{}.local", r#type);
        let name = format!("{}.{}", name, r#type);

        debug!("New service {} {}", r#type, name);

        Self {
            r#type,
            name,
            port,
            txt: txt.into_iter().map(|x| x.into()).collect::<Vec<_>>(),
        }
    }
}

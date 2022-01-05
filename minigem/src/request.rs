use crate::{connection::Connection, Error, Response};
use url::Url;

pub struct Request {
    pub(crate) url: String,
}

impl Request {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    pub fn send(self) -> Result<Response, Error> {
        let url = Url::parse(&self.url)?;

        if url.scheme() != "gemini" {
            return Err(Error::BadScheme);
        }

        if url.host().is_none() {
            return Err(Error::BadHost);
        }

        Connection::new(self).send()
    }
}

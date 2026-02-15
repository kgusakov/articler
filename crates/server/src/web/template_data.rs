use db::repository::{Id, clients::ClientRow};
use serde::Serialize;

#[derive(Serialize)]
pub(in crate::web) struct Client {
    id: Id,
    client_id: String,
    client_name: String,
    client_secret: String,
}

impl From<ClientRow> for Client {
    fn from(value: ClientRow) -> Self {
        Client {
            id: value.id,
            client_id: value.client_id,
            client_name: value.name,
            client_secret: value.client_secret,
        }
    }
}

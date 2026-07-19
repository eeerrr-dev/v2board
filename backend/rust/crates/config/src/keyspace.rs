use uuid::Uuid;

/// Installation-bound Redis key namespace shared by the API and worker.
///
/// Redis is deliberately disposable, but sharing one Redis service or logical
/// database must never let two native installations read, delete, or lock each
/// other's state. The immutable PostgreSQL installation identity supplies that
/// boundary without adding an operator-controlled alias or compatibility key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedisKeyspace {
    prefix: String,
}

impl RedisKeyspace {
    pub fn new(installation_id: Uuid) -> Self {
        Self {
            prefix: format!("v2board:{installation_id}:"),
        }
    }

    pub fn key(&self, logical_key: &str) -> String {
        format!("{}{logical_key}", self.prefix)
    }

    pub fn pattern(&self, logical_pattern: &str) -> String {
        self.key(logical_pattern)
    }
}

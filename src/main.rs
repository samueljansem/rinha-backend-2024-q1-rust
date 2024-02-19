use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::RwLock;

#[derive(Default, Clone)]
struct Client {
    balance: i64,
    limit: i64,
    transactions: RingBuffer<Transaction>,
}

impl Client {
    pub fn with_limit(limit: i64) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }

    pub fn transact(&mut self, transaction: Transaction) -> Result<(), &'static str> {
        match transaction.r#type {
            TransactionType::CREDIT => {
                self.balance += transaction.value;
                self.transactions.push(transaction);
                Ok(())
            }
            TransactionType::DEBIT => {
                if self.balance + self.limit >= transaction.value {
                    self.balance -= transaction.value;
                    self.transactions.push(transaction);
                    Ok(())
                } else {
                    Err("Saldo insuficiente")
                }
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct Description(String);

impl TryFrom<String> for Description {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > 10 {
            Err("Descrição inválida")
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(Clone, Serialize)]
struct RingBuffer<T>(VecDeque<T>);

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::with_capacity(10)
    }
}

impl<T> RingBuffer<T> {
    fn with_capacity(size: usize) -> Self {
        Self(VecDeque::with_capacity(size))
    }

    fn push(&mut self, item: T) {
        if self.0.len() == self.0.capacity() {
            self.0.pop_back();
            self.0.push_front(item);
        } else {
            self.0.push_front(item);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
enum TransactionType {
    #[serde(rename = "d")]
    DEBIT,

    #[serde(rename = "c")]
    CREDIT,
}

#[derive(Clone, Serialize, Deserialize)]
struct Transaction {
    #[serde(rename = "valor")]
    value: i64,

    #[serde(rename = "tipo")]
    r#type: TransactionType,

    #[serde(rename = "descricao")]
    description: Description,

    #[serde(
        rename = "realizada_em",
        with = "time::serde::rfc3339",
        default = "OffsetDateTime::now_utc"
    )]
    created_at: OffsetDateTime,
}

type AppState = Arc<HashMap<u8, RwLock<Client>>>;

#[tokio::main]
async fn main() {
    let clients = HashMap::<u8, RwLock<Client>>::from_iter([
        (1, RwLock::new(Client::with_limit(100_000))),
        (2, RwLock::new(Client::with_limit(80_000))),
        (3, RwLock::new(Client::with_limit(1_000_000))),
        (4, RwLock::new(Client::with_limit(10_000_000))),
        (5, RwLock::new(Client::with_limit(500_000))),
    ]);

    let app = Router::new()
        .route("/clientes/:id/extrato", get(get_statement))
        .route("/clientes/:id/transacoes", post(create_transaction))
        .with_state(Arc::new(clients));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn get_statement(Path(id): Path<u8>, State(state): State<AppState>) -> impl IntoResponse {
    match state.get(&id) {
        Some(client) => {
            let client = client.read().await;
            Ok(Json(json!(
                {
                    "saldo": {
                        "total": client.balance,
                        "limite": client.limit,
                        "data_extrato": OffsetDateTime::now_utc().format(&Rfc3339).unwrap()
                    },
                    "ultimas_transacoes": client.transactions,
                }
            )))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_transaction(
    Path(id): Path<u8>,
    State(state): State<AppState>,
    Json(transaction): Json<Transaction>,
) -> impl IntoResponse {
    match state.get(&id) {
        Some(client) => {
            let mut client = client.write().await;
            match client.transact(transaction) {
                Ok(_) => Ok(Json(json!(
                    {
                        "saldo": client.balance,
                        "limite": client.limit
                    }
                ))),
                Err(_) => Err(StatusCode::UNPROCESSABLE_ENTITY),
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

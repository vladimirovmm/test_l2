#![cfg(test)]

use std::{fs, sync::LazyLock};

use aptos::APTOS_ACCOUNTS;
use eyre::{Context, Result};
use jsonrpsee::http_client::HttpClientBuilder;
use jwt_jsonrpsee::ClientLayer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::debug;
use tracing_test::traced_test;

use crate::{engine_client::MvEngine, jwt::get_jwt};

pub(crate) mod aptos;
pub(crate) mod engine_client;
pub(crate) mod jwt;

type Slot = u64;
const LAST_SLOT_FILE: &str = "last.slot";

pub(crate) const URL: &str = "http://localhost:9042";
static NEXT_SLOL: LazyLock<Mutex<Slot>> = LazyLock::new(|| {
    let last_slot = fs::read_to_string(LAST_SLOT_FILE)
        .map(|value| {
            value
                .parse::<Slot>()
                .expect("Не валидное значение в {LAST_SLOT_FILE}")
        })
        .unwrap_or_default();
    Mutex::new(last_slot)
});

async fn next_slot() -> Slot {
    let mut slot = NEXT_SLOL.lock().await;
    *slot += 1;
    fs::write(LAST_SLOT_FILE, slot.to_string())
        .with_context(|| format!("Ошибка при записи номера последнего слота {LAST_SLOT_FILE:?}"))
        .unwrap();
    *slot
}

#[traced_test]
#[tokio::test]
async fn test_deposite() -> Result<()> {
    let jwt = get_jwt().await;
    let client = HttpClientBuilder::new()
        .set_http_middleware(tower::ServiceBuilder::new().layer(ClientLayer::new(jwt)))
        .build(URL)
        .context("Ошибка при попытки создать клиента для service-engine")?;

    debug!("response: {:#?}", client.engine_l2info_v1().await?);

    debug!("Запрос с пустым массивом событий");
    client
        .engine_applyattributes_v1(json!({
            "parent_payload": 0,
            "events": [],
            "max_payload_size": 1001,
        }))
        .await
        .context("Пустой массив событий")
        .unwrap();

    debug!("Запрос с пустым массивом событий слота");
    client
        .engine_applyattributes_v1(json!({
            "parent_payload": 0,
            "events": [
                {
                    "slot": next_slot().await,
                    "events":[]
                }
            ],
            "max_payload_size": 1001,
        }))
        .await
        .context("Пустой массив событий")
        .unwrap();

    debug!("Пример запроса через json");

    let response: Value = client.engine_applyattributes_v1(json!({
            "parent_payload": 1,
            "max_payload_size": 1001,
            "events": [
                {
                    "slot": next_slot().await,
                    "events":[
                        {
                            "Deposit":{
                                "account":"0x0000000000000000000000000000000000000000000000000000000000000001",
                                "amount":1
                            }
                        }
                    ]
                }
            ],
        }))
        .await
        .context("запрос на депозит")?;
    debug!("response: {response:#?}");

    debug!("Запрос на пополнение нескольких аккаунтов (engine_applyAttributes_v1)");
    let response: Value = client
        .engine_applyattributes_v1(RequestEngine::all().await)
        .await
        .context("запрос на депозит")?;
    debug!("response: {response:#?}");

    debug!("response: {:#?}", client.engine_l2info_v1().await?);

    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestEngine {
    parent_payload: Slot,
    max_payload_size: Slot,
    events: Vec<RequestSlot>,
}

impl RequestEngine {
    async fn all() -> Self {
        Self {
            parent_payload: 1,
            max_payload_size: 1001,
            events: vec![
                RequestSlot {
                    slot: next_slot().await,
                    events: vec![
                        // Alice
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[0].into(),
                            amount: 1,
                        }),
                        // Bob
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[1].into(),
                            amount: 2,
                        }),
                        // Eve
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[2].into(),
                            amount: 3,
                        }),
                    ],
                },
                RequestSlot {
                    slot: next_slot().await,
                    events: vec![
                        // Alice
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[0].into(),
                            amount: 1,
                        }),
                        // Bob
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[1].into(),
                            amount: 2,
                        }),
                        // Eve
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[2].into(),
                            amount: 3,
                        }),
                        // Eve
                        RequestEvent::Deposit(TxDeposit {
                            account: APTOS_ACCOUNTS[2].into(),
                            amount: 4,
                        }),
                        // 0x0
                        RequestEvent::Deposit(TxDeposit {
                            account: "0".repeat(64),
                            amount: 1004,
                        }),
                        // 0x1
                        RequestEvent::Deposit(TxDeposit {
                            account: format!("{:0>64}", "1"),
                            amount: 1005,
                        }),
                    ],
                },
                RequestSlot {
                    slot: next_slot().await,
                    events: (0..100)
                        .map(|index| {
                            // Alice
                            RequestEvent::Deposit(TxDeposit {
                                account: APTOS_ACCOUNTS[0].into(),
                                amount: index,
                            })
                        })
                        .collect::<Vec<_>>(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RequestSlot {
    slot: Slot,
    events: Vec<RequestEvent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum RequestEvent {
    Deposit(TxDeposit),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxDeposit {
    account: String,
    amount: u64,
}

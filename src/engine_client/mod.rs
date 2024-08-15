use std::fmt::Debug;

use async_trait::async_trait;
use eyre::{Context, Result};
use jsonrpsee::{
    core::client::ClientT,
    http_client::{transport::HttpBackend, HttpClient},
    rpc_params,
};
use jwt_jsonrpsee::ClientAuth;
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, instrument};

#[async_trait]
pub(crate) trait MvEngine: ClientT {
    /// Получинеие информации о текущем состоянии ноды.
    #[instrument(level = "debug", skip(self))]
    async fn engine_l2info_v1(&self) -> Result<Value> {
        debug!("Получинеие информации о текущем состоянии ноды. engine_l2Info_v1: request");

        self.request::<Value, _>("engine_l2Info_v1", rpc_params![])
            .await
            .context("запрос на получения статуса ноды")
    }

    /// Отправка собыитий.
    /// Нужен для запросов на депозит
    #[instrument(level = "debug", skip(self))]
    async fn engine_applyattributes_v1<T>(&self, value: T) -> Result<Value>
    where
        T: Serialize + Send + Debug,
    {
        let value = serde_json::to_value(value)
            .context("Произошла ошибка при преобразовании значения `value` в json")?;
        self.request::<Value, _>("engine_applyAttributes_v1", rpc_params!(value))
            .await
            .context("запрос на депозит")
    }
}
impl MvEngine for HttpClient<ClientAuth<HttpBackend>> {}

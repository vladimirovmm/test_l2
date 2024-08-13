#![cfg(test)]

use std::{fs, path::PathBuf, str::FromStr};

use async_once_cell::OnceCell;
use eyre::{Context, ContextCompat, Result};
use jwt_jsonrpsee::JwtSecret;
use rand::random;
use serde_yaml::Value;
use tracing::{debug, info};
use tracing_test::traced_test;

// const URL: &str = "http://localhost:9042";
const JWT_PATH: &str = "engine.jwt";

static JWT: OnceCell<JwtSecret> = OnceCell::new();

async fn get_jwt() -> JwtSecret {
    *JWT.get_or_init(async {
        JwtSecret::from_str(
            &fs::read_to_string(JWT_PATH)
                .with_context(|| format!("При чтении JWT {JWT_PATH:?} произошла ошибка"))
                .unwrap(),
        )
        .context("Неудолось декодировать JWT")
        .unwrap()
    })
    .await
}

#[traced_test]
#[tokio::test]
async fn test_mv_deposite() -> Result<()> {
    let _jwt = get_jwt().await;
    //
    Ok(())
}

/// Добавить ключ в конфиг. По умолчанию он генерируется при запуске
#[ignore]
#[traced_test]
#[tokio::test]
async fn patch_mv_node_config() -> Result<()> {
    const CONFIG_FILE_NAME: &str = "node.yaml";
    const DEFAULT_JWT_PATH: &str = "l2/test-node/engine.jwt";

    debug!("Чтение конфига из {CONFIG_FILE_NAME:?}");
    let config_str = fs::read_to_string(CONFIG_FILE_NAME).context("Неудалось открыть конфиг")?;
    let mut config: Value =
        serde_yaml::from_str(&config_str).context("При десериализации конфига произошла ошибка")?;

    debug!("Проверка на существование поля в конфиге engine_service::jwt_path");
    let engine_service = config
        .as_mapping_mut()
        .context("Не валидный конфиг. Ожидался Mapping")?
        .entry(Value::String("engine_service".into()))
        .or_insert(Value::Mapping(Default::default()))
        .as_mapping_mut()
        .context("Не валидный конфиг `engine_service`. Ожидался Mapping")?;

    if engine_service.contains_key("jwt_path") {
        debug!("JWT уже существует в конфиге");
        return Ok(());
    }

    debug!("Генерация JWT");
    let jwt = JwtSecret::new(random());
    debug!("Сохранение JWT");
    fs::write(JWT_PATH, jwt.to_string()).context("Ошибка при сохранении JWT")?;
    let full_jwt_path = PathBuf::from(JWT_PATH).canonicalize().unwrap();
    info!("Ключ сохранен в {full_jwt_path:?}");

    engine_service.insert(
        "jwt_path".to_string().into(),
        DEFAULT_JWT_PATH.to_string().into(),
    );
    debug!("Путь до JWT установлен в конфиге {DEFAULT_JWT_PATH}");

    fs::write(CONFIG_FILE_NAME, serde_yaml::to_string(&config).unwrap())
        .context("Неудалось записать конфиг в {FILE_NAME}")?;
    let full_config_path = PathBuf::from(CONFIG_FILE_NAME).canonicalize().unwrap();
    info!("Конфиг успешно сохранён в {full_config_path:?}");

    info!("Перенесите {full_config_path:?} и {full_jwt_path:?} в нужную директорию и если путь до ключа отличается измените на нужный");

    Ok(())
}

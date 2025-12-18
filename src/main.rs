use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metrics = metrics_provider::MetricsClient::new().await?;

    let user_to_log = env::var("GITHUB_ACTOR")
        .unwrap_or_else(|_| {
            env::var("USERNAME") // Windows
                .or_else(|_| env::var("USER")) // Linux/Mac
                .unwrap_or_else(|_| "local_dev".to_string())
        });

    println!("Работаем от имени: {}", user_to_log);

    metrics.add_metric(&user_to_log).await?;
    println!("Метрика для {} обновлена.", user_to_log);

    metrics.add_default_metric().await?;
    println!("Общая системная метрика обновлена.");

    Ok(())
}
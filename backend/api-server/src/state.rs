use sqlx::PgPool;

use crate::routes::price::PriceCache;

#[derive(Clone)]
pub struct AppState {
    pub db: Option<PgPool>,
    pub rpc_url: String,
    pub price_cache: PriceCache,
    pub http: reqwest::Client,
}

impl AppState {
    pub async fn new(database_url: &str, rpc_url: String) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(database_url).await?;
        sqlx::migrate!("../migrations").run(&db).await?;
        Ok(Self {
            db: Some(db),
            rpc_url,
            price_cache: PriceCache::new(),
            http: reqwest::Client::new(),
        })
    }

    pub fn without_db() -> Self {
        Self {
            db: None,
            rpc_url: "https://sepolia.base.org".into(),
            price_cache: PriceCache::new(),
            http: reqwest::Client::new(),
        }
    }

    pub fn require_db(&self) -> Result<&PgPool, &'static str> {
        self.db.as_ref().ok_or("database not available")
    }
}

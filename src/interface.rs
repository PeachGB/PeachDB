use std::error::Error;

use crate::db::{DataBase, Field};

pub struct Database {
    inner: DataBase,
}

impl Database {
    pub async fn open(name: &str) -> Result<Self, Box<dyn Error>> {
        let inner = DataBase::new(name).await?;
        Ok(Self { inner })
    }

    pub async fn get(&self, key: [u8; 16]) -> Option<Field> {
        self.inner.get(key).await
    }

    pub async fn insert(&mut self, key: [u8; 16], value: Field) -> Result<(), Box<dyn Error>> {
        self.inner.insert(key, value).await
    }

    pub async fn persist(&mut self, key: [u8; 16], value: Field) -> Result<(), Box<dyn Error>> {
        self.inner.persist(key, value).await
    }

    pub async fn flush(&self) -> Result<(), Box<dyn Error>> {
        self.inner.flush().await
    }

    pub async fn close(self) -> Result<(), Box<dyn Error>> {
        self.flush().await
    }
}

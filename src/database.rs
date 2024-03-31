use std::collections::HashMap;
use tokio::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use serenity::Result;
use std::fmt::Display;
use std::hash::Hash;

pub trait Database<K, V> {
    async fn get(&self, key: K) -> Result<V>;
    async fn set(&self, key: K, value: &V) -> Result<()>;
    async fn delete(&self, key: K) -> Result<()>;
}

#[derive(Clone)]
pub struct FsDatabase<K, V> {
    cache: Arc<RwLock<HashMap<K, V>>>,
    folder_name: String,
}

impl<K, V> FsDatabase<K, V>
    where
        K: Clone + Eq + Hash + Display,
        V: Clone + Serialize + for<'a> Deserialize<'a> + Default,
{
    pub async fn create(folder_name: impl Into<String>) -> Self {
        let folder_name = folder_name.into();
        fs::create_dir_all(&folder_name).await.unwrap();
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            folder_name,
        }
    }
}

impl<K, V> Database<K, V> for FsDatabase<K, V>
    where
        K: Clone + Eq + Hash + Display,
        V: Clone + Serialize + for<'a> Deserialize<'a> + Default,
{
    async fn get(&self, key: K) -> Result<V> {
        {
            let cache = self.cache.read().await;
            if let Some(cached_data) = cache.get(&key) {
                return Ok(cached_data.clone());
            }
        }
        let path = format!("{}/{}.json", self.folder_name, key);
        if !Path::new(&path).exists() {
            return Ok(V::default());
        }
        let data = fs::read_to_string(&path).await?;
        let value: V = serde_json::from_str(&data)?;
        {
            let mut cache = self.cache.write().await;
            cache.insert(key, value.clone());
        }
        Ok(value)
    }

    async fn set(&self, key: K, value: &V) -> Result<()> {
        {
            let mut cache = self.cache.write().await;
            cache.insert(key.clone(), value.clone());
        }
        let path = format!("{}/{}.json", self.folder_name, key);
        let data = serde_json::to_string(value)?;
        fs::write(path, data).await?;
        Ok(())
    }

    async fn delete(&self, key: K) -> Result<()> {
        let path = format!("{}/{}.json", self.folder_name, key);
        fs::remove_file(path).await?;
        {
            let mut cache = self.cache.write().await;
            cache.remove(&key);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct MemoryDatabase<K, V> {
    data: Arc<RwLock<HashMap<K, V>>>,
}

impl<K, V> MemoryDatabase<K, V>
    where
        K: Clone + Eq + Hash + Display,
        V: Clone + Default,
{
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<K, V> Database<K, V> for MemoryDatabase<K, V>
    where
        K: Clone + Eq + Hash + Display,
        V: Clone + Default,
{
    async fn get(&self, key: K) -> Result<V> {
        let data = self.data.read().await;
        Ok(data.get(&key).cloned().unwrap_or_default())
    }

    async fn set(&self, key: K, value: &V) -> Result<()> {
        let mut data = self.data.write().await;
        data.insert(key, value.clone());
        Ok(())
    }

    async fn delete(&self, key: K) -> Result<()> {
        let mut data = self.data.write().await;
        data.remove(&key);
        Ok(())
    }
}
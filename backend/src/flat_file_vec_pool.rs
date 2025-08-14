use async_trait::async_trait;
use deadpool::managed::{Manager, Pool, RecycleResult, Metrics};
use std::path::PathBuf;
use anyhow::Result;
use common::flat_file_vec::{FlatFileVec, FixedRepr};

/// FlatFileVec用の汎用的なManager
pub struct FlatFileVecManager<T: FixedRepr> {
    pub path: PathBuf,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: FixedRepr> FlatFileVecManager<T> {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<T: FixedRepr + Send + Sync + 'static> Manager for FlatFileVecManager<T> {
    type Type = FlatFileVec<T>;
    type Error = anyhow::Error;

    async fn create(&self) -> Result<FlatFileVec<T>> {
        FlatFileVec::open_readonly(&self.path).map_err(Into::into)
    }

    async fn recycle(&self, _obj: &mut FlatFileVec<T>, _metrics: &Metrics) -> RecycleResult<anyhow::Error> {
        // ファイルハンドルの再利用可否チェック
        // 通常は読み取り専用なので問題なし
        Ok(())
    }
}

/// FlatFileVecプールの型エイリアス
pub type FlatFileVecPool<T> = Pool<FlatFileVecManager<T>>;

/// プールビルダーのヘルパー関数
pub fn create_flat_file_vec_pool<T: FixedRepr + Send + Sync + 'static>(
    path: impl Into<PathBuf>,
    max_size: usize,
) -> Result<FlatFileVecPool<T>> {
    let manager = FlatFileVecManager::new(path);
    Pool::builder(manager)
        .max_size(max_size)
        .build()
        .map_err(Into::into)
} 
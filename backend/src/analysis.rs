use crate::flat_file_vec_pool::{create_flat_file_vec_pool, FlatFileVecPool};
use common::mahjong::{Dimension, Hand, HandConverter, Metrics, Tile, NUM_ROUNDS};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;

#[derive(Debug, Serialize)]
pub struct TsumoAnalysis {
    pub probabilities: Vec<TsumoProbability>,
}

#[derive(Debug, Serialize)]
pub struct TsumoProbability {
    pub draws_left: u32,
    pub probability: f64,
}

/// メンツ実現確率分析結果
#[derive(Debug, Serialize)]
pub struct MentsuAnalysis {
    pub probabilities: Vec<MentsuProbability>,
}

#[derive(Debug, Serialize)]
pub struct MentsuProbability {
    pub mentsu_type: String,
    pub probability: f64,
}

/// 共有可能な手牌分析エンジン
#[derive(Clone)]
pub struct SharedHandAnalyzer {
    converter: Arc<HandConverter>,
    // ツモ率データファイル用プール（13枚用）
    tsumo_13_pool: Arc<FlatFileVecPool<u32>>,
    // ツモ率データファイル用プール（14枚用）
    tsumo_14_pool: Arc<FlatFileVecPool<u32>>,
    // メトリクスデータファイル用プール（13枚用）
    metrics_13_pool: Arc<FlatFileVecPool<Metrics>>,
    // メトリクスデータファイル用プール（14枚用）
    metrics_14_pool: Arc<FlatFileVecPool<Metrics>>,
}

impl SharedHandAnalyzer {
    /// 新しい共有分析エンジンを作成
    pub fn new(
        conv_path: impl AsRef<Path>,
        tsumo_13_path: impl Into<PathBuf>,
        tsumo_14_path: impl Into<PathBuf>,
        metrics_13_path: impl Into<PathBuf>,
        metrics_14_path: impl Into<PathBuf>,
        max_pool_size: usize,
    ) -> Result<Self> {
        // HandConverterを読み込み
        let converter = HandConverter::load_from_file(conv_path)?;
        let tsumo_13_pool = create_flat_file_vec_pool(tsumo_13_path, max_pool_size)?;
        let tsumo_14_pool = create_flat_file_vec_pool(tsumo_14_path, max_pool_size)?;
        let metrics_13_pool = create_flat_file_vec_pool(metrics_13_path, max_pool_size)?;
        let metrics_14_pool = create_flat_file_vec_pool(metrics_14_path, max_pool_size)?;

        Ok(SharedHandAnalyzer {
            converter: Arc::new(converter),
            // プールは後で追加する予定
            tsumo_13_pool: Arc::new(tsumo_13_pool),
            tsumo_14_pool: Arc::new(tsumo_14_pool),
            metrics_13_pool: Arc::new(metrics_13_pool),
            metrics_14_pool: Arc::new(metrics_14_pool),
        })
    }

    /// 手牌を分析してツモ率を計算
    pub async fn analyze_tsumo(&self, hand: &[Tile]) -> Result<TsumoAnalysis> {
        let probs;
        let hand_id;
        if hand.len() == 13 {
            hand_id = self.converter.encode_hand13_fast(&Hand::from_tiles(hand)) as usize;
            probs = self
                .tsumo_13_pool
                .get()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get pool: {}", e))?
                .get_range(hand_id * NUM_ROUNDS, (hand_id + 1) * NUM_ROUNDS)?;
        } else if hand.len() == 14 {
            hand_id = self.converter.encode_hand14_fast(&Hand::from_tiles(hand)) as usize;
            probs = self
                .tsumo_14_pool
                .get()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get pool: {}", e))?
                .get_range(hand_id * NUM_ROUNDS, (hand_id + 1) * NUM_ROUNDS)?;
        } else {
            return Err(anyhow::anyhow!("Invalid hand length: {}", hand.len()));
        }

        let probabilities = if hand.len() == 13 {
            probs
                .into_iter()
                .enumerate()
                .map(|(round, p)| TsumoProbability {
                    draws_left: (round as u32) + 1,
                    probability: (p as f64) / 2f64.powi(32),
                })
                .collect()
        } else {
            probs
                .into_iter()
                .enumerate()
                .map(|(round, p)| TsumoProbability {
                    draws_left: round as u32,
                    probability: (p as f64) / 2f64.powi(32),
                })
                .collect()
        };
        Ok(TsumoAnalysis { probabilities })
    }

    /// 手牌を分析してメンツ実現確率を計算
    pub async fn analyze_mentsu(&self, hand: &[Tile], draws_left: usize) -> Result<MentsuAnalysis> {
        let met;
        let hand_id;
        let trans;
        let jihai_cnt;
        if hand.len() == 13 {
            if draws_left < 1 || draws_left > NUM_ROUNDS {
                return Err(anyhow::anyhow!("Invalid draws_left: {}", draws_left));
            }
            let _hand;
            let _hi;
            (_hand, jihai_cnt) = Hand::from_tiles_with_jihai_cnt(hand);
            (_hi, trans) = self.converter.encode_hand13(&_hand);
            hand_id = _hi as usize;
            met = self
                .metrics_13_pool
                .get()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get pool: {}", e))?
                .get(hand_id * NUM_ROUNDS + draws_left - 1)?;
        } else if hand.len() == 14 {
            if draws_left >= NUM_ROUNDS {
                return Err(anyhow::anyhow!("Invalid draws_left: {}", draws_left));
            }
            let _hand;
            let _hi;
            (_hand, jihai_cnt) = Hand::from_tiles_with_jihai_cnt(hand);
            (_hi, trans) = self.converter.encode_hand14(&_hand);
            hand_id = _hi as usize;
            met = self
                .metrics_14_pool
                .get()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get pool: {}", e))?
                .get(hand_id * NUM_ROUNDS + draws_left)?;
        } else {
            return Err(anyhow::anyhow!("Invalid hand length: {}", hand.len()));
        }

        const SUPAI_LOOKUP: [char; 3] = ['m', 'p', 's'];

        let mut probabilities = Vec::with_capacity(21 + 27 + 27 + 7 + 7 + 1);
        for (i, p) in met.values.into_iter().enumerate() {
            let dim = Dimension::from_id(i % Dimension::len());
            let probability = (p as f64) / 2f64.powi(30);
            match dim {
                Dimension::Shuntsu(Tile::Supai(s, mut n)) => {
                    let mut t = trans[s as usize];
                    if t < 0 {
                        t = !t;
                        n = 6 - n;
                    }
                    probabilities.push(MentsuProbability {
                        mentsu_type: format!(
                            "{}{}{}{}",
                            n + 1,
                            n + 2,
                            n + 3,
                            SUPAI_LOOKUP[t as usize]
                        ),
                        probability,
                    });
                }
                Dimension::Kotsu(Tile::Supai(s, mut n)) => {
                    let mut t = trans[s as usize];
                    if t < 0 {
                        t = !t;
                        n = 8 - n;
                    }
                    probabilities.push(MentsuProbability {
                        mentsu_type: format!(
                            "{}{}{}{}",
                            n + 1,
                            n + 1,
                            n + 1,
                            SUPAI_LOOKUP[t as usize]
                        ),
                        probability,
                    });
                }
                Dimension::Toitsu(Tile::Supai(s, mut n)) => {
                    let mut t = trans[s as usize];
                    if t < 0 {
                        t = !t;
                        n = 8 - n;
                    }
                    probabilities.push(MentsuProbability {
                        mentsu_type: format!("{}{}{}", n + 1, n + 1, SUPAI_LOOKUP[t as usize]),
                        probability,
                    });
                }
                Dimension::Kotsu(Tile::Jihai(n)) => {
                    for (ji, &cnt) in jihai_cnt.iter().enumerate() {
                        if cnt == n as usize {
                            probabilities.push(MentsuProbability {
                                mentsu_type: format!("{}{}{}z", ji + 1, ji + 1, ji + 1),
                                probability,
                            });
                        }
                    }
                }
                Dimension::Toitsu(Tile::Jihai(n)) => {
                    for (ji, &cnt) in jihai_cnt.iter().enumerate() {
                        if cnt == n as usize {
                            probabilities.push(MentsuProbability {
                                mentsu_type: format!("{}{}z", ji + 1, ji + 1),
                                probability,
                            });
                        }
                    }
                }
                Dimension::Kokushi => {
                    probabilities.push(MentsuProbability {
                        mentsu_type: "Kokushi".to_string(),
                        probability,
                    });
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Internal error: Invalid dimension: {:?}",
                        dim
                    ))
                }
            };
        }
        Ok(MentsuAnalysis { probabilities })
    }
}

use std::{
    array, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use common::{
    flat_file_vec::FlatFileVec,
    mahjong::{Dimension, Hand, HandConverter, Metrics, Tile, NUM_HAND13, NUM_HAND14, NUM_ROUNDS},
};
use dp::metrics;
use itertools::{iproduct, izip};
use rayon::prelude::*;

const SHARD_SIZE: usize = 1 << 21;
const NUM_SHARDS_13: usize = (NUM_HAND13 + SHARD_SIZE - 1) / SHARD_SIZE;
const NUM_SHARDS_14: usize = (NUM_HAND14 + SHARD_SIZE - 1) / SHARD_SIZE;

fn log(msg: impl std::fmt::Display) {
    println!(
        "[{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        msg
    );
}

struct DpMain {
    conv: HandConverter,
    dir: PathBuf,
}

impl DpMain {
    fn resume(conv: HandConverter, dir: impl AsRef<Path>) -> Self {
        fs::create_dir_all(&dir).unwrap();
        Self {
            conv,
            dir: dir.as_ref().to_path_buf(),
        }
    }

    fn get_metrics_temp_path(&self, round: usize, dim_id: usize, shard_id: usize) -> PathBuf {
        self.dir.join(format!(
            "metrics_temp/{:02}/{:02}/{:03}.dat",
            dim_id, round, shard_id
        ))
    }

    fn get_tsumo_temp_path(&self, round: usize) -> PathBuf {
        self.dir.join(format!("tsumo_temp/{:02}.dat", round))
    }

    fn fill_tsumo_temp(&self) -> Result<()> {
        let paths = glob::glob(self.dir.join("tsumo_temp/??.dat").to_str().unwrap()).unwrap();
        let last_path = paths
            .map(|r| {
                r.as_deref()
                    .unwrap()
                    .to_path_buf()
                    .into_os_string()
                    .into_string()
                    .unwrap()
            })
            .max();

        let mut round: usize = match last_path {
            Some(last) => {
                Path::new(last.as_str())
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()
                    + 1
            }
            None => 0,
        };
        if round == common::mahjong::NUM_ROUNDS * 2 {
            return Ok(());
        }

        // init agari hands and prev_memo
        let mut cur_memo: Vec<u128>;
        let mut agari_hands: Vec<u32> = Vec::new();
        if round == 0 {
            cur_memo = dp::tsumo::dp14_r0(&self.conv);
            for (hi, &v) in cur_memo.iter().enumerate() {
                if v > 0 {
                    agari_hands.push(hi as u32);
                }
            }
            FlatFileVec::save_all(cur_memo.iter().copied(), self.get_tsumo_temp_path(0)).unwrap();
            round += 1;
        } else {
            let dp0 = FlatFileVec::<u128>::open_readonly(self.get_tsumo_temp_path(0))?;
            for (hi, v) in dp0.into_iter().enumerate() {
                if v.unwrap() > 0 {
                    agari_hands.push(hi as u32);
                }
            }
            cur_memo = FlatFileVec::<u128>::load_all(self.get_tsumo_temp_path(round - 1))?;
        }
        agari_hands.shrink_to_fit();

        while round < common::mahjong::NUM_ROUNDS * 2 {
            println!(
                "[{}], round={}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                round
            );
            if round % 2 == 0 {
                cur_memo = dp::tsumo::dp13_to_dp14(
                    &self.conv,
                    &cur_memo,
                    &agari_hands,
                    (136u128 - 13).pow((round / 2) as u32),
                );
            } else {
                cur_memo = dp::tsumo::dp14_to_dp13(&self.conv, &cur_memo);
                println!(
                    "{}",
                    (dp::tsumo::check(&self.conv, &cur_memo) as f64)
                        / ((136u128 - 13).pow((round / 2 + 1) as u32) as f64)
                );
            }
            FlatFileVec::save_all(cur_memo.iter().copied(), self.get_tsumo_temp_path(round))?;
            round += 1;
        }
        Ok(())
    }

    fn write_metrics_temp<I>(&self, metrics: I, round: usize, dim_id: usize) -> Result<()>
    where
        I: IntoIterator<Item = u32>,
    {
        log(format!(
            "writing metrics_temp for round={:02}, dim_id={:02}",
            round, dim_id
        ));
        let mut iter = metrics.into_iter();
        let mut shard_id = 0;

        loop {
            // Take up to SHARD_SIZE elements from the iterator
            let shard: Vec<u32> = iter.by_ref().take(SHARD_SIZE).collect();

            // If no elements were collected, we're done
            if shard.is_empty() {
                break;
            }

            let path = self.get_metrics_temp_path(round, dim_id, shard_id);
            FlatFileVec::save_all(shard, path)?;
            shard_id += 1;
        }
        Ok(())
    }

    fn fill_metrics_temp(&self, start_task_id: usize) -> Result<()> {
        log("construct agari metrics");
        let agari_metrics = metrics::construct_agari_metrics(&self.conv);
        log("construct agari metrics done");

        let tasks = [
            // Dimension::Shuntsu(Tile::Supai(0, 0)),
            // Dimension::Shuntsu(Tile::Supai(0, 1)),
            // Dimension::Shuntsu(Tile::Supai(0, 2)),
            // Dimension::Shuntsu(Tile::Supai(0, 3)),
            // Dimension::Kotsu(Tile::Supai(0, 0)),
            // Dimension::Kotsu(Tile::Supai(0, 1)),
            // Dimension::Kotsu(Tile::Supai(0, 2)),
            // Dimension::Kotsu(Tile::Supai(0, 3)),
            // Dimension::Kotsu(Tile::Supai(0, 4)),
            // Dimension::Toitsu(Tile::Supai(0, 0)),
            // Dimension::Toitsu(Tile::Supai(0, 1)),
            // Dimension::Toitsu(Tile::Supai(0, 2)),
            // Dimension::Toitsu(Tile::Supai(0, 3)),
            // Dimension::Toitsu(Tile::Supai(0, 4)),
            // Dimension::Kotsu(Tile::Jihai(0)),
            // Dimension::Toitsu(Tile::Jihai(0)),
            Dimension::Kokushi,
        ];

        for (task_id, task) in tasks[start_task_id..].iter().enumerate() {
            log(format!(
                "task_id={}, task={:?}",
                task_id + start_task_id,
                task
            ));
            let tile = match task {
                Dimension::Shuntsu(Tile::Supai(_, n)) => Some(Tile::Supai(0, *n)),
                Dimension::Kotsu(Tile::Supai(_, n)) => Some(Tile::Supai(0, *n)),
                Dimension::Toitsu(Tile::Supai(_, n)) => Some(Tile::Supai(0, *n)),
                Dimension::Shuntsu(Tile::Jihai(_)) => Some(Tile::Jihai(0)),
                Dimension::Kotsu(Tile::Jihai(_)) => Some(Tile::Jihai(0)),
                Dimension::Toitsu(Tile::Jihai(_)) => Some(Tile::Jihai(0)),
                Dimension::Kokushi => None,
            };
            // 数牌
            if let Some(Tile::Supai(_, _)) = tile {
                self.do_metrics_dp_supai(task, &agari_metrics)?;
            }
            // 字牌
            if let Some(Tile::Jihai(_)) = tile {
                self.do_metrics_dp_jihai(task, &agari_metrics)?;
            }
            // 国士無双
            if let None = tile {
                self.do_metrics_dp_kokushi(&agari_metrics)?;
            }
        }
        Ok(())
    }

    fn do_metrics_dp_supai(
        &self,
        task: &Dimension,
        agari_metrics: &[(u32, Metrics)],
    ) -> Result<()> {
        let dims: [[u8; 2]; 3] = {
            match task {
                Dimension::Shuntsu(Tile::Supai(_, n)) => array::from_fn(|s| {
                    [
                        Dimension::Shuntsu(Tile::Supai(s as u8, *n)).to_id(),
                        Dimension::Shuntsu(Tile::Supai(s as u8, 6 - n)).to_id(),
                    ]
                }),
                Dimension::Kotsu(Tile::Supai(_, n)) => array::from_fn(|s| {
                    [
                        Dimension::Kotsu(Tile::Supai(s as u8, *n)).to_id(),
                        Dimension::Kotsu(Tile::Supai(s as u8, 8 - n)).to_id(),
                    ]
                }),
                Dimension::Toitsu(Tile::Supai(_, n)) => array::from_fn(|s| {
                    [
                        Dimension::Toitsu(Tile::Supai(s as u8, *n)).to_id(),
                        Dimension::Toitsu(Tile::Supai(s as u8, 8 - n)).to_id(),
                    ]
                }),
                _ => unreachable!(),
            }
        };

        let agari: Vec<(u32, [[u32; 2]; 3])> = agari_metrics
            .iter()
            .map(|(hi, m)| {
                let mut res = [[0u32; 2]; 3];
                for i in 0..3 {
                    for j in 0..2 {
                        res[i][j] = m.values[dims[i][j] as usize];
                    }
                }
                (*hi, res)
            })
            .collect();

        let mut metrics_14 = vec![[[0u32; 2]; 3]; NUM_HAND14];
        let mut metrics_13 = vec![[[0u32; 2]; 3]; NUM_HAND13];

        log(format!("round=00"));
        for (hi, m) in agari.iter() {
            metrics_14[*hi as usize] = m.clone();
        }
        for i in 0..3 {
            for j in 0..2 {
                if j == 1 && dims[i][0] == dims[i][1] {
                    continue;
                }
                self.write_metrics_temp(
                    metrics_14.iter().map(|m| m[i][j]),
                    0,
                    dims[i][j] as usize,
                )?;
            }
        }

        for round in 1..(NUM_ROUNDS * 2) {
            log(format!("round={:02}", round));
            if round % 2 == 0 {
                let tsumo_13 = FlatFileVec::<u128>::load_all(
                    self.dir.join(format!("tsumo_temp/{:02}.dat", round - 1)),
                )?;
                metrics::process_13_to_14_supai(
                    &self.conv,
                    &metrics_13,
                    &mut metrics_14,
                    &tsumo_13,
                    &agari,
                );
                for i in 0..3 {
                    for j in 0..2 {
                        if j == 1 && dims[i][0] == dims[i][1] {
                            continue;
                        }
                        self.write_metrics_temp(
                            metrics_14.iter().map(|m| m[i][j]),
                            round,
                            dims[i][j] as usize,
                        )?;
                    }
                }
            } else {
                metrics::process_14_to_13_supai(&self.conv, &metrics_14, &mut metrics_13);
                for i in 0..3 {
                    for j in 0..2 {
                        if j == 1 && dims[i][0] == dims[i][1] {
                            continue;
                        }
                        self.write_metrics_temp(
                            metrics_13.iter().map(|m| m[i][j]),
                            round,
                            dims[i][j] as usize,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn do_metrics_dp_jihai(
        &self,
        task: &Dimension,
        agari_metrics: &[(u32, Metrics)],
    ) -> Result<()> {
        let dims: [u8; 5] = {
            match task {
                Dimension::Shuntsu(Tile::Jihai(_)) => {
                    array::from_fn(|s| Dimension::Shuntsu(Tile::Jihai(s as u8)).to_id())
                }
                Dimension::Kotsu(Tile::Jihai(_)) => {
                    array::from_fn(|s| Dimension::Kotsu(Tile::Jihai(s as u8)).to_id())
                }
                Dimension::Toitsu(Tile::Jihai(_)) => {
                    array::from_fn(|s| Dimension::Toitsu(Tile::Jihai(s as u8)).to_id())
                }
                _ => unreachable!(),
            }
        };

        let agari: Vec<(u32, [u32; 5])> = agari_metrics
            .iter()
            .map(|(hi, m)| {
                let mut res = [0u32; 5];
                for i in 0..5 {
                    res[i] = m.values[dims[i] as usize];
                }
                (*hi, res)
            })
            .collect();

        let mut metrics_14 = vec![[0u32; 5]; NUM_HAND14];
        let mut metrics_13 = vec![[0u32; 5]; NUM_HAND13];

        log(format!("round=00"));
        for (hi, m) in agari.iter() {
            metrics_14[*hi as usize] = m.clone();
        }
        for i in 0..5 {
            self.write_metrics_temp(metrics_14.iter().map(|m| m[i]), 0, dims[i] as usize)?;
        }

        for round in 1..(NUM_ROUNDS * 2) {
            log(format!("round={:02}", round));
            if round % 2 == 0 {
                let tsumo_13 = FlatFileVec::<u128>::load_all(
                    self.dir.join(format!("tsumo_temp/{:02}.dat", round - 1)),
                )?;
                metrics::process_13_to_14_jihai(
                    &self.conv,
                    &metrics_13,
                    &mut metrics_14,
                    &tsumo_13,
                    &agari,
                );
                for i in 0..5 {
                    self.write_metrics_temp(
                        metrics_14.iter().map(|m| m[i]),
                        round,
                        dims[i] as usize,
                    )?;
                }
            } else {
                metrics::process_14_to_13_jihai(&self.conv, &metrics_14, &mut metrics_13);
                for i in 0..5 {
                    self.write_metrics_temp(
                        metrics_13.iter().map(|m| m[i]),
                        round,
                        dims[i] as usize,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn do_metrics_dp_kokushi(&self, agari_metrics: &[(u32, Metrics)]) -> Result<()> {
        let agari: Vec<(u32, u32)> = agari_metrics
            .iter()
            .map(|(hi, m)| (*hi, m.values[Dimension::Kokushi.to_id() as usize]))
            .collect();

        let mut metrics_14 = vec![0u32; NUM_HAND14];
        let mut metrics_13 = vec![0u32; NUM_HAND13];

        log(format!("round=00"));
        for (hi, m) in agari.iter().copied() {
            metrics_14[hi as usize] = m;
        }
        self.write_metrics_temp(
            metrics_14.iter().copied(),
            0,
            Dimension::Kokushi.to_id() as usize,
        )?;

        for round in 1..(NUM_ROUNDS * 2) {
            log(format!("round={:02}", round));
            if round % 2 == 0 {
                let tsumo_13 = FlatFileVec::<u128>::load_all(
                    self.dir.join(format!("tsumo_temp/{:02}.dat", round - 1)),
                )?;
                metrics::process_13_to_14_kokushi(
                    &self.conv,
                    &metrics_13,
                    &mut metrics_14,
                    &tsumo_13,
                    &agari,
                );
                self.write_metrics_temp(
                    metrics_14.iter().copied(),
                    round,
                    Dimension::Kokushi.to_id() as usize,
                )?;
            } else {
                metrics::process_14_to_13_kokushi(&self.conv, &metrics_14, &mut metrics_13);
                self.write_metrics_temp(
                    metrics_13.iter().copied(),
                    round,
                    Dimension::Kokushi.to_id() as usize,
                )?;
            }
        }
        Ok(())
    }

    fn collect_metrics_14_temps(&self) -> Result<()> {
        let mut metrics_14_store =
            FlatFileVec::<Metrics>::open_or_create(self.dir.join("metrics_14.dat"))?;
        if metrics_14_store.len() == NUM_HAND14 * NUM_ROUNDS {
            log(format!("14: metrics_14.dat already exists"));
            return Ok(());
        }
        assert_eq!(metrics_14_store.len() % (NUM_ROUNDS * SHARD_SIZE), 0);
        let start_shard_id = metrics_14_store.len() / (NUM_ROUNDS * SHARD_SIZE);
        for shard_id in start_shard_id..NUM_SHARDS_14 {
            log(format!("14: shard_id={:3}/{:3}", shard_id, NUM_SHARDS_14));
            let size = SHARD_SIZE.min(NUM_HAND14 - shard_id * SHARD_SIZE);

            log(format!("    loading shards"));
            let mut shards: [Vec<u32>; NUM_ROUNDS * Dimension::len()] =
                core::array::from_fn(|_| Vec::new());
            shards.par_iter_mut().enumerate().for_each(|(i, shard)| {
                *shard = FlatFileVec::<u32>::load_all(self.get_metrics_temp_path(
                    (i / Dimension::len()) * 2,
                    i % Dimension::len(),
                    shard_id,
                ))
                .unwrap();
            });
            log(format!("    filling output"));
            let mut output = vec![Metrics::default(); size * NUM_ROUNDS];
            output.par_iter_mut().enumerate().for_each(|(i, m)| {
                let hi = i / NUM_ROUNDS;
                let round = i % NUM_ROUNDS;
                for (dim_id, mv) in m.values.iter_mut().enumerate() {
                    *mv = shards[round * Dimension::len() + dim_id][hi];
                }
            });
            log(format!("    extending metrics 14 store"));
            metrics_14_store.extend(output)?;
            log(format!("    removing shards"));
            for round in 0..NUM_ROUNDS {
                for dim_id in 0..Dimension::len() {
                    fs::remove_file(self.get_metrics_temp_path(round * 2, dim_id, shard_id))
                        .unwrap();
                }
            }
        }
        Ok(())
    }

    fn collect_metrics_13_temps(&self) -> Result<()> {
        let mut metrics_13_store =
            FlatFileVec::<Metrics>::open_or_create(self.dir.join("metrics_13.dat"))?;
        if metrics_13_store.len() == NUM_HAND13 * NUM_ROUNDS {
            log(format!("13: metrics_13.dat already exists"));
            return Ok(());
        }
        assert_eq!(metrics_13_store.len() % (NUM_ROUNDS * SHARD_SIZE), 0);
        let start_shard_id = metrics_13_store.len() / (NUM_ROUNDS * SHARD_SIZE);
        for shard_id in start_shard_id..NUM_SHARDS_13 {
            log(format!("13: shard_id={:3}/{:3}", shard_id, NUM_SHARDS_13));
            let size = SHARD_SIZE.min(NUM_HAND13 - shard_id * SHARD_SIZE);

            log(format!("    loading shards"));
            let mut shards: [Vec<u32>; NUM_ROUNDS * Dimension::len()] =
                core::array::from_fn(|_| Vec::new());
            shards.par_iter_mut().enumerate().for_each(|(i, shard)| {
                *shard = FlatFileVec::<u32>::load_all(self.get_metrics_temp_path(
                    (i / Dimension::len()) * 2 + 1,
                    i % Dimension::len(),
                    shard_id,
                ))
                .unwrap();
            });
            log(format!("    filling output"));
            let mut output = vec![Metrics::default(); size * NUM_ROUNDS];
            output.par_iter_mut().enumerate().for_each(|(i, m)| {
                let hi = i / NUM_ROUNDS;
                let round = i % NUM_ROUNDS;
                for (dim_id, mv) in m.values.iter_mut().enumerate() {
                    *mv = shards[round * Dimension::len() + dim_id][hi];
                }
            });
            log(format!("    extending metrics 13 store"));
            metrics_13_store.extend(output)?;
            log(format!("    removing shards"));
            for round in 0..NUM_ROUNDS {
                for dim_id in 0..Dimension::len() {
                    fs::remove_file(self.get_metrics_temp_path(round * 2 + 1, dim_id, shard_id))
                        .unwrap();
                }
            }
        }
        Ok(())
    }

    fn collect_tsumo_13_temps(&self) -> Result<()> {
        let mut temp_files: Vec<FlatFileVec<u128>> = (0..NUM_ROUNDS)
            .map(|round| {
                FlatFileVec::<u128>::open_readonly(self.get_tsumo_temp_path(round * 2 + 1)).unwrap()
            })
            .collect();

        let mut tsumo_13_store =
            FlatFileVec::<u32>::open_or_create(self.dir.join("tsumo_13.dat"))?;

        const SHARD_SIZE: usize = 1 << 28;
        let mut hi_start = 0;
        while hi_start < NUM_HAND13 {
            log(format!("13: hi_start={:10}/{:10}", hi_start, NUM_HAND13));
            let size = SHARD_SIZE.min(NUM_HAND13 - hi_start);
            let hi_end = hi_start + size;
            let mut temp = vec![0u32; size * NUM_ROUNDS];
            for (r, ffv) in temp_files.iter_mut().enumerate() {
                let div = (136u128 - 13).pow(1 + r as u32);
                for (i, v) in ffv.get_range(hi_start, hi_end).unwrap().iter().enumerate() {
                    let k = v.leading_zeros().min(32);
                    temp[i * NUM_ROUNDS + r] =
                        u32::try_from((v << k) / (div >> (32 - k))).unwrap_or(u32::MAX);
                }
            }
            tsumo_13_store.extend(temp)?;
            hi_start = hi_end;
        }
        Ok(())
    }

    fn collect_tsumo_14_temps(&self) -> Result<()> {
        let mut temp_files: Vec<FlatFileVec<u128>> = (0..NUM_ROUNDS)
            .map(|round| {
                FlatFileVec::<u128>::open_readonly(self.get_tsumo_temp_path(round * 2)).unwrap()
            })
            .collect();

        let mut tsumo_14_store =
            FlatFileVec::<u32>::open_or_create(self.dir.join("tsumo_14.dat"))?;

        const SHARD_SIZE: usize = 1 << 28;
        let mut hi_start = 0;
        while hi_start < NUM_HAND14 {
            log(format!("14: hi_start={:10}/{:10}", hi_start, NUM_HAND14));
            let size = SHARD_SIZE.min(NUM_HAND14 - hi_start);
            let hi_end = hi_start + size;
            let mut temp = vec![0u32; size * NUM_ROUNDS];
            for (r, ffv) in temp_files.iter_mut().enumerate() {
                let div = (136u128 - 13).pow(r as u32);
                for (i, v) in ffv.get_range(hi_start, hi_end).unwrap().iter().enumerate() {
                    let k = v.leading_zeros().min(32);
                    temp[i * NUM_ROUNDS + r] =
                        u32::try_from((v << k) / (div >> (32 - k))).unwrap_or(u32::MAX);
                }
            }
            tsumo_14_store.extend(temp)?;
            hi_start = hi_end;
        }
        Ok(())
    }
}

fn debug(mut hand: Hand, dims: &[Dimension], converter: &HandConverter, dir: &Path) {
    println!("{:?}", hand);
    match hand.num_tiles() {
        13 => {
            println!("13");
            let hi = converter.encode_hand13_fast(&hand);
            println!("{:?}", converter.decode_hand13(hi));
            for round in 0..18 {
                let round = round * 2 + 1;
                println!("round={}", round);
                let mut tsumo_13 = FlatFileVec::<u128>::open_readonly(
                    dir.join(format!("tsumo_temp/{:02}.dat", round))
                )
                .unwrap();
                println!(
                    "tsumo: {}",
                    (tsumo_13.get(hi as usize).unwrap() as f64)
                        / ((136u128 - 13).pow((round + 1) / 2u32) as f64)
                );

                let shard_id = hi as usize / SHARD_SIZE;
                let idx = hi as usize % SHARD_SIZE;

                let mut total = 0;
                for dim_id in 0..Dimension::len() {
                    let mut metrics_13 = FlatFileVec::<u32>::open_readonly(
                        dir.join(format!("metrics_temp/{:02}/{:02}/{:03}.dat",
                        dim_id, round, shard_id))
                    )
                    .unwrap();
                    match Dimension::from_id(dim_id) {
                        Dimension::Shuntsu(Tile::Supai(_, _)) => {
                            total += metrics_13.get(idx).unwrap() as u64 * 3;
                        }
                        Dimension::Kotsu(Tile::Supai(_, _)) => {
                            total += metrics_13.get(idx).unwrap() as u64 * 3;
                        }
                        Dimension::Toitsu(Tile::Supai(_, _)) => {
                            total += metrics_13.get(idx).unwrap() as u64 * 2;
                        }
                        Dimension::Kotsu(Tile::Jihai(n)) => {
                            total += metrics_13.get(idx).unwrap() as u64
                                * 3
                                * hand.jihai[n as usize] as u64;
                        }
                        Dimension::Toitsu(Tile::Jihai(n)) => {
                            total += metrics_13.get(idx).unwrap() as u64
                                * 2
                                * hand.jihai[n as usize] as u64;
                        }
                        Dimension::Kokushi => {
                            total += metrics_13.get(idx).unwrap() as u64 * 14;
                        }
                        _ => unreachable!(),
                    }
                }
                println!("verify: {}", (total as f64) / (((1 << 30) as f64) * 14.0));
            }
        }
        14 => {
            println!("14");
            let hi = converter.encode_hand14_fast(&hand);
            println!("{:?}", converter.decode_hand14(hi));
            for round in 0..18 {
                let round = round * 2;
                println!("round={}", round);
                let mut tsumo_14 = FlatFileVec::<u128>::open_readonly(
                    dir.join(format!("tsumo_temp/{:02}.dat", round))
                )
                .unwrap();
                println!(
                    "tsumo: {}",
                    (tsumo_14.get(hi as usize).unwrap() as f64)
                        / ((136u128 - 13).pow(round / 2u32) as f64)
                );

                let shard_id = hi as usize / SHARD_SIZE;
                let idx = hi as usize % SHARD_SIZE;

                let mut total = 0;
                for dim_id in 0..Dimension::len() {
                    let mut metrics_14 = FlatFileVec::<u32>::open_readonly(
                        dir.join(format!("metrics_temp/{:02}/{:02}/{:03}.dat",
                        dim_id, round, shard_id))
                    )
                    .unwrap();
                    match Dimension::from_id(dim_id) {
                        Dimension::Shuntsu(Tile::Supai(_, _)) => {
                            total += metrics_14.get(idx).unwrap() as u64 * 3;
                        }
                        Dimension::Kotsu(Tile::Supai(_, _)) => {
                            total += metrics_14.get(idx).unwrap() as u64 * 3;
                        }
                        Dimension::Toitsu(Tile::Supai(_, _)) => {
                            total += metrics_14.get(idx).unwrap() as u64 * 2;
                        }
                        Dimension::Kotsu(Tile::Jihai(n)) => {
                            total += metrics_14.get(idx).unwrap() as u64
                                * 3
                                * hand.jihai[n as usize] as u64;
                        }
                        Dimension::Toitsu(Tile::Jihai(n)) => {
                            total += metrics_14.get(idx).unwrap() as u64
                                * 2
                                * hand.jihai[n as usize] as u64;
                        }
                        Dimension::Kokushi => {
                            total += metrics_14.get(idx).unwrap() as u64 * 14;
                        }
                        _ => unreachable!(),
                    }
                }
                println!("verify: {}", (total as f64) / (((1 << 30) as f64) * 14.0));
            }
        }
        _ => unreachable!(),
    }
}

fn main() {
}

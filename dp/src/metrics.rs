use std::collections;

use itertools::Itertools;
use rayon::prelude::*;

use common::mahjong::{Dimension, Hand, HandConverter, Metrics, Tile, NUM_HAND13, NUM_HAND14};

pub fn construct_agari_metrics(conv: &HandConverter) -> Vec<(u32, Metrics)> {
    let mut entries = Vec::new();

    // 4面子+1雀頭
    for mentsu_locations in (0..(21 + 28)).combinations_with_replacement(4) {
        let mut hand = Hand::new();
        for i in mentsu_locations.iter().copied() {
            if i < 21 {
                let suit = i / 7;
                let num = i % 7;
                hand.supai[suit][num] += 1;
                hand.supai[suit][num + 1] += 1;
                hand.supai[suit][num + 2] += 1;
            } else if i < 21 + 27 {
                let suit = (i - 21) / 9;
                let num = (i - 21) % 9;
                hand.supai[suit][num] += 3;
            } else {
                hand.jihai[3] += 1;
                hand.jihai[0] -= 1;
            }
        }
        if !hand.supai.iter().all(|l| l.iter().all(|v| *v <= 4)) || hand.jihai[0] > 7 {
            continue;
        }
        let mut memo = [0u8; Dimension::len()];
        for i in mentsu_locations {
            if i < 21 + 27 {
                memo[i] += 1;
            } else {
                memo[Dimension::Kotsu(Tile::Jihai(3)).to_id() as usize] = 1;
            }
        }
        for suit in 0..3 {
            for num in 0..9 {
                hand.supai[suit][num] += 2;
                if hand.supai[suit][num] <= 4 {
                    let (hi, trans) = conv.encode_hand14(&hand);
                    if trans == [0, 1, 2] {
                        let t =
                            Dimension::Toitsu(Tile::Supai(suit as u8, num as u8)).to_id() as usize;
                        memo[t] += 1;
                        entries.push((hi, memo.clone()));
                        memo[t] -= 1;
                    }
                }
                hand.supai[suit][num] -= 2;
            }
        }
        if hand.jihai[0] > 0 {
            hand.jihai[0] -= 1;
            hand.jihai[2] += 1;
            let (hi, trans) = conv.encode_hand14(&hand);
            if trans == [0, 1, 2] {
                let t = Dimension::Toitsu(Tile::Jihai(2)).to_id() as usize;
                memo[t] = 1;
                entries.push((hi, memo.clone()));
                memo[t] = 0;
            }
            hand.jihai[0] += 1;
            hand.jihai[2] -= 1;
        }
    }

    // 七対子
    for p in (0..34).combinations(7) {
        let jihai_temp = p.iter().copied().filter(|v| *v >= 27).collect_vec();
        if !jihai_temp.is_empty()
            && !(jihai_temp[0] == 27
                && jihai_temp
                    .iter()
                    .copied()
                    .zip(jihai_temp[1..].iter().copied())
                    .all(|(x, y)| x + 1 == y))
        {
            continue;
        }
        let mut hand = Hand::new();
        for v in p.iter().copied() {
            if v < 27 {
                hand.supai[v / 9][v % 9] += 2;
            } else {
                hand.jihai[2] += 1;
                hand.jihai[0] -= 1;
            }
        }

        let (hi, trans) = conv.encode_hand14(&hand);
        if trans == [0, 1, 2] {
            let mut memo = [0u8; Dimension::len()];
            for i in p {
                if i < 27 {
                    memo[Dimension::Toitsu(Tile::Supai((i / 9) as u8, (i % 9) as u8)).to_id()
                        as usize] += 1;
                } else {
                    memo[Dimension::Toitsu(Tile::Jihai(2)).to_id() as usize] = 1;
                }
            }
            entries.push((hi, memo));
        }
    }
    {
        let mut hand = Hand {
            supai: [
                [1, 0, 0, 0, 0, 0, 0, 0, 1],
                [1, 0, 0, 0, 0, 0, 0, 0, 1],
                [1, 0, 0, 0, 0, 0, 0, 0, 1],
            ],
            jihai: [0, 7, 0, 0, 0],
        };
        hand.supai[0][0] += 1;
        let mut kokushi = [0u8; Dimension::len()];
        kokushi[Dimension::Kokushi.to_id() as usize] = 1;
        entries.push((conv.encode_hand14_fast(&hand), kokushi.clone()));
        hand.supai[0][0] -= 1;
        hand.jihai[1] -= 1;
        hand.jihai[2] += 1;
        entries.push((conv.encode_hand14_fast(&hand), kokushi));
    }

    let mut agari_shapes: collections::HashMap<u32, (u32, [u32; Dimension::len()])> =
        collections::HashMap::new();
    for (hi, l) in entries {
        let out = agari_shapes
            .entry(hi)
            .or_insert((0, [0u32; Dimension::len()]));
        out.0 += 1;
        for (di, v) in l.iter().copied().enumerate() {
            out.1[di] += v as u32;
        }
    }
    let mut res = Vec::new();
    for (hand_id, (cnt, mut l)) in agari_shapes {
        // assert!(cnt <= 4, "cnt = {} for this one: {:?}", cnt, conv.decode_hand14(hi));
        l.iter_mut().for_each(|v| {
            *v = (u32::MAX as u64).min(((*v as u64) << (u32::BITS - 2)) / (cnt as u64)) as u32;
        });
        res.push((hand_id, Metrics { values: l }));
    }
    res
}

// [数牌の種類m/s/p][鏡像かどうか (i.e 123 <-> 789)]
pub fn process_13_to_14_supai(
    conv: &HandConverter,
    metrics_13: &[[[u32; 2]; 3]],
    output: &mut Vec<[[u32; 2]; 3]>,
    tsumo_13: &[u128],
    agari_metrics: &[(u32, [[u32; 2]; 3])],
) {
    assert_eq!(tsumo_13.len(), NUM_HAND13);
    assert_eq!(metrics_13.len(), NUM_HAND13);
    assert_eq!(output.len(), NUM_HAND14);
    let derive = |hand_id: usize| -> [[u32; 2]; 3] {
        let mut best = 0;
        let mut sum = [[0u64; 2]; 3];
        let mut total = 0u64;
        conv.decode_hand14(hand_id as u32)
            .for_each_discard_hand(|hand, cnt| {
                let cnt = cnt as u64;
                let (hi, trans) = conv.encode_hand13(hand);
                let hi = hi as usize;
                let p = tsumo_13[hi];
                if p > best {
                    best = p;
                    for (i, v) in trans.into_iter().enumerate() {
                        let (s, d) = if v >= 0 {
                            (v as usize, 0)
                        } else {
                            (!v as usize, 1)
                        };
                        sum[s][0] = cnt * metrics_13[hi][i][d] as u64;
                        sum[s][1] = cnt * metrics_13[hi][i][d ^ 1] as u64;
                    }
                    total = cnt;
                } else if p == best {
                    for (i, v) in trans.into_iter().enumerate() {
                        let (s, d) = if v >= 0 {
                            (v as usize, 0)
                        } else {
                            (!v as usize, 1)
                        };
                        sum[s][0] += cnt * metrics_13[hi][i][d] as u64;
                        sum[s][1] += cnt * metrics_13[hi][i][d ^ 1] as u64;
                    }
                    total += cnt;
                }
            });
        let mut out = [[0u32; 2]; 3];
        for i in 0..3 {
            for j in 0..2 {
                out[i][j] = (sum[i][j] / total) as u32;
            }
        }
        out
    };
    output
        .par_iter_mut()
        .enumerate()
        .for_each(|(hi, v)| *v = derive(hi));
    for &(hi, l) in agari_metrics {
        output[hi as usize] = l;
    }
}

pub fn process_13_to_14_jihai(
    conv: &HandConverter,
    metrics_13: &[[u32; 5]],
    output: &mut Vec<[u32; 5]>,
    tsumo_13: &[u128],
    agari_metrics: &[(u32, [u32; 5])],
) {
    assert_eq!(tsumo_13.len(), NUM_HAND13);
    assert_eq!(metrics_13.len(), NUM_HAND13);
    assert_eq!(output.len(), NUM_HAND14);
    let derive = |hand_id| {
        let mut best = 0;
        let mut sum = [0u64; 5];
        let mut total = 0u64;

        let mut hand14 = conv.decode_hand14(hand_id as u32);

        let hand14_jihai = hand14.jihai.clone();

        hand14.for_each_discard_hand(|hand13, cnt| {
            let cnt = cnt as u64;
            let hi = conv.encode_hand13_fast(hand13) as usize;
            let p = tsumo_13[hi];
            if p > best {
                best = p;
                for (i, s) in sum.iter_mut().enumerate() {
                    if hand14_jihai[i] <= hand13.jihai[i] {
                        *s = cnt * metrics_13[hi][i] as u64;
                    } else {
                        // jihai[i]を捨牌
                        *s = (cnt - i as u64) * metrics_13[hi][i] as u64
                            + i as u64 * metrics_13[hi][i - 1] as u64;
                    }
                }
                total = cnt;
            } else if p == best {
                for (i, s) in sum.iter_mut().enumerate() {
                    if hand14_jihai[i] <= hand13.jihai[i] {
                        *s += cnt * metrics_13[hi][i] as u64;
                    } else {
                        // jihai[i]を捨牌
                        *s += (cnt - i as u64) * metrics_13[hi][i] as u64
                            + i as u64 * metrics_13[hi][i - 1] as u64;
                    }
                }
                total += cnt;
            }
        });
        let mut out = [0u32; 5];
        for (i, v) in sum.into_iter().enumerate() {
            out[i] = (v / total) as u32;
        }
        out
    };
    output
        .par_iter_mut()
        .enumerate()
        .for_each(|(hi, v)| *v = derive(hi));
    for &(hi, l) in agari_metrics {
        output[hi as usize] = l;
    }
}

pub fn process_13_to_14_kokushi(
    conv: &HandConverter,
    metrics_13: &[u32],
    output: &mut Vec<u32>,
    tsumo_13: &[u128],
    agari_metrics: &[(u32, u32)],
) {
    assert_eq!(tsumo_13.len(), NUM_HAND13);
    assert_eq!(metrics_13.len(), NUM_HAND13);
    assert_eq!(output.len(), NUM_HAND14);
    let derive = |hand_id| {
        let mut best = 0;
        let mut sum = 0u64;
        let mut total = 0u64;
        conv.decode_hand14(hand_id as u32)
            .for_each_discard_hand(|hand, cnt| {
                let cnt = cnt as u64;
                let hi = conv.encode_hand13_fast(hand) as usize;
                let p = tsumo_13[hi];
                if p > best {
                    best = p;
                    sum = cnt * metrics_13[hi] as u64;
                    total = cnt;
                } else if p == best {
                    sum += cnt * metrics_13[hi] as u64;
                    total += cnt;
                }
            });
        (sum / total) as u32
    };
    output
        .par_iter_mut()
        .enumerate()
        .for_each(|(hi, v)| *v = derive(hi));
    for &(hi, l) in agari_metrics {
        output[hi as usize] = l;
    }
}

pub fn process_14_to_13_supai(
    conv: &HandConverter,
    metrics_14: &[[[u32; 2]; 3]],
    output: &mut Vec<[[u32; 2]; 3]>,
) {
    assert_eq!(metrics_14.len(), NUM_HAND14);
    assert_eq!(output.len(), NUM_HAND13);
    let derive = |hand_id| {
        let mut sum = [[0u64; 2]; 3];
        conv.decode_hand13(hand_id as u32)
            .for_each_draw_hand(|hand, cnt| {
                let cnt = cnt as u64;
                let (hi, trans) = conv.encode_hand14(hand);
                let hi = hi as usize;
                for (i, v) in trans.into_iter().enumerate() {
                    let (s, d) = if v >= 0 {
                        (v as usize, 0)
                    } else {
                        (!v as usize, 1)
                    };
                    sum[s][0] += cnt * metrics_14[hi][i][d] as u64;
                    sum[s][1] += cnt * metrics_14[hi][i][d ^ 1] as u64;
                }
            });
        let mut out = [[0u32; 2]; 3];
        for i in 0..3 {
            for j in 0..2 {
                out[i][j] = (sum[i][j] / (136u64 - 13)) as u32;
            }
        }
        out
    };
    output
        .par_iter_mut()
        .enumerate()
        .for_each(|(hi, v)| *v = derive(hi));
}

pub fn process_14_to_13_jihai(
    conv: &HandConverter,
    metrics_14: &[[u32; 5]],
    output: &mut Vec<[u32; 5]>,
) {
    assert_eq!(metrics_14.len(), NUM_HAND14);
    assert_eq!(output.len(), NUM_HAND13);
    let derive = |hand_id| {
        let mut hand13 = conv.decode_hand13(hand_id as u32);

        let hand13_jihai = hand13.jihai.clone();
        let mut sum = [0u64; 5];
        hand13.for_each_draw_hand(|hand14, cnt| {
            let cnt = cnt as u64;
            let hi = conv.encode_hand14_fast(hand14) as usize;
            for (i, s) in sum.iter_mut().enumerate() {
                if hand13_jihai[i] <= hand14.jihai[i] {
                    *s += cnt * metrics_14[hi][i] as u64;
                } else {
                    // jihai[i]をツモでjihai[i]--
                    *s += cnt
                        * ((hand13_jihai[i] as u64 - 1) * metrics_14[hi][i] as u64
                            + metrics_14[hi][i + 1] as u64)
                        / (hand13_jihai[i] as u64);
                }
            }
        });
        let mut out = [0u32; 5];
        for i in 0..5 {
            out[i] = (sum[i] / (136u64 - 13)) as u32;
        }
        out
    };
    output
        .par_iter_mut()
        .enumerate()
        .for_each(|(hi, v)| *v = derive(hi));
}

pub fn process_14_to_13_kokushi(conv: &HandConverter, metrics_14: &[u32], output: &mut Vec<u32>) {
    assert_eq!(metrics_14.len(), NUM_HAND14);
    assert_eq!(output.len(), NUM_HAND13);
    let derive = |hand_id| {
        let mut sum = 0u64;
        conv.decode_hand13(hand_id as u32)
            .for_each_draw_hand(|hand, cnt| {
                sum += cnt as u64 * metrics_14[conv.encode_hand14_fast(hand) as usize] as u64;
            });
        (sum / (136u64 - 13)) as u32
    };
    output
        .par_iter_mut()
        .enumerate()
        .for_each(|(hi, v)| *v = derive(hi));
}

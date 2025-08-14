use itertools::Itertools;
use rayon::prelude::*;

use common::mahjong::{Hand, HandConverter, NUM_HAND13, NUM_HAND14};

// 残り０巡のdp14を計算する。残り０巡のため、すでに和了形になっている手のみを考えればよい。
pub fn dp14_r0(conv: &HandConverter) -> Vec<u128> {
    let mut res = vec![0; NUM_HAND14];

    // 4面子+1雀頭
    for mentsu_locations in (0..(21 + 28)).combinations_with_replacement(4) {
        let mut hand = Hand::new();
        for i in mentsu_locations {
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
        if !hand.supai.iter().all(|l| l.iter().all(|v| *v <= 4)) {
            continue
        }
        for suit in 0..3 {
            for num in 0..9 {
                hand.supai[suit][num] += 2;
                if hand.supai[suit][num] <= 4 {
                    res[conv.encode_hand14_fast(&hand) as usize] = 1;
                }
                hand.supai[suit][num] -= 2;
            }
        }
        hand.jihai[2] += 1;
        hand.jihai[0] -= 1;
        res[conv.encode_hand14_fast(&hand) as usize] = 1;
        hand.jihai[0] += 1;
        hand.jihai[2] -= 1;
    }

    // 七対子
    for p in (0..34).combinations(7) {
        let mut hand = Hand::new();
        for v in p {
            if v < 27 {
                hand.supai[v / 9][v % 9] += 2;
            } else {
                hand.jihai[0] -= 1;
                hand.jihai[2] += 1;
            }
        }
        res[conv.encode_hand14_fast(&hand) as usize] = 1;
    }

    // 国士無双
    let mut kokushi = Hand {
        supai: [
            [1, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 0, 1],
        ],
        jihai: [0, 7, 0, 0, 0],
    };
    kokushi.supai[0][0] += 1;
    res[conv.encode_hand14_fast(&kokushi) as usize] = 1;
    kokushi.supai[0][0] -= 1;
    kokushi.jihai[1] -= 1;
    kokushi.jihai[2] += 1;
    res[conv.encode_hand14_fast(&kokushi) as usize] = 1;
    res
}

// dp14からdp13を計算する。13牌にランダムに１牌を積もって14牌のDPを計算する。
pub fn dp14_to_dp13(conv: &HandConverter, dp14: &[u128]) -> Vec<u128> {
    let derive = |hand_id: usize| {
        let mut total = 0;
        conv.decode_hand13(hand_id as u32).for_each_draw_hand(
            |hand, cnt| total += dp14[conv.encode_hand14_fast(hand) as usize]*(cnt as u128));
        total
    };
    (0..NUM_HAND13).into_par_iter().map(derive).collect()
}

// dp13からdp14を計算する。14牌から最適な１牌を選んで捨てることで13牌のDPを計算する。
pub fn dp13_to_dp14(conv: &HandConverter, dp13: &[u128], agari_hands: &[u32], one: u128) -> Vec<u128> {
    let derive = |hand_id: usize| {
        let mut best = 0;
        conv.decode_hand14(hand_id as u32).for_each_discard_hand(
            |hand, _| best = best.max(dp13[conv.encode_hand13_fast(hand) as usize]));
        best
    };
    let mut out: Vec<u128> = (0..NUM_HAND14).into_par_iter().map(derive).collect();
    for hi in agari_hands {
        out[*hi as usize] = one;
    }
    out
}

// 和了確率を計算する。デバッグ用
pub fn check(conv: &HandConverter, table: &Vec<u128>) -> u128 {
    // https://riichi-tools.moe/tsumo-prob?hand=678m56p233789s11z
    let hand = Hand {
        supai: [
            [0,0,0,0,0,1,1,1,0],
            [0,0,0,0,1,1,0,0,0],
            [0,1,2,0,0,0,1,1,1],
        ],
        jihai: [6,0,1,0,0],
    };
    table[conv.encode_hand13_fast(&hand) as usize]
}

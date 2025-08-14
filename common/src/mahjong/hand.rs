use std::path::Path;

use itertools::{Itertools, MultiProduct};
use serde::{Deserialize, Serialize};

use crate::{io, mahjong::Tile};

use anyhow::Result;

pub const NUM_HAND13: usize = 322565293;
pub const NUM_HAND14: usize = 923597122;

fn product_repeat<I>(it: I, repeat: usize) -> MultiProduct<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    (0..repeat).map(|_| it.clone()).multi_cartesian_product()
}

fn to_octal<T: Iterator<Item = u32>>(iterator: T) -> u32 {
    let mut res = 0;
    for v in iterator {
        res <<= 3;
        res |= v;
    }
    res
}

fn from_octal(mut v: u32, dst: &mut [u8]) {
    for t in dst.iter_mut().rev() {
        *t = (v & 7) as u8;
        v >>= 3;
    }
}

// 手牌を表す構造体
// supai[0][k] は萬子のk番目の牌の枚数
// supai[1][k] は筒子のk番目の牌の枚数
// supai[2][k] は索子のk番目の牌の枚数
// jihai[k] は手牌にk枚ある字牌の種類数
#[derive(Clone, Debug)]
pub struct Hand {
    pub supai: [[u8; 9]; 3],
    pub jihai: [u8; 5],
}

impl Hand {
    pub fn new() -> Self {
        Self {
            supai: [[0; 9]; 3],
            jihai: [7, 0, 0, 0, 0],
        }
    }

    pub fn from_tiles(tiles: &[Tile]) -> Self {
        Self::from_tiles_with_jihai_cnt(tiles).0
    }

    pub fn from_tiles_with_jihai_cnt(tiles: &[Tile]) -> (Self, [usize; 7]) {
        let mut hand = Self::new();
        let mut jihai_cnt = [0; 7];
        for tile in tiles {
            match tile {
                Tile::Supai(suit, num) => hand.supai[*suit as usize][*num as usize] += 1,
                Tile::Jihai(num) => jihai_cnt[*num as usize] += 1,
            }
        }
        hand.jihai[0] = 0;
        for cnt in jihai_cnt {
            hand.jihai[cnt] += 1;
        }
        (hand, jihai_cnt)
    }

    pub fn for_each_discard_hand<F: FnMut(&Hand, u8)>(&mut self, mut op: F) {
        for suit in 0..3 {
            for num in 0..9 {
                let cnt = self.supai[suit][num];
                if cnt > 0 {
                    self.supai[suit][num] -= 1;
                    op(self, cnt);
                    self.supai[suit][num] += 1;
                }
            }
        }
        for i in 1..5 {
            let cnt = self.jihai[i];
            if cnt > 0 {
                self.jihai[i] -= 1;
                self.jihai[i - 1] += 1;
                op(self, cnt * (i as u8));
                self.jihai[i - 1] -= 1;
                self.jihai[i] += 1;
            }
        }
    }

    pub fn for_each_draw_hand<F: FnMut(&Hand, u8)>(&mut self, mut op: F) {
        for suit in 0..3 {
            for num in 0..9 {
                let cnt = self.supai[suit][num];
                if cnt < 4 {
                    self.supai[suit][num] += 1;
                    op(self, 4 - cnt);
                    self.supai[suit][num] -= 1;
                }
            }
        }
        for i in 0..4 {
            let cnt = self.jihai[i];
            if cnt > 0 {
                self.jihai[i] -= 1;
                self.jihai[i + 1] += 1;
                op(self, ((4 - i) as u8) * cnt);
                self.jihai[i + 1] -= 1;
                self.jihai[i] += 1;
            }
        }
    }

    pub fn num_tiles(&self) -> usize {
        self.supai
            .iter()
            .map(|&v| v.iter().map(|&v| v as usize).sum::<usize>())
            .sum::<usize>()
            + self
                .jihai
                .iter()
                .enumerate()
                .map(|(i, &v)| v as usize * i)
                .sum::<usize>()
    }
}

/// # Supai Encoding
/// Represent number of each supai with 3 bits. Bit-pack them like `(cnt[1], cnt[2], ..., cnt[9])`.
///
/// # Jihai Encoding
/// Bit-pack of `(# of solo jihai, # of toitsu jihai, # of kotsu jihai, # of kantsu jihai)`.
///
/// # Hand Encoding
/// * 00-17 bits: smallest supai encoding
/// * 18-35 bits: mid supai encoding
/// * 36-53 bits: largest supai encoding
/// * 54-62 bits: jihai encoding
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct HandConverter {
    su_lookup: Vec<u32>,
    ji_lookup: Vec<u32>,
    hand13_lookup: Vec<u64>,
    hand14_lookup: Vec<u64>,
}

impl HandConverter {
    pub fn empty() -> HandConverter {
        HandConverter {
            su_lookup: vec![],
            ji_lookup: vec![],
            hand13_lookup: vec![],
            hand14_lookup: vec![],
        }
    }
    pub fn new() -> HandConverter {
        let mut su_lookup = Vec::with_capacity(203122);
        let mut ji_lookup = Vec::with_capacity(177);

        let mut su_memo: [Vec<u32>; 15] = Default::default();
        let mut ji_memo: [Vec<u32>; 15] = Default::default();

        for p in product_repeat(0..5, 9) {
            let cnt = p.iter().copied().sum::<u32>();
            if cnt > 14 {
                continue;
            }
            let x = to_octal(p.iter().copied());
            let y = to_octal(p.iter().copied().rev());
            if x <= y {
                su_lookup.push(x);
                su_memo[cnt as usize].push(x);
            }
        }
        su_lookup.sort_unstable();
        for l in su_memo.iter_mut() {
            for v in l.iter_mut() {
                *v = su_lookup.binary_search(v).unwrap() as u32;
            }
        }

        for p in (0..8).combinations_with_replacement(4) {
            let jihai = [p[0], p[1] - p[0], p[2] - p[1], p[3] - p[2], 7 - p[3]];
            let cnt = jihai
                .iter()
                .copied()
                .enumerate()
                .map(|(i, v)| (i as u32) * v)
                .sum::<u32>();
            if cnt <= 14 {
                let code = to_octal(jihai.iter().copied());
                ji_lookup.push(code);
                ji_memo[cnt as usize].push(code);
            }
        }
        ji_lookup.sort_unstable();
        for l in ji_memo.iter_mut() {
            for v in l.iter_mut() {
                *v = ji_lookup.binary_search(v).unwrap() as u32;
            }
        }
        let generate_all_hands = |tiles: usize, expected_len: usize| {
            let mut hands = Vec::with_capacity(expected_len);
            let mut helper = |a: &u32, b: &u32, c: &u32, r: usize| {
                let mut su_codes = [*a as u64, *b as u64, *c as u64];
                su_codes.sort_unstable();
                let x = su_codes[0] | (su_codes[1] << 18) | (su_codes[2] << 36);
                for d in ji_memo[r].iter() {
                    hands.push(x | ((*d as u64) << 54));
                }
            };

            for p in (0..(tiles + 1)).combinations_with_replacement(3) {
                let q = [p[0], p[1] - p[0], p[2] - p[1]];
                if (q[0] > q[1]) || q[1] > q[2] {
                    continue;
                }
                let r = tiles - p[2];

                if q[0] == q[2] {
                    for abc in su_memo
                        .get(q[0])
                        .unwrap()
                        .iter()
                        .combinations_with_replacement(3)
                    {
                        helper(abc[0], abc[1], abc[2], r);
                    }
                } else if q[0] == q[1] {
                    for ab in su_memo
                        .get(q[0])
                        .unwrap()
                        .iter()
                        .combinations_with_replacement(2)
                    {
                        for c in su_memo.get(q[2]).unwrap() {
                            helper(ab[0], ab[1], c, r);
                        }
                    }
                } else if q[1] == q[2] {
                    for bc in su_memo
                        .get(q[1])
                        .unwrap()
                        .iter()
                        .combinations_with_replacement(2)
                    {
                        for a in su_memo.get(q[0]).unwrap() {
                            helper(a, bc[0], bc[1], r);
                        }
                    }
                } else {
                    for abc in q
                        .map(|t| su_memo.get(t).unwrap())
                        .iter()
                        .copied()
                        .multi_cartesian_product()
                    {
                        helper(abc[0], abc[1], abc[2], r);
                    }
                }
            }
            hands.sort_unstable();
            hands
        };

        println!("Generating hand13_lookup...");
        let hand13_lookup = generate_all_hands(13, NUM_HAND13);
        println!("Generating hand14_lookup...");
        let hand14_lookup = generate_all_hands(14, NUM_HAND14);
        println!("Done!");
        assert_eq!(hand13_lookup.len(), NUM_HAND13);
        assert_eq!(hand14_lookup.len(), NUM_HAND14);
        HandConverter {
            su_lookup,
            ji_lookup,
            hand13_lookup,
            hand14_lookup,
        }
    }

    pub fn save_as_file<P: AsRef<Path>>(&self, filename: P) -> Result<()> {
        io::save_object(filename, self)
    }

    pub fn load_from_file<P: AsRef<Path>>(filename: P) -> Result<Self> {
        io::load_object(filename)
    }

    fn encode_into_key(&self, hand: &Hand) -> (u64, [i8; 3]) {
        let mut memo = [(0u64, 0i8); 3];
        for i in 0..3usize {
            let p = to_octal(hand.supai[i].iter().map(|&v| v as u32));
            let q = to_octal(hand.supai[i].iter().rev().map(|&v| v as u32));
            if p <= q {
                memo[i] = (self.su_lookup.binary_search(&p).unwrap() as u64, i as i8);
            } else {
                memo[i] = (self.su_lookup.binary_search(&q).unwrap() as u64, !(i as i8));
            }
        }
        memo.sort_unstable();
        let trans = core::array::from_fn(|i| memo[i].1);
        let mut key = self
            .ji_lookup
            .binary_search(&to_octal(hand.jihai.iter().map(|&v| v as u32)))
            .unwrap() as u64;
        for (v, _) in memo.iter().rev() {
            key <<= 18;
            key |= v;
        }
        (key, trans)
    }

    fn encode_into_key_fast(&self, hand: &Hand) -> u64 {
        let mut memo = [0u64; 3];
        for i in 0..3usize {
            let p = to_octal(hand.supai[i].iter().map(|&v| v as u32));
            let q = to_octal(hand.supai[i].iter().rev().map(|&v| v as u32));
            memo[i] = self.su_lookup.binary_search(&(p.min(q))).unwrap() as u64;
        }
        memo.sort_unstable();
        let mut key = self
            .ji_lookup
            .binary_search(&to_octal(hand.jihai.iter().map(|&v| v as u32)))
            .unwrap() as u64;
        for v in memo.iter().rev() {
            key <<= 18;
            key |= v;
        }
        key
    }

    fn decode_from_key(&self, mut key: u64) -> Hand {
        let mut supai = [[0u8; 9]; 3];
        let mut jihai = [0u8; 5];
        let x = self.su_lookup[(key & 0x3ffff) as usize];
        key >>= 18;
        from_octal(x, supai.get_mut(0).unwrap());
        let x = self.su_lookup[(key & 0x3ffff) as usize];
        key >>= 18;
        from_octal(x, supai.get_mut(1).unwrap());
        let x = self.su_lookup[(key & 0x3ffff) as usize];
        key >>= 18;
        from_octal(x, supai.get_mut(2).unwrap());
        let x = self.ji_lookup[key as usize];
        from_octal(x, &mut jihai);
        Hand { supai, jihai }
    }

    /// Encode a hand with 14 tiles into a u32. This also returns a translation done on supai.
    ///
    /// # Arguments
    ///
    /// * `hand` - Hand with 14 tiles
    ///
    /// # Returns
    /// * `hand index`
    /// * `translation` - Denotes how the suits were reordered and which numbers were reversed during encoding
    ///
    /// # Translation list
    /// Negative value means that the number is reversed. You should bit-negate it to find out the original suit.
    ///
    /// For example, if it is `[!1, 2, 0]`, then:
    /// * suit 0 of the encoded is suit 1 of the original, but number is reversed (1 -> 9, 9 -> 1)
    /// * suit 1 of the encoded is suit 2 of the original
    /// * suit 2 of the encoded is suit 0 of the original
    pub fn encode_hand14(&self, hand: &Hand) -> (u32, [i8; 3]) {
        let (key, trans) = self.encode_into_key(hand);
        (
            self.hand14_lookup.binary_search(&key).unwrap() as u32,
            trans,
        )
    }
    pub fn encode_hand14_fast(&self, hand: &Hand) -> u32 {
        let key = self.encode_into_key_fast(hand);
        self.hand14_lookup.binary_search(&key).unwrap() as u32
    }

    /// Encode a hand with 13 tiles into a u32. This also returns a translation done on supai.
    ///
    /// # Arguments
    ///
    /// * `hand` - Hand with 13 tiles
    ///
    /// # Returns
    /// * `hand index`
    /// * `translation` - Denotes how the suits were reordered and which numbers were reversed during encoding
    ///
    /// # Translation list
    /// Negative value means that the number is reversed. You should bit-not it to find out the original suit.
    ///
    /// For example, if it is `[!1, 2, 0]`, then:
    /// * suit 0 of the encoded is suit 1 of the original, but number is reversed (1 -> 9, 9 -> 1)
    /// * suit 1 of the encoded is suit 2 of the original
    /// * suit 2 of the encoded is suit 0 of the original
    pub fn encode_hand13(&self, hand: &Hand) -> (u32, [i8; 3]) {
        let (key, trans) = self.encode_into_key(hand);
        (
            self.hand13_lookup.binary_search(&key).unwrap() as u32,
            trans,
        )
    }
    pub fn encode_hand13_fast(&self, hand: &Hand) -> u32 {
        let key = self.encode_into_key_fast(hand);
        self.hand13_lookup.binary_search(&key).unwrap() as u32
    }

    pub fn decode_hand14(&self, encoded: u32) -> Hand {
        self.decode_from_key(self.hand14_lookup[encoded as usize])
    }

    pub fn decode_hand13(&self, encoded: u32) -> Hand {
        self.decode_from_key(self.hand13_lookup[encoded as usize])
    }
}


pub fn parse_hand_str(s: &str) -> Result<Vec<Tile>> {
    let mut tiles = Vec::new();
    let mut mode = 'z';
    for c in s.chars().rev() {
        match c {
            'm' | 'p' | 's' | 'z' => {mode = c},
            '1'..='9' => {
                match mode {
                    'm' => tiles.push(Tile::Supai(0, (c as u8 - '1' as u8) as u8)),
                    'p' => tiles.push(Tile::Supai(1, (c as u8 - '1' as u8) as u8)),
                    's' => tiles.push(Tile::Supai(2, (c as u8 - '1' as u8) as u8)),
                    'z' => tiles.push(Tile::Jihai((c as u8 - '1' as u8) as u8)),
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(anyhow::anyhow!("Invalid character: {}", c));
            }
        }
    }
    Ok(tiles)
}
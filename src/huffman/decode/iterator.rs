use crate::huffman::decode::symboltable::prepare_lookup;
use crate::model::Model;
use log::debug;
use std::collections::LinkedList;
use succinct::rank::BitRankSupport;
use succinct::rsdict::RsDict;

/// The Decoder<I> struct decodes iterable data structures
#[derive(Debug)]
pub struct Decoder<I> {
    data: I,
    buffer: u64,
    vault: u64,
    sentinel: u64,
    remaining_outputbytes: u64,
    rbv: RsDict,
    table: Vec<(u8, u8)>,
    _reserve: LinkedList<u8>,
    _vaultstatus: u64,
    _bufferstatus: u64,
}

fn initiate_buffer(iter: &mut impl Iterator<Item = u8>) -> (u64, u64) {
    let mut result = 0u64;
    let mut used = 0u64;

    while used < 64 {
        if let Some(value) = iter.next() {
            result += (value as u64) << (56 - used);
            used += 8;
        } else {
            break;
        }
    }
    (result, used)
}

fn initiate_sentinel(sentinel: u64) -> u64 {
    // TODO Remove constraint
    assert!(sentinel <= 8);
    sentinel
}

fn initiate_reserve() -> LinkedList<u8> {
    LinkedList::<u8>::new()
}

impl<I: Iterator<Item = u8>> Decoder<I> {
    pub fn new<M: Model>(mut iter: I, model: &M, output: u64) -> Self {
        let (buffer, bufferstatus) = initiate_buffer(&mut iter);
        Decoder {
            buffer: buffer,
            data: iter,
            _vaultstatus: 0,
            _bufferstatus: bufferstatus,
            vault: 0,
            sentinel: initiate_sentinel(model.sentinel() as u64),
            _reserve: initiate_reserve(),
            remaining_outputbytes: output,
            // TODO Move rbv and table into own struct and trait for better overview
            rbv: {
                let (_, v) = prepare_lookup(&model.to_btreemap());
                v
            },
            table: {
                let (t, _) = prepare_lookup(&model.to_btreemap());
                t
            },
        }
    }
}

impl<I: Iterator<Item = u8>> Decoder<I> {
    fn consume_buffer(&mut self) -> Option<u8> {
        debug!(
            "Consuming b{:064b} v{:064b} {} {}",
            self.buffer, self.vault, self._vaultstatus, self._bufferstatus
        );
        let lookup_value = self.buffer >> (64 - self.sentinel);
        let (cut, sym) = self.get_cut_and_symbol(lookup_value);
        if cut as u64 > self._bufferstatus {
            return None;
        }
        if cut <= self._vaultstatus as usize {
            // normal process
            self.buffer <<= cut;
            self.buffer += self.vault >> (64 - cut);
            self.vault <<= cut;
            self._vaultstatus -= cut as u64;
            return Some(sym);
        } else if self._vaultstatus > 0 {
            // TODO Same as above might be just to a min(cut,vault)
            self.buffer <<= cut;
            self.buffer += self.vault >> (64 - self._vaultstatus);
            self._bufferstatus -= cut as u64 - self._vaultstatus;
            self.vault <<= self._vaultstatus;
            self._vaultstatus -= self._vaultstatus;
            return Some(sym);
        } else {
            self.buffer <<= cut;
            self._bufferstatus -= cut as u64;
            return Some(sym);
        }
    }
    fn empty_vault(&mut self) {
        while self.vault & 0x00FF_FFFF_FFFF_FFFF > 0 {
            let lookup_value = self.buffer >> (64 - self.sentinel);
            let (cut, sym) = self.get_cut_and_symbol(lookup_value);
            assert!(cut as u64 <= self._vaultstatus);
            self.buffer <<= cut;
            self.buffer += self.vault >> (64 - cut);
            self.vault <<= cut;
            self._vaultstatus -= cut as u64;
            self._reserve.push_back(sym);
        }
    }
    fn get_cut_and_symbol(&mut self, val: u64) -> (usize, u8) {
        let pos = self.rbv.rank1(val + 1) as usize - 1;
        let (sym, length) = self.table[pos];
        debug!("Cut {} Symbol {:b}", length, sym);
        (length as usize, sym)
    }
}

impl<I: Iterator<Item = u8>> Iterator for Decoder<I> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_outputbytes == 0 {
            debug!("Finished decoding");
            debug!("Buffer {:064b} Vault {:064b}", self.buffer, self.vault);
            return None;
        }
        if let Some(val) = self.data.next() {
            // Inner data source still not empty
            debug!(
                "Buffer {:064b} Read byte {:08b} {:?}",
                self.buffer, val, self._reserve
            );

            // Check vault fill
            if self.vault & 0x0000_0000_FFFF_FFFF > 0 {
                self.empty_vault();
                debug!("Reserve {:?}", self._reserve)
            };

            // TODO Starting here a lot of overlap with empty_vault()
            // move value to vault
            self.vault += (val as u64) << (64 - self._vaultstatus - 8);
            self._vaultstatus += 8;

            // decode word
            let lookup_value = self.buffer >> (64 - self.sentinel);
            let (cut, sym) = self.get_cut_and_symbol(lookup_value);
            assert!(cut as u64 <= self._vaultstatus);

            // fill buffer from vault
            self.buffer <<= cut;
            self.buffer += self.vault >> (64 - cut);

            // update vault
            self.vault <<= cut;
            self._vaultstatus -= cut as u64;

            // TODO Might be optimised using .or_else()
            match self._reserve.pop_front() {
                Some(from_reserve) => {
                    self._reserve.push_back(sym);
                    self.remaining_outputbytes -= 1;
                    return Some(from_reserve);
                }
                None => {
                    self.remaining_outputbytes -= 1;
                    return Some(sym);
                }
            }
        } else if let Some(reserve) = self._reserve.pop_front() {
            // Inner data source empty. First output reserve
            self.remaining_outputbytes -= 1;
            return Some(reserve);
        } else {
            // Finish output by consuming buffer
            self.remaining_outputbytes -= 1;
            self.consume_buffer()
        }
    }
}

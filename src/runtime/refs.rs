#[derive(Clone, Copy)]
pub struct IORingTaskRef {
    tid: u32,
    tidx: usize,
}

pub struct IORingCompleterRef {
    cid: u32,
    cidx: usize,
}

impl IORingTaskRef {
    pub fn new(tidx: usize, tid: u32) -> Self {
        Self { tid, tidx }
    }

    pub fn tid(&self) -> u32 {
        self.tid
    }

    pub fn tidx(&self) -> usize {
        self.tidx
    }
}

impl IORingCompleterRef {
    pub fn new(cidx: usize, cid: u32) -> Self {
        Self { cid, cidx }
    }

    pub fn decode(val: u64) -> Self {
        Self {
            cid: ((val >> 32) & 0xffffffff) as u32,
            cidx: (val & 0xffffffff) as usize,
        }
    }

    pub fn cid(&self) -> u32 {
        self.cid
    }

    pub fn cidx(&self) -> usize {
        self.cidx
    }

    pub fn encode(&self) -> u64 {
        let hi = ((self.cid as u64) & 0xffffffff) << 32;
        let low = (self.cidx as u64) & 0xffffffff;

        hi | low
    }
}

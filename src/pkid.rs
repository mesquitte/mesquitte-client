use std::collections::{hash_map, HashMap};

use crate::token::Token;

const PKID_MAX: u16 = 65535;
const PKID_MIN: u16 = 1;

pub struct PacketIds {
    index: HashMap<u16, Token>,
    last_issued_id: u16,
}

impl PacketIds {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            last_issued_id: PKID_MIN,
        }
    }

    // pub fn claim_id(&mut self, )

    pub fn get_id(&mut self, token: Token) -> u16 {
        let mut i = self.last_issued_id;
        let mut looped = false;
        loop {
            i += 1;
            if i == 0 {
                i += 1;
                looped = true;
            }
            if let hash_map::Entry::Vacant(e) = self.index.entry(i) {
                e.insert(token);
                self.last_issued_id = i;
                return i;
            }
            if (looped && i == self.last_issued_id) || (self.last_issued_id == 0 && i == PKID_MAX) {
                return 0;
            }
        }
    }

    pub fn get_token(&mut self, id: u16) -> Option<&mut Token> {
        self.index.get_mut(&id)
    }

    pub fn free_id(&mut self, id: &u16) {
        self.index.remove(id);
    }
}

use std::collections::{hash_map, HashMap};

use crate::{error::MqttError, token::Token};

pub const PKID_MAX: u16 = 65535;
const PKID_MIN: u16 = 1;

#[derive(Clone)]
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

    pub fn claim_id(&mut self, token: Token, id: u16) {
        if !self.index.contains_key(&id) {
            self.index.insert(id, token);
        } else {
            let old = self.index.get_mut(&id).unwrap();
            old.flow_complete();
            self.index.insert(id, token);
        }
        if id > self.last_issued_id {
            self.last_issued_id = id;
        }
    }

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

    pub fn clean_up(&mut self) {
        for (_, token) in self.index.iter_mut() {
            token.set_error(MqttError::ConnectionLost);
        }
        self.index.clear();
    }
}

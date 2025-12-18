//! Transaction parsing utilities

use crate::Result;

/// Parse transaction from raw bytes or JSON
pub fn parse_transaction(_data: &[u8]) -> Result<crate::data_source::Transaction> {
    // TODO
    Err(crate::Error::custom("Not implemented"))
}

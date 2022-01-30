use anchor_lang::prelude::*;

#[error]
pub enum FortuneError {
    #[msg("Bid is too low")]
    BidTooLow,
    #[msg("Only seller can change ask")]
    InvalidAskAuth,
    #[msg("Locked listing")]
    LockedListing,
    #[msg("Ask cannot be less than zero")]
    ZeroAsk,
}

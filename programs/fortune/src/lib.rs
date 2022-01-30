use anchor_lang::prelude::*;
use anchor_spl::token;
use anchor_spl::token::{Mint, Token, TokenAccount};
use solana_program::program::invoke_signed;
use solana_program::system_instruction;
use spl_token::instruction::sync_native;

declare_id!("rTxHpHTgJ6ZeWgjAX9eEpS92Ei6SxD86rh1u55H6pWv");

mod error;

#[program]
pub mod fortune {
    use super::*;
    // TODO: Globals account
    const FEE_SCALAR: u64 = 1000;

    // Create a new market, permisionless
    pub fn create_market(
        ctx: Context<CreateMarket>,
        listing_fee: u64,
        _market_vault_bump: u8,
    ) -> ProgramResult {
        // Set market data
        ctx.accounts.market.authority = ctx.accounts.signer.key();
        ctx.accounts.market.lamport_vault = ctx.accounts.market_vault.key();
        ctx.accounts.market.listing_fee = listing_fee;
        Ok(())
    }
    // Create a new listing within a market
    pub fn create_listing(
        ctx: Context<CreateListing>,
        ask: u64,
        _listing_bump: u8,
        _nft_vault_bump: u8,
    ) -> ProgramResult {
        // Set listing data
        ctx.accounts.listing.market = ctx.accounts.market.key();
        ctx.accounts.listing.seller = ctx.accounts.signer.key();
        ctx.accounts.listing.ask = ask;
        ctx.accounts.listing.nft_mint = ctx.accounts.nft_mint.key();
        ctx.accounts.listing.lock = false;

        // Transfer NFT to vault
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.nft_account.to_account_info(),
                    to: ctx.accounts.nft_vault.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
                &[&[]],
            ),
            1,
        )?;
        Ok(())
    }

    // Buy at ask price
    pub fn buy(
        ctx: Context<Buy>,
        _listing_bump: u8,
        _market_vault_bump: u8,
        nft_vault_bump: u8,
    ) -> ProgramResult {
        // Active listing
        require!(
            ctx.accounts.listing.lock == false,
            error::FortuneError::LockedListing
        );
        // Transfer NFT to buyer
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.nft_vault.to_account_info(),
                    to: ctx.accounts.signer_nft_acc.to_account_info(),
                    authority: ctx.accounts.nft_vault.to_account_info(),
                },
                &[&[
                    &b"vault"[..],
                    &ctx.accounts.nft_mint.key().as_ref(),
                    &[nft_vault_bump],
                ]],
            ),
            1,
        )?;
        // Transfer lamport price to seller
        invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.signer.key(),
                &ctx.accounts.seller.key(),
                ctx.accounts.listing.ask,
            ),
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.seller.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[],
        )?;
        // Transfer fees to market vault
        let fee_amount: u64 =
            (ctx.accounts.listing.ask * ctx.accounts.market.listing_fee) / FEE_SCALAR;
        invoke_signed(
            &system_instruction::transfer(
                &ctx.accounts.signer.key(),
                &ctx.accounts.market_vault.key(),
                fee_amount,
            ),
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.market_vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[],
        )?;
        // Sync native for market vault
        let ix = sync_native(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.market_vault.key(),
        )?;
        invoke_signed(
            &ix,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.market_vault.to_account_info(),
            ],
            &[],
        )?;
        ctx.accounts.listing.lock = true;
        Ok(())
    }
    // Change the ask price TODO: orderbook
    pub fn ask(ctx: Context<Ask>, amount: u64, _listing_bump: u8) -> ProgramResult {
        ctx.accounts.listing.ask = amount;
        Ok(())
    }

    // Close a listing and recover rent
    pub fn close_listing(
        ctx: Context<CloseListing>,
        _listing_bump: u8,
        nft_vault_bump: u8,
    ) -> ProgramResult {
        // Unlocked
        require!(
            ctx.accounts.listing.lock == false,
            error::FortuneError::LockedListing
        );
        // Transfer NFT to seller
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.nft_vault.to_account_info(),
                    to: ctx.accounts.signer_nft_acc.to_account_info(),
                    authority: ctx.accounts.nft_vault.to_account_info(),
                },
                &[&[
                    &b"vault"[..],
                    &ctx.accounts.nft_mint.key().as_ref(),
                    &[nft_vault_bump],
                ]],
            ),
            1,
        )?;
        Ok(())
    }

    pub fn withdraw_fees(
        ctx: Context<WithdrawFees>,
        amount: u64,
        market_vault_bump: u8,
    ) -> ProgramResult {
        // Transfer lamports to authority
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::Transfer {
                    from: ctx.accounts.market_vault.to_account_info(),
                    to: ctx.accounts.target_wsol_acc.to_account_info(),
                    authority: ctx.accounts.market_vault.to_account_info(),
                },
                &[&[
                    &b"vault"[..],
                    &ctx.accounts.market.key().as_ref(),
                    &ctx.accounts.native_mint.key().as_ref(),
                    &[market_vault_bump],
                ]],
            ),
            amount,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(
    listing_fee: u64,
    market_vault_bump: u8)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    // Initialize market account
    #[account(
        init,
        space = 240,
        payer = signer,
    )]
    pub market: Account<'info, Market>,
    // Lamport account owned by market
    #[account(
        init,
        payer = signer,
        token::mint = native_mint,
        token::authority = market_vault,
        seeds = [b"vault", market.key().as_ref(), native_mint.key().as_ref()],
        bump = market_vault_bump
    )]
    pub market_vault: Account<'info, TokenAccount>,
    #[account(address = spl_token::native_mint::ID)]
    pub native_mint: Account<'info, Mint>,
    // System programs + sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(
    ask: u64,
    listing_bump: u8,
    nft_vault_bump: u8)]
pub struct CreateListing<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    // Listing are identified by their market + nft account
    #[account(
        init,
        space = 240,
        payer = signer,
        seeds = [b"listing", market.key().as_ref(), nft_mint.key().as_ref(), signer.key().as_ref()],
        bump = listing_bump
    )]
    pub listing: Account<'info, Listing>,
    // Market TODO: Any market can be passed in here
    #[account(mut)]
    pub market: Account<'info, Market>,
    // Vault for nft
    #[account(
        init_if_needed,
        payer = signer,
        token::mint = nft_mint,
        token::authority = nft_vault,
        seeds = [b"vault", nft_mint.key().as_ref()],
        bump = nft_vault_bump
    )]
    pub nft_vault: Account<'info, TokenAccount>,
    // Holds the NFT, owned by the signer
    #[account(
        mut,
        constraint = nft_account.owner == signer.key(),
        constraint = nft_account.mint == nft_mint.key()
    )]
    pub nft_account: Account<'info, TokenAccount>,
    // Mint address identifies the NFT
    #[account()]
    pub nft_mint: Account<'info, Mint>,
    // System programs + sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(
    listing_bump: u8,
    market_vault_bump: u8,
    nft_vault_bump: u8)]
pub struct Buy<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init_if_needed,
        payer = signer,
        token::mint = nft_mint,
        token::authority = signer,
    )]
    pub signer_nft_acc: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        close = seller,
        seeds = [b"listing", market.key().as_ref(), nft_mint.key().as_ref(), seller.key().as_ref()],
        bump = listing_bump
    )]
    pub listing: Box<Account<'info, Listing>>,
    // Listing seller
    #[account(
        mut,
        constraint = seller.key() == listing.seller)]
    pub seller: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = market.key() == listing.market)]
    pub market: Box<Account<'info, Market>>,
    // Lamport account owned by market
    #[account(
        mut,
        seeds = [b"vault".as_ref(), market.key().as_ref(), native_mint.key().as_ref()],
        bump = market_vault_bump
    )]
    pub market_vault: Box<Account<'info, TokenAccount>>,
    // Vault for nft
    #[account(
        mut,
        seeds = [b"vault", nft_mint.key().as_ref()],
        bump = nft_vault_bump
    )]
    pub nft_vault: Box<Account<'info, TokenAccount>>,
    #[account()]
    pub nft_mint: Box<Account<'info, Mint>>,
    #[account(address = spl_token::native_mint::ID)]
    pub native_mint: Account<'info, Mint>,
    // System programs + sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(
    amount: u64,
    listing_bump: u8)]
pub struct Ask<'info> {
    // Only seller can ask
    #[account(
        mut,
        constraint = signer.key() == listing.seller)]
    signer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"listing", market.key().as_ref(), nft_mint.key().as_ref(), signer.key().as_ref()],
        bump = listing_bump
    )]
    pub listing: Account<'info, Listing>,
    #[account(
        mut,
        constraint = market.key() == listing.market)]
    pub market: Account<'info, Market>,
    #[account()]
    pub nft_mint: Account<'info, Mint>,
    // System programs + sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(
    listing_bump: u8,
    nft_vault_bump: u8)]
pub struct CloseListing<'info> {
    #[account(
        mut,
        constraint = signer.key() == listing.seller)]
    pub signer: Signer<'info>,
    #[account(
        init_if_needed,
        payer = signer,
        token::mint = nft_mint,
        token::authority = signer,
    )]
    pub signer_nft_acc: Box<Account<'info, TokenAccount>>,
    // Vault for nft
    #[account(
        mut,
        constraint = nft_vault.mint == listing.nft_mint,
        constraint = nft_vault.mint == nft_mint.key(),
        seeds = [b"vault", nft_mint.key().as_ref()],
        bump = nft_vault_bump
    )]
    pub nft_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        close = signer,
        seeds = [b"listing", market.key().as_ref(), nft_mint.key().as_ref(), signer.key().as_ref()],
        bump = listing_bump,
    )]
    pub listing: Account<'info, Listing>,
    // Market
    #[account(mut)]
    pub market: Account<'info, Market>,
    #[account()]
    pub nft_mint: Account<'info, Mint>,
    // System programs + sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(
    amount: u64,
    market_vault_bump: u8)]
pub struct WithdrawFees<'info> {
    #[account(
        mut,
        constraint = signer.key() == market.authority)]
    pub signer: Signer<'info>,
    // Doesn't have to be owned by the signer
    #[account(
        mut,
        constraint = target_wsol_acc.mint == native_mint.key())]
    pub target_wsol_acc: Account<'info, TokenAccount>,
    // Market
    #[account(
        mut,
        constraint = market.lamport_vault == market_vault.key())]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        seeds = [b"vault".as_ref(), market.key().as_ref(), native_mint.key().as_ref()],
        bump = market_vault_bump
    )]
    pub market_vault: Box<Account<'info, TokenAccount>>,
    #[account(address = spl_token::native_mint::ID)]
    pub native_mint: Account<'info, Mint>,
    // System programs + sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(Default)]
// Market manages listings and collects lamport fees
pub struct Market {
    authority: Pubkey,
    lamport_vault: Pubkey,
    listing_fee: u64,
}

#[account]
#[derive(Default)]
// Listings manage nft accounts and sale information
pub struct Listing {
    market: Pubkey,
    seller: Pubkey,
    nft_mint: Pubkey,
    ask: u64,
    lock: bool,
}
